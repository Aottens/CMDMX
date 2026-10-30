//! Cue lists and chases running (VS3-03, VS3-04, `specs/modules/cuelists.md` CUE-10..22, 28..34,
//! 50, 51, 53).
//!
//! The live state of a list ([`ListRun`], never saved, CUE-24): current and next cue, the actions
//! since the last tick, the pause, the link timer and one fade per fixture attribute. An action
//! only records what to do; the next tick performs it at its own timestamp (PBK, Timing), so every
//! timer counts from that tick. List time stands still while the list is paused (CUE-12). The
//! merge reads the fades through [`ListRun::fade_value`]. A chase is a list whose steps are
//! started by a step clock instead of links. Nothing here reads a clock.

use std::collections::BTreeMap;

use crate::attribute::{AttributeGroup, AttributeKind};
use crate::engine::{resolve_mode, resolve_stored};
use crate::palette::{Palette, PaletteId};
use crate::patch::{Fixture, FixtureId};
use crate::playback::{
    fade_progress, stagger_ranks, CueNumber, Direction, Kind, Link, ListAction, ListLive,
    LoopAction, Origin, Origins, Playback,
};
use crate::profile::{Mode, Profile, DIMMER};
use crate::programmer::Item;
use crate::value::{Stored, Value};
use crate::Timestamp;

/// Allowance for float error when a link falls due (CUE-20).
const EPSILON: f64 = 1e-6;

/// item → attribute → the list's value for it.
type Fades = BTreeMap<Item, BTreeMap<String, ListFade>>;

/// One attribute of a running list: from its value when the cue started to the cue state's value.
#[derive(Debug, Clone, PartialEq)]
struct ListFade {
    from: Value,
    /// Kept as stored so a palette reference stays live (CUE-13).
    to: Stored,
    /// List time the fade starts: the cue's start plus delay and overlap stagger.
    start: f64,
    fade: f32,
    /// LTP order (ENG-02): the cue start that last changed it.
    order: u64,
    /// CUE-13: a cell value the cue does not hold, fading to its fixture; gone once it is there.
    released: bool,
}

impl ListFade {
    fn value(&self, clock: f64, resolve: impl Fn(Stored) -> Value) -> Value {
        let k = fade_progress((clock - self.start) as f32, self.fade);
        self.from.lerp(resolve(self.to), k)
    }
}

/// CUE-20: the cue a link will start, when.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Due {
    cue: CueNumber,
    /// List time.
    at: f64,
    /// The tick that armed it: a link never fires in its own tick.
    armed: u64,
}

/// How a cue starts: Go, Go to cue and links (`Run`) use the cue's times and follow links;
/// Back has no delay and no links (CUE-11); `Snap` is an edit to the current state (CUE-53).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum How {
    Run,
    Back,
    Snap,
}

/// What a tick reads from the engine.
pub(crate) struct Env<'a> {
    pub(crate) fixtures: &'a BTreeMap<FixtureId, Fixture>,
    pub(crate) profiles: &'a BTreeMap<String, Profile>,
    pub(crate) palettes: &'a BTreeMap<PaletteId, Palette>,
    /// The output of the LTP attributes (the previous tick's).
    pub(crate) held: &'a BTreeMap<Item, BTreeMap<String, Value>>,
}

impl<'a> Env<'a> {
    fn lookup(&self, fixture: FixtureId) -> Option<(&'a Fixture, &'a Mode)> {
        let fixture = self.fixtures.get(&fixture)?;
        Some((fixture, resolve_mode(self.profiles, fixture)?))
    }

    fn held(&self, item: Item, name: &str) -> Option<Value> {
        crate::engine::held_value(self.held, item, name)
    }
}

/// The list, the engine's view and the tick a list action is performed in.
pub(crate) struct Ctx<'a> {
    pub(crate) playback: &'a Playback,
    /// Where the list's LTP attributes release to (PBK-20).
    pub(crate) from: &'a mut Origins,
    pub(crate) env: Env<'a>,
    /// The engine's activation counter.
    pub(crate) seq: &'a mut u64,
    pub(crate) now: Timestamp,
    /// A chase's effective tempo at this tick (CUE-29); 0 stops it.
    pub(crate) bpm: f32,
}

/// A fade the transition starts, before the overlap stagger.
struct Plan {
    item: Item,
    name: String,
    ltp: bool,
    from: Value,
    to: Stored,
    delay: f32,
    fade: f32,
    order: u64,
    released: bool,
    /// Takes part in the overlap stagger and the duration (not an LTP value that is only held).
    spread: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ListRun {
    pub(crate) current: Option<CueNumber>,
    /// `None` is the end of the list, or the first cue while off.
    pub(crate) next: Option<CueNumber>,
    /// Go, Back, Stop and Go to cue since the last tick.
    pub(crate) pending: Vec<ListAction>,
    /// CUE-16: fired by its fader; the first tick starts the next cue unless Go does.
    pub(crate) fire_first: bool,
    /// CUE-53: stored cues changed; the next tick puts the current state on stage.
    pub(crate) edited: bool,
    ticks: u64,
    /// CUE-28: how far the step clock is into the current step, in steps.
    phase: f64,
    /// The chase's previous tick, for Δt.
    last: Option<Timestamp>,
    /// CUE-32: Stop paused the step clock. `paused_at` freezes the fades; Back and Go to cue
    /// let those run while the clock stays held.
    held: bool,
    /// CUE-33: a tap asks for the next step at the next tick, the clock counting from here.
    pub(crate) sync: Option<Timestamp>,
    /// CUE-30: the last step of a bounce went down.
    down: bool,
    /// CUE-30: the step shown before the current one (Back on Random). ponytail: one step deep,
    /// a second Back on Random is refused; a history if that ever matters.
    prev: Option<CueNumber>,
    /// CUE-30: steps started since the chase fired, and the state of its random sequence.
    shown: usize,
    rng: u64,
    /// List time = tick time − `offset`; frozen at `paused_at` while paused.
    offset: f64,
    paused_at: Option<Timestamp>,
    started: f64,
    duration: f32,
    progress: f32,
    fades: Fades,
    link: Option<Due>,
}

impl ListRun {
    fn clock(&self, now: Timestamp) -> f64 {
        self.paused_at.unwrap_or(now) - self.offset
    }

    /// CUE-12, 32: Stop held the list (a chase: its step clock).
    pub(crate) fn paused(&self) -> bool {
        self.paused_at.is_some() || self.held
    }

    /// CUE-10: the cue Go starts; the first cue while the list has not started one (a chase
    /// going `Backward` starts on its last step, CUE-28).
    pub(crate) fn next_cue(&self, playback: &Playback) -> Option<CueNumber> {
        self.next.or_else(|| {
            let backward =
                playback.kind == Kind::Chase && playback.chase.direction == Direction::Backward;
            let first = self.current.is_none().then(|| match backward {
                true => playback.cues.last(),
                false => playback.cues.first(),
            });
            first.flatten().map(|c| c.number)
        })
    }

    /// CUE-11, 32: the cue or step Back starts; a chase goes one step against its direction.
    pub(crate) fn back_cue(&self, playback: &Playback) -> Option<CueNumber> {
        let current = self.current?;
        if playback.kind != Kind::Chase {
            return playback.cue_before(current);
        }
        let cues = &playback.cues;
        let at = cues.iter().position(|c| c.number == current)?;
        let number = |i: usize| cues.get(i).map(|c| c.number);
        let to = |by: isize| at.checked_add_signed(by).and_then(number);
        let looping = playback.chase.looping == LoopAction::Loop;
        let wrap = |i: usize| number(i).filter(|_| looping);
        match playback.chase.direction {
            Direction::Forward => to(-1).or_else(|| wrap(cues.len().saturating_sub(1))),
            Direction::Backward => to(1).or_else(|| wrap(0)),
            Direction::Bounce => {
                let way = if self.down { -1 } else { 1 };
                to(-way).or_else(|| to(way).filter(|_| looping))
            }
            Direction::Random => self.prev,
        }
    }

    pub(crate) fn live(&self, playback: &Playback) -> ListLive {
        ListLive {
            current: self.current,
            next: self.next_cue(playback),
            paused: self.paused(),
            progress: self.progress,
        }
    }

    /// The list's value for an attribute at `now` and its LTP order, for the merge.
    pub(crate) fn fade_value(
        &self,
        item: Item,
        name: &str,
        now: Timestamp,
        resolve: impl Fn(Stored) -> Value,
    ) -> Option<(Value, u64)> {
        let fade = self.fades.get(&item).and_then(|a| a.get(name))?;
        Some((fade.value(self.clock(now), resolve), fade.order))
    }

    /// The items the list has a fade for (the tick finds the cells it must merge on their own).
    pub(crate) fn items(&self) -> impl Iterator<Item = Item> + '_ {
        self.fades.keys().copied()
    }

    /// CUE-07, 08: the current and next cue follow a renumbered cue.
    pub(crate) fn renumbered(&mut self, map: impl Fn(CueNumber) -> CueNumber) {
        self.current = self.current.map(&map);
        self.next = self.next.map(&map);
    }

    /// CUE-12: is a fade, delay, stagger or link timer unfinished?
    fn running(&self, clock: f64) -> bool {
        let unfinished = |f: &ListFade| clock < f.start + f64::from(f.fade) - EPSILON;
        self.link.is_some() || self.fades.values().flatten().any(|(_, f)| unfinished(f))
    }

    /// CUE-13: a released cell value that has arrived is let go.
    fn let_go(&mut self, now: Timestamp) {
        let clock = self.clock(now);
        for a in self.fades.values_mut() {
            a.retain(|_, f| !(f.released && clock >= f.start + f64::from(f.fade) - EPSILON));
        }
    }

    fn resume(&mut self, now: Timestamp) {
        if let Some(at) = self.paused_at.take() {
            self.offset += now - at;
        }
    }

    /// Perform the actions since the last tick, follow a due link, put an edit on stage and
    /// update the progress. Only for a list that is in the merge.
    pub(crate) fn tick(&mut self, cx: &mut Ctx) {
        if cx.playback.kind == Kind::Chase {
            return self.tick_chase(cx);
        }
        self.ticks += 1;
        let go = |a: &ListAction| matches!(a, ListAction::Go | ListAction::Goto(_));
        if std::mem::take(&mut self.fire_first) && !self.pending.iter().any(go) {
            self.pending.insert(0, ListAction::Go);
        }
        for action in std::mem::take(&mut self.pending) {
            match action {
                ListAction::Go if self.paused() => self.resume(cx.now),
                ListAction::Go => {
                    if let Some(next) = self.next_cue(cx.playback) {
                        self.start(cx, next, How::Run);
                    }
                }
                ListAction::Back => {
                    self.resume(cx.now);
                    self.back(cx);
                }
                // Stop pauses what runs; Stop on a paused list goes back (CUE-12).
                ListAction::Stop if self.paused() => {
                    self.resume(cx.now);
                    self.back(cx);
                }
                ListAction::Stop => {
                    if self.running(self.clock(cx.now)) {
                        self.paused_at = Some(cx.now);
                    }
                }
                ListAction::Goto(cue) => {
                    self.resume(cx.now);
                    self.start(cx, cue, How::Run);
                }
                ListAction::Next(_) | ListAction::Release => {}
            }
        }
        let clock = self.clock(cx.now);
        if let Some(due) = self.link.filter(|d| !self.paused() && d.armed < self.ticks) {
            if clock >= due.at - EPSILON {
                self.start(cx, due.cue, How::Run);
            }
        }
        if std::mem::take(&mut self.edited) {
            self.snap(cx);
        }
        self.let_go(cx.now);
        self.progress = match self.current {
            None => 0.0,
            Some(_) if self.duration <= 0.0 => 1.0,
            Some(_) => ((clock - self.started) / f64::from(self.duration)).clamp(0.0, 1.0) as f32,
        };
    }

    fn back(&mut self, cx: &mut Ctx) {
        if let Some(before) = self.back_cue(cx.playback) {
            self.start(cx, before, How::Back);
        }
    }

    /// CUE-28, 32, 33: a chase tick. The actions since the last tick first, then a tap's beat
    /// sync, else the step clock moves by Δt · BPM / 60 and starts the next step when it
    /// reaches one (at most one step per tick; a tick that started a step by an action or a
    /// sync does not also move the clock).
    fn tick_chase(&mut self, cx: &mut Ctx) {
        let now = cx.now;
        let dt = (now - self.last.replace(now).unwrap_or(now)).max(0.0);
        let go = |a: &ListAction| matches!(a, ListAction::Go | ListAction::Goto(_));
        if std::mem::take(&mut self.fire_first) && !self.pending.iter().any(go) {
            self.pending.insert(0, ListAction::Go);
        }
        let acted = !self.pending.is_empty();
        for action in std::mem::take(&mut self.pending) {
            match action {
                ListAction::Go if self.held => {
                    self.resume(now);
                    self.held = false;
                }
                ListAction::Go => {
                    if let Some(next) = self.next_cue(cx.playback) {
                        self.start_step(cx, next);
                    }
                }
                // Stop on a held chase goes back (CUE-32); it stays held.
                ListAction::Back => self.step_back(cx),
                ListAction::Stop if self.held => self.step_back(cx),
                ListAction::Stop => {
                    self.held = true;
                    if self.running(self.clock(now)) {
                        self.paused_at = Some(now);
                    }
                }
                ListAction::Goto(cue) => {
                    self.resume(now);
                    self.start_step(cx, cue);
                }
                ListAction::Next(_) | ListAction::Release => {}
            }
        }
        let bpm = f64::from(cx.bpm);
        let mut synced = false;
        if let Some(at) = self.sync.take().filter(|_| !self.held) {
            if !acted {
                self.step_next(cx);
            }
            self.phase = ((now - at) * bpm / 60.0).max(0.0) % 1.0;
            synced = true;
        }
        let moving = !(acted || synced || self.held) && self.current.is_some() && bpm > 0.0;
        if moving && self.next.is_some() {
            self.phase += dt * bpm / 60.0;
            if self.phase >= 1.0 - EPSILON {
                let rest = (self.phase - 1.0).max(0.0) % 1.0;
                self.step_next(cx);
                self.phase = rest;
            }
        }
        if std::mem::take(&mut self.edited) {
            self.snap(cx);
        }
        self.let_go(now);
        self.progress = match self.current {
            Some(_) => self.phase.clamp(0.0, 1.0) as f32,
            None => 0.0,
        };
    }

    /// CUE-32: Back on a chase starts the step before; the fades run, the clock stays as it is.
    fn step_back(&mut self, cx: &mut Ctx) {
        self.resume(cx.now);
        if let Some(step) = self.back_cue(cx.playback) {
            self.start_step(cx, step);
            self.prev = None;
        }
    }

    fn step_next(&mut self, cx: &mut Ctx) {
        if let Some(next) = self.next {
            self.start_step(cx, next);
        }
    }

    /// CUE-28, 29: start a step now: its values fade in over the crossfade, the step clock
    /// restarts and the step after it is chosen (CUE-30).
    fn start_step(&mut self, cx: &mut Ctx, cue: CueNumber) {
        let cues = &cx.playback.cues;
        let Some(index) = cues.iter().position(|c| c.number == cue) else {
            return;
        };
        if self.current.is_none() {
            // CUE-30: a fixed sequence per playback and step count, restarted by a kill.
            self.rng = (u64::from(cx.playback.id) << 32) ^ (cues.len() as u64);
        }
        if let Some(from) = self.current.filter(|&c| c != cue) {
            self.down = cue < from;
            self.prev = Some(from);
        }
        self.current = Some(cue);
        self.shown += 1;
        self.phase = 0.0;
        self.next = self.chase_next(cx.playback, index);
        self.transition(cx, index, How::Run);
    }

    /// CUE-30: the step after the one at `at`, none at the end of a `StopOnFinal` round.
    fn chase_next(&mut self, playback: &Playback, at: usize) -> Option<CueNumber> {
        let cues = &playback.cues;
        let number = |i: usize| cues.get(i).map(|c| c.number);
        let to = |by: isize| at.checked_add_signed(by).and_then(number);
        let looping = playback.chase.looping == LoopAction::Loop;
        let wrap = |i: usize| number(i).filter(|_| looping);
        match playback.chase.direction {
            Direction::Forward => to(1).or_else(|| wrap(0)),
            Direction::Backward => to(-1).or_else(|| wrap(cues.len().saturating_sub(1))),
            // On: the way it went, else back; the round ends on the first step on the way down.
            Direction::Bounce if !looping && self.down && at == 0 => None,
            Direction::Bounce => {
                let way = if self.down { -1 } else { 1 };
                to(way).or_else(|| to(-way)).or_else(|| number(at))
            }
            Direction::Random if !looping && self.shown >= cues.len() => None,
            Direction::Random => number(self.random_other(cues.len(), at)),
        }
    }

    /// CUE-30: a pseudo-random step other than `at` (splitmix64: no clock, no crate).
    fn random_other(&mut self, len: usize, at: usize) -> usize {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        let others = len.saturating_sub(1);
        let k = usize::try_from(z).unwrap_or(0).checked_rem(others);
        match k {
            Some(k) if k >= at => k + 1,
            Some(k) => k,
            None => at,
        }
    }

    /// CUE-13: start a cue now; the next cue is the one after it (CUE-21) and, unless it was a
    /// Back, its link is armed (CUE-20).
    fn start(&mut self, cx: &mut Ctx, cue: CueNumber, how: How) {
        self.link = None;
        let Some(index) = cx.playback.cues.iter().position(|c| c.number == cue) else {
            return;
        };
        let clock = self.clock(cx.now);
        self.current = Some(cue);
        self.next = cx.playback.cue_after(cue);
        self.started = clock;
        self.duration = self.transition(cx, index, how);
        if how != How::Run {
            return;
        }
        let next = self
            .next
            .and_then(|n| cx.playback.cues.iter().find(|c| c.number == n));
        self.link = next.and_then(|next| {
            let (offset, from) = match next.link {
                Link::Wait => return None,
                Link::After { offset } => (offset, clock + f64::from(self.duration)),
                Link::With { offset } => (offset, clock),
            };
            let at = from + f64::from(offset);
            Some(Due {
                cue: next.number,
                at,
                armed: self.ticks,
            })
        });
    }

    /// CUE-53: the current cue's state as it is now, at once; the cue deleted leaves the output.
    fn snap(&mut self, cx: &mut Ctx) {
        let index = self
            .current
            .and_then(|n| cx.playback.cues.iter().position(|c| c.number == n));
        if let Some(index) = index {
            self.transition(cx, index, How::Snap);
        }
    }

    /// CUE-13, 14: fade every attribute to the state of the cue at `index`; returns the duration
    /// (the longest delay + stagger + fade of what it fades, 0 when nothing changes).
    fn transition(&mut self, cx: &mut Ctx, index: usize, how: How) -> f32 {
        let Some(cue) = cx.playback.cues.get(index) else {
            return 0.0;
        };
        let (env, clock) = (&cx.env, self.clock(cx.now));
        let state = cx.playback.cue_state(index);
        *cx.seq = cx.seq.saturating_add(1);
        let order = *cx.seq;
        let zero = Stored::Value(Value::Level(0.0));
        // CUE-29: a chase step fades over the crossfade of its step time, with no delay, group
        // times or stagger.
        let chase_fade = (cx.playback.kind == Kind::Chase).then(|| {
            let step = if cx.bpm > 0.0 { 60.0 / cx.bpm } else { 0.0 };
            step * cx.playback.chase.crossfade / 100.0
        });
        // Delay and fade of an attribute of `group` (CUE-14, 11).
        let times = |group: AttributeGroup, rising: bool| {
            let (delay, fade) = match (chase_fade, cue.group_times.get(&group)) {
                (Some(fade), _) => (0.0, fade),
                (None, Some(g)) => (g.delay, g.fade),
                (None, None) => (cue.delay, if rising { cue.fade_in } else { cue.fade_out }),
            };
            match how {
                How::Run => (delay, fade),
                How::Back => (0.0, fade),
                How::Snap => (0.0, 0.0),
            }
        };
        let mut plans: Vec<Plan> = Vec::new();
        for (&item, attributes) in &state {
            let Some((fx, mode)) = env.lookup(item.fixture) else {
                continue;
            };
            let resolve =
                |s: Stored, name: &str| resolve_stored(env.palettes, item, fx, mode, name, s);
            for (name, &(stored, _)) in attributes {
                // PRG-53: a cell the mode lacks has no attributes: kept in the cue, plays nothing.
                let Some(l) = mode.item_logical(item.cell, name) else {
                    continue;
                };
                let old = self.fades.get(&item).and_then(|a| a.get(name));
                let ltp = l.kind == AttributeKind::Ltp;
                // CUE-13 amendment (VS2-40): a cell HTP the list does not hold comes in from where
                // the cell is within the list: full for the virtual Dimmer, else its fixture's fade.
                let from_list = match item.cell {
                    Some(cell) if old.is_none() && !ltp => {
                        if name == DIMMER && mode.virtual_cell_dimmer(cell) {
                            l.default
                        } else {
                            (self.fades.get(&Item::fixture(item.fixture)))
                                .and_then(|a| a.get(name))
                                .map_or(Value::Level(0.0), |o| o.value(clock, |s| resolve(s, name)))
                        }
                    }
                    _ => Value::Level(0.0),
                };
                // The same value as it has or fades to: nothing changes (HTP not there: 0). A cell
                // value coming in always changes something: the list holds it from now on.
                let same = match old {
                    Some(o) => !o.released && o.to == stored,
                    None => item.cell.is_none() && !ltp && stored == zero,
                };
                let to = resolve(stored, name);
                if same || !to.same_kind(&l.default) {
                    continue;
                }
                let current = match old {
                    Some(o) if !ltp => o.value(clock, |s| resolve(s, name)),
                    _ if !ltp => from_list,
                    _ => env.held(item, name).unwrap_or(l.default),
                };
                let (delay, fade) = times(l.group, ltp || to.level() >= current.level());
                plans.push(Plan {
                    item,
                    name: name.clone(),
                    ltp,
                    from: current,
                    to: stored,
                    delay,
                    fade,
                    order,
                    released: false,
                    spread: true,
                });
            }
        }
        // What the list had and the state does not hold: HTP fades out, LTP keeps its value.
        // CUE-13 amendment: a cell value (HTP and LTP) instead fades to its fixture's value in
        // the state and is then let go, so the merge gives the cell that value (PRG-51).
        for (&item, attributes) in &self.fades {
            let Some((fx, mode)) = env.lookup(item.fixture) else {
                continue;
            };
            let held_in_state = state.get(&item);
            for (name, old) in attributes {
                if held_in_state.is_some_and(|a| a.contains_key(name)) {
                    continue;
                }
                let Some(l) = mode.item_logical(item.cell, name) else {
                    continue;
                };
                let resolve = |s: Stored| resolve_stored(env.palettes, item, fx, mode, name, s);
                if let Some(cell) = item.cell.filter(|_| !old.released) {
                    let ltp = l.kind == AttributeKind::Ltp;
                    let current = if ltp {
                        env.held(item, name).unwrap_or(l.default)
                    } else {
                        old.value(clock, resolve)
                    };
                    let own = (state.get(&Item::fixture(item.fixture)))
                        .and_then(|a| a.get(name))
                        .map(|&(stored, _)| stored);
                    let to = if name == DIMMER && mode.virtual_cell_dimmer(cell) {
                        Stored::Value(l.default)
                    } else {
                        own.unwrap_or(if ltp { Stored::Value(current) } else { zero })
                    };
                    // Released: HTP over `fade_out`, LTP over `fade_in`.
                    let (delay, fade) = times(l.group, ltp);
                    plans.push(Plan {
                        item,
                        name: name.clone(),
                        ltp,
                        from: current,
                        to,
                        delay,
                        fade,
                        order: old.order,
                        released: true,
                        spread: true,
                    });
                    continue;
                }
                if old.released {
                    continue;
                }
                if l.kind == AttributeKind::Ltp {
                    let current = env.held(item, name).unwrap_or(l.default);
                    plans.push(Plan {
                        item,
                        name: name.clone(),
                        ltp: true,
                        from: current,
                        to: Stored::Value(current),
                        delay: 0.0,
                        fade: 0.0,
                        order: old.order,
                        released: false,
                        spread: false,
                    });
                } else if old.to != zero {
                    let (delay, fade) = times(l.group, false);
                    plans.push(Plan {
                        item,
                        name: name.clone(),
                        ltp: false,
                        from: old.value(clock, resolve),
                        to: zero,
                        delay,
                        fade,
                        order: old.order,
                        released: false,
                        spread: true,
                    });
                }
            }
        }
        // PBK-15: the fixtures that fade start one after the other in user-number order, or by
        // the cue's order (PRG-80).
        let rank_of = stagger_ranks(
            &cue.order,
            (plans.iter().filter(|p| p.spread))
                .filter_map(|p| Some((env.fixtures.get(&p.item.fixture)?.number, p.item))),
        );
        let stagger = if chase_fade.is_some() {
            0.0
        } else {
            1.0 - cue.overlap / 100.0
        };
        let mut duration = 0.0_f32;
        for p in plans {
            let rank = rank_of.get(&p.item).copied().unwrap_or(0);
            let lag = if p.spread {
                rank as f32 * p.fade * stagger
            } else {
                0.0
            };
            if p.spread {
                duration = duration.max(p.delay + lag + p.fade);
            }
            if p.ltp {
                // PBK-20: a kill releases to what the attribute was before the list took it.
                (cx.from.entry(p.item).or_default())
                    .entry(p.name.clone())
                    .or_insert(Origin {
                        value: p.from,
                        lag: 0.0,
                    });
            }
            let fade = ListFade {
                from: p.from,
                to: p.to,
                start: clock + f64::from(p.delay + lag),
                fade: p.fade,
                order: p.order,
                released: p.released,
            };
            self.fades.entry(p.item).or_default().insert(p.name, fade);
        }
        duration
    }
}
