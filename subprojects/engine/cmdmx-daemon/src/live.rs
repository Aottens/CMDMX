//! IPC-58, IPC-59: the live output for the app. Made on the daemon loop between ticks from
//! `Engine::explain` (ENG-25), never inside `Engine::tick`, at most 25 times per second.
//!
//! ponytail: the feed and every watcher each call `explain` per fixture (ceiling: many fixtures
//! x watchers at 25 Hz). Upgrade: explain once per look and share it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{SyncSender, TrySendError};

use cmdmx_core::attribute::AttributeDef;
use cmdmx_core::colour::rgb_equivalent;
use cmdmx_core::engine::{Base, Engine, Explanation, Modifier, Source};
use cmdmx_core::patch::FixtureId;
use cmdmx_core::value::Value;
use cmdmx_core::UniverseId;
use serde::Serialize;

use crate::ipc::{playback_label, PROTOCOL};

/// IPC-58, IPC-59: the rate cap, 25 per second.
pub const EVERY: f64 = 0.04;

/// IPC-58: a fixture id is a number, a cell's `"201.3"`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
enum Id {
    Fixture(FixtureId),
    Cell(String),
}

/// IPC-58: what one fixture (or cell, `"201.3"`) outputs.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    id: Id,
    i: f32,
    rgb: Option<[f32; 3]>,
    source: Option<String>,
    tracked: bool,
    effect: bool,
}

fn level(v: &Value) -> f32 {
    match v {
        Value::Level(l) => *l,
        Value::Colour(_) => 0.0,
    }
}

/// IPC-59: what gives the value: the playback (and cue), Programmer, Held; `None` when dark by default.
fn base_label(engine: &Engine, base: &Base) -> Option<String> {
    match base.source {
        Source::Programmer => Some("Programmer".into()),
        Source::Held => Some("Held".into()),
        Source::Default => None,
        Source::Playback { id, cue } => playback_label(engine, id).map(|l| match cue {
            Some(c) => format!("{l} \u{b7} Q{}", f64::from(c)),
            None => l,
        }),
    }
}

/// IPC-58: what holds a lit value down to 0: the first master below full, the Grand Master, DBO.
fn held_down(engine: &Engine, modifiers: &[Modifier]) -> Option<String> {
    modifiers.iter().find_map(|m| match m {
        Modifier::Master {
            playback, level, ..
        } if *level < 1.0 => Some(format!(
            "Master {} {} %",
            playback_label(engine, *playback).unwrap_or_default(),
            (level * 100.0).round()
        )),
        Modifier::GrandMaster(l) => Some(format!("Grand Master {} %", (l * 100.0).round())),
        Modifier::Dbo => Some("DBO".into()),
        _ => None,
    })
}

/// ENG-20, ENG-22: the Colour plus each emitter's level times its RGB equivalent, clamped.
fn colour(own: &[&Explanation], defs: &[&AttributeDef]) -> Option<[f32; 3]> {
    let Value::Colour(t) = own.iter().find(|x| x.name == "Colour")?.value else {
        return None;
    };
    let mut rgb = [t.r, t.g, t.b];
    for d in defs.iter().filter(|d| d.emitter) {
        let (Some(eq), Some(x)) = (
            d.cie.and_then(rgb_equivalent),
            own.iter().find(|x| x.name == d.name),
        ) else {
            continue;
        };
        // The auto part of an auto-mixed emitter is already in the Colour (ENG-21).
        let l = if x.modifiers.contains(&Modifier::AutoMix) {
            level(&x.base.value)
        } else {
            level(&x.value)
        };
        for (c, e) in rgb.iter_mut().zip(eq) {
            *c += l * e;
        }
    }
    Some(rgb.map(|c| c.clamp(0.0, 1.0)))
}

/// One entry from the explanations of a fixture or cell. A cell (`parent` is the fixture's
/// Dimmer) outputs its own Dimmer (full without) times the fixture's (full without); its label
/// comes from its own Dimmer unless that is dark by default.
fn entry(
    engine: &Engine,
    id: Id,
    own: &[&Explanation],
    parent: Option<Option<&Explanation>>,
    defs: &[&AttributeDef],
) -> Entry {
    let mine = own.iter().find(|x| x.name == "Dimmer").copied();
    let (i, shown) = match parent {
        None => (mine.map_or(0.0, |d| level(&d.value)), mine),
        Some(p) => (
            mine.map_or(1.0, |d| level(&d.value)) * p.map_or(1.0, |d| level(&d.value)),
            mine.filter(|d| d.base.source != Source::Default).or(p),
        ),
    };
    let source = shown.and_then(|d| {
        if i <= 0.0 && level(&d.base.value) > 0.0 {
            if let Some(h) = held_down(engine, &d.modifiers) {
                return Some(h);
            }
        }
        base_label(engine, &d.base)
    });
    Entry {
        id,
        i,
        rgb: colour(own, defs),
        source,
        tracked: shown.is_some_and(|d| d.base.tracked),
        effect: shown.is_some_and(|d| d.modifiers.contains(&Modifier::Effect)),
    }
}

/// IPC-58: one entry per patched, not parked fixture and per cell in play, sorted by id.
pub fn entries(engine: &Engine) -> Vec<Entry> {
    let mut out = Vec::new();
    for f in engine.fixtures().filter(|f| !f.parked) {
        let Some(all) = engine.explain(f.id) else {
            continue;
        };
        let mode = engine.fixture_mode(f.id);
        let own: Vec<_> = all.iter().filter(|x| x.cell.is_none()).collect();
        let defs: Vec<_> = mode.iter().flat_map(|m| &m.attributes).collect();
        out.push(entry(engine, Id::Fixture(f.id), &own, None, &defs));
        let parent = own.iter().find(|x| x.name == "Dimmer").copied();
        for n in all.iter().filter_map(|x| x.cell).collect::<BTreeSet<_>>() {
            let own: Vec<_> = all.iter().filter(|x| x.cell == Some(n)).collect();
            let defs: Vec<_> = (mode.and_then(|m| m.cell(n)).iter())
                .flat_map(|c| &c.attributes)
                .collect();
            let id = Id::Cell(format!("{}.{n}", f.id));
            out.push(entry(engine, id, &own, Some(parent), &defs));
        }
    }
    out
}

#[derive(Serialize)]
struct Message<'a, T: Serialize> {
    v: u64,
    r#type: &'static str,
    fixtures: &'a [T],
}

fn to_json(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// IPC-58: the `output` deltas to every client.
pub struct Feed {
    last: Vec<Entry>,
    next: f64,
}

impl Feed {
    /// What the snapshot just told the clients.
    pub fn new(engine: &Engine) -> Self {
        Feed {
            last: entries(engine),
            next: 0.0,
        }
    }

    /// `{"type":"output","fixtures":[…]}` with the changed entries only; `None` when nothing
    /// changed or the last look was less than [`EVERY`] ago.
    pub fn delta(&mut self, engine: &Engine, now: f64) -> Option<String> {
        if now < self.next - 1e-9 {
            return None;
        }
        self.next = now + EVERY;
        let current = entries(engine);
        // ponytail: O(n^2) compare, fine for a rig; a map by id if fixtures reach thousands.
        let changed: Vec<_> = (current.iter())
            .filter(|e| !self.last.contains(e))
            .cloned()
            .collect();
        self.last = current;
        (!changed.is_empty()).then(|| {
            to_json(&Message {
                v: PROTOCOL,
                r#type: "output",
                fixtures: &changed,
            })
        })
    }
}

/// IPC-59: what a client watches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Grid,
    Dmx(UniverseId),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct GridValue {
    v: Value,
    palette: Option<u32>,
    source: Option<String>,
    tracked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct GridFixture {
    id: Id,
    values: BTreeMap<String, GridValue>,
}

/// IPC-59: every attribute of every fixture, and a row of its own for each cell after it.
fn grid(engine: &Engine) -> Vec<GridFixture> {
    let mut out = Vec::new();
    for f in engine.fixtures().filter(|f| !f.parked) {
        let Some(all) = engine.explain(f.id) else {
            continue;
        };
        let cells: BTreeSet<_> = all.iter().filter_map(|x| x.cell).collect();
        let own: Vec<_> = all.iter().filter(|x| x.cell.is_none()).collect();
        for cell in std::iter::once(None).chain(cells.into_iter().map(Some)) {
            let rows = match cell {
                None => own.clone(),
                Some(_) => all.iter().filter(|x| x.cell == cell).collect(),
            };
            let values = rows
                .into_iter()
                .map(|x| {
                    let palette = (x.base.palette)
                        .and_then(|id| engine.palettes().find(|p| p.id == id))
                        .map(|p| p.number);
                    let value = GridValue {
                        v: x.value,
                        palette,
                        source: base_label(engine, &x.base),
                        tracked: x.base.tracked,
                    };
                    (x.name.clone(), value)
                })
                .collect();
            let id = match cell {
                None => Id::Fixture(f.id),
                Some(n) => Id::Cell(format!("{}.{n}", f.id)),
            };
            out.push(GridFixture { id, values });
        }
    }
    out
}

struct Watcher {
    reply: SyncSender<String>,
    view: View,
    /// What was last sent; `None` until the first (whole) message.
    grid: Option<Vec<GridFixture>>,
    dmx: Option<Vec<u8>>,
    next: f64,
}

/// IPC-59: who watches what, one view per client.
#[derive(Default)]
pub struct Watchers {
    by_client: BTreeMap<u64, Watcher>,
}

impl Watchers {
    /// Set (or with `None` end) what `client` watches. `lines` are the output nodes' lines; a
    /// line neither they nor the engine know is refused.
    pub fn set(
        &mut self,
        client: u64,
        reply: SyncSender<String>,
        view: Option<View>,
        engine: &Engine,
        lines: &[UniverseId],
    ) -> Result<(), String> {
        let Some(view) = view else {
            self.by_client.remove(&client);
            return Ok(());
        };
        if let View::Dmx(line) = view {
            if engine.frame(line).is_none() && !lines.contains(&line) {
                return Err(format!("Unknown line {line}"));
            }
        }
        self.by_client.insert(
            client,
            Watcher {
                reply,
                view,
                grid: None,
                dmx: None,
                next: f64::NEG_INFINITY,
            },
        );
        Ok(())
    }

    /// Send each watcher what changed since its last message, at most every [`EVERY`]; a
    /// watcher whose connection is gone is dropped.
    pub fn send(&mut self, engine: &Engine, now: f64) {
        self.by_client.retain(|_, w| {
            if now < w.next - 1e-9 {
                return true;
            }
            let sent = match w.view {
                View::Grid => {
                    let all = grid(engine);
                    let changed: Vec<_> = match &w.grid {
                        None => all.clone(),
                        Some(last) => all.iter().filter(|g| !last.contains(g)).cloned().collect(),
                    };
                    if w.grid.is_some() && changed.is_empty() {
                        w.next = now + EVERY;
                        return true;
                    }
                    let msg = to_json(&Message {
                        v: PROTOCOL,
                        r#type: "output_grid",
                        fixtures: &changed,
                    });
                    w.reply.try_send(msg).map(|()| w.grid = Some(all))
                }
                View::Dmx(line) => {
                    let ch =
                        (engine.frame(line)).map_or_else(|| vec![0; 512], |f| f.slots().to_vec());
                    if w.dmx.as_ref() == Some(&ch) {
                        w.next = now + EVERY;
                        return true;
                    }
                    let msg = to_json(&serde_json::json!(
                        {"v": PROTOCOL, "type": "output_dmx", "line": line, "channels": ch}
                    ));
                    w.reply.try_send(msg).map(|()| w.dmx = Some(ch))
                }
            };
            match sent {
                Ok(()) => {
                    w.next = now + EVERY;
                    true
                }
                // A slow client keeps its place and gets the whole change at the next look.
                Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            }
        });
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.by_client.len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use cmdmx_core::colour::TargetColour;
    use cmdmx_core::engine::Settings;
    use cmdmx_core::patch::Fixture;
    use cmdmx_core::playback::{Cue, CueNumber, ListAction, Playback, Values};
    use cmdmx_core::profile::Profile;
    use cmdmx_core::programmer::Item;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::sync::mpsc::{sync_channel, Receiver};

    const DIMMER: &str = r#"{ "manufacturer": "T", "model": "Dimmer", "source": "channel-list",
      "modes": [ { "name": "1ch", "footprint": 1,
        "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } ] }"#;

    /// RGBW par without auto-mix: Dimmer 1, RGB 2–4, White 5 at D65 (ENG-20, ENG-22).
    const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW", "source": "channel-list",
      "modes": [ { "name": "5ch", "footprint": 5,
        "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
        "attributes": [
          { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
          { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true,
            "cie": [0.3127, 0.3290] } ] } ] }"#;

    fn fixture(id: u32, profile: &str, universe: UniverseId, address: u16) -> Fixture {
        Fixture {
            parked: false,
            id,
            number: 0,
            name: String::new(),
            handle: None,
            profile: profile.into(),
            mode: None,
            universe,
            address,
        }
    }

    fn dimmer(fixture: u32, level: f32) -> Values {
        BTreeMap::from([(
            Item::fixture(fixture),
            BTreeMap::from([("Dimmer".to_string(), level.into())]),
        )])
    }

    /// Dimmers 1 and 2 on universe 1, playback 1 "Dienst" on 1.1 holding both at full.
    fn engine() -> Engine {
        let mut e = Engine::new(Settings::default());
        let profile: Profile = serde_json::from_str(DIMMER).unwrap();
        e.add_profile("dimmer", profile).unwrap();
        e.patch(fixture(1, "dimmer", 1, 1)).unwrap();
        e.patch(fixture(2, "dimmer", 1, 2)).unwrap();
        let mut both = dimmer(1, 1.0);
        both.extend(dimmer(2, 1.0));
        e.add_playback(Playback::new(1, 1, 1, both));
        e.set_playback_legend(1, "Dienst", None, None).unwrap();
        e.set_fader(1, 1.0, 0.0).unwrap();
        e.tick(0.0);
        e
    }

    /// Through the wire text, as a client reads it (`to_value` would widen the f32s).
    fn json_of(entries: &[Entry]) -> Value {
        parse(&serde_json::to_string(entries).unwrap())
    }

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    fn drain(rx: &Receiver<String>) -> Vec<Value> {
        rx.try_iter().map(|m| parse(&m)).collect()
    }

    #[test]
    fn ipc_58_an_entry_per_fixture_with_its_intensity_and_source() {
        let mut e = engine();
        e.patch(fixture(3, "dimmer", 1, 3)).unwrap();
        e.patch_park(&[3]).unwrap();
        e.tick(0.025);
        assert_eq!(
            json_of(&entries(&e)),
            json!([
                {"id":1,"i":1.0,"rgb":null,"source":"Dienst","tracked":false,"effect":false},
                {"id":2,"i":1.0,"rgb":null,"source":"Dienst","tracked":false,"effect":false}
            ]),
            "a parked fixture has no entry"
        );
        e.set_fader(1, 0.0, 0.025).unwrap();
        e.tick(0.05);
        assert_eq!(
            json_of(&entries(&e))[0],
            json!({"id":1,"i":0.0,"rgb":null,"source":null,"tracked":false,"effect":false}),
            "dark by default"
        );
    }

    #[test]
    fn ipc_58_colour_is_the_colour_plus_direct_emitters() {
        let mut e = Engine::new(Settings::default());
        let profile: Profile = serde_json::from_str(RGBW).unwrap();
        e.add_profile("rgbw", profile).unwrap();
        e.patch(fixture(7, "rgbw", 1, 1)).unwrap();
        e.set_programmer_value(7, "Dimmer", 0.5).unwrap();
        let blue = TargetColour {
            r: 0.0,
            g: 0.0,
            b: 1.0,
        };
        e.set_programmer_value(7, "Colour", blue).unwrap();
        e.set_programmer_value(7, "White", 0.5).unwrap();
        e.tick(0.0);
        assert_eq!(
            json_of(&entries(&e)),
            json!([{"id":7,"i":0.5,"rgb":[0.5,0.5,1.0],"source":"Programmer","tracked":false,
                    "effect":false}])
        );
    }

    #[test]
    fn ipc_58_a_list_names_its_cue_and_says_tracked() {
        let mut e = engine();
        let q = |n| CueNumber::new(n).unwrap();
        let q2 = Cue {
            number: q(2.0),
            ..Cue::new(dimmer(2, 0.5))
        };
        e.add_playback(Playback {
            cues: vec![Cue::new(dimmer(1, 0.8)), q2],
            ..Playback::new(2, 1, 2, BTreeMap::new())
        });
        e.set_playback_legend(2, "Lied", None, None).unwrap();
        e.set_fader(1, 0.0, 0.0).unwrap();
        e.set_fader(2, 1.0, 0.0).unwrap();
        e.cuelist(Some(2), ListAction::Go, 0.0).unwrap();
        e.tick(0.025);
        e.cuelist(Some(2), ListAction::Go, 0.05).unwrap();
        e.tick(0.075);
        let v = json_of(&entries(&e));
        assert_eq!(
            v[0],
            json!({"id":1,"i":0.8,"rgb":null,"source":"Lied · Q2","tracked":true,"effect":false})
        );
        assert_eq!(v[1]["source"], "Lied · Q2");
        assert_eq!(v[1]["tracked"], false);
    }

    #[test]
    fn ipc_58_a_master_at_0_gives_its_label() {
        let mut e = engine();
        e.select_fixture(1).unwrap();
        e.record_group(3).unwrap();
        e.clear_selection();
        let master = e.record_group_master(1, 8, 3).unwrap();
        e.set_playback_legend(master, "Zaal", None, None).unwrap();
        e.set_fader(master, 0.0, 0.0).unwrap();
        e.tick(0.025);
        let v = json_of(&entries(&e));
        assert_eq!(v[0]["i"], 0.0);
        assert_eq!(v[0]["source"], "Master Zaal 0 %");
        assert_eq!(v[1]["source"], "Dienst", "not in the group");
    }

    #[test]
    fn ipc_58_a_change_is_sent_once_and_nothing_while_nothing_changes() {
        let mut e = engine();
        let mut feed = Feed::new(&e);
        assert_eq!(feed.delta(&e, 0.0), None, "the snapshot told it all");
        e.set_fader(1, 0.5, 0.1).unwrap();
        e.tick(0.1);
        let d = parse(&feed.delta(&e, 0.1).unwrap());
        assert_eq!(d["v"], 1);
        assert_eq!(d["type"], "output");
        assert_eq!(d["fixtures"].as_array().unwrap().len(), 2);
        assert_eq!(d["fixtures"][0]["i"], 0.5);
        for n in 1..10 {
            let now = 0.1 + f64::from(n) * 0.1;
            e.tick(now);
            assert_eq!(feed.delta(&e, now), None, "tick {n}");
        }
        e.set_programmer_value(2, "Dimmer", 1.0).unwrap();
        e.tick(1.1);
        let d = parse(&feed.delta(&e, 1.1).unwrap());
        assert_eq!(
            d["fixtures"],
            json!([{"id":2,"i":1.0,"rgb":null,"source":"Programmer","tracked":false,
                    "effect":false}]),
            "only the changed entry"
        );
    }

    #[test]
    fn ipc_58_the_rate_cap() {
        // At most 25 a second (IDEA-44): every other 25 ms tick.
        let mut e = engine();
        let mut feed = Feed::new(&e);
        e.set_fader(1, 0.5, 0.0).unwrap();
        e.tick(0.0);
        assert!(feed.delta(&e, 0.0).is_some());
        e.set_fader(1, 0.4, 0.025).unwrap();
        e.tick(0.025);
        assert_eq!(
            feed.delta(&e, 0.025),
            None,
            "less than 0.04 s after the last"
        );
        e.set_fader(1, 0.3, 0.05).unwrap();
        e.tick(0.05);
        let d = parse(&feed.delta(&e, 0.05).unwrap());
        assert_eq!(d["fixtures"][0]["i"], json!(0.3), "the newest value");
        e.set_fader(1, 0.2, 0.075).unwrap();
        e.tick(0.075);
        assert_eq!(feed.delta(&e, 0.075), None);
        e.tick(0.1);
        assert!(feed.delta(&e, 0.1).is_some(), "20 a second on a 40 Hz tick");
    }

    #[test]
    fn ipc_59_watching_the_grid_sends_it_whole_then_changed_fixtures_only() {
        let mut e = engine();
        let mut w = Watchers::default();
        let (tx, rx) = sync_channel(16);
        w.set(1, tx, Some(View::Grid), &e, &[]).unwrap();
        w.send(&e, 0.0);
        let m = drain(&rx);
        assert_eq!(m.len(), 1);
        assert_eq!(
            m[0],
            json!({"v":1,"type":"output_grid","fixtures":[
                {"id":1,"values":{"Dimmer":{"v":1.0,"palette":null,"source":"Dienst","tracked":false}}},
                {"id":2,"values":{"Dimmer":{"v":1.0,"palette":null,"source":"Dienst","tracked":false}}}]})
        );
        w.send(&e, 0.05);
        assert!(drain(&rx).is_empty(), "nothing changed");
        e.set_programmer_value(2, "Dimmer", 0.5).unwrap();
        e.tick(0.075);
        w.send(&e, 0.075);
        assert!(
            drain(&rx).is_empty(),
            "at most one look per 0.04 s, also a quiet one"
        );
        w.send(&e, 0.1);
        assert_eq!(
            drain(&rx),
            vec![json!({"v":1,"type":"output_grid","fixtures":[
                {"id":2,"values":{"Dimmer":{"v":0.5,"palette":null,"source":"Programmer","tracked":false}}}]})],
            "the programmer overrides (ENG-04); only fixture 2 changed"
        );
    }

    #[test]
    fn ipc_59_grid_change_unwatch_and_rate_cap() {
        let mut e = engine();
        let mut w = Watchers::default();
        let (tx, rx) = sync_channel(16);
        w.set(1, tx.clone(), Some(View::Grid), &e, &[]).unwrap();
        w.send(&e, 0.0);
        drain(&rx);
        e.set_fader(1, 0.0, 0.0).unwrap();
        e.set_programmer_value(2, "Dimmer", 0.5).unwrap();
        e.tick(0.025);
        w.send(&e, 0.025);
        assert!(drain(&rx).is_empty(), "less than 0.04 s after the last");
        w.send(&e, 0.05);
        let m = drain(&rx);
        assert_eq!(m.len(), 1);
        assert_eq!(
            m[0]["fixtures"],
            json!([
                {"id":1,"values":{"Dimmer":{"v":0.0,"palette":null,"source":null,"tracked":false}}},
                {"id":2,"values":{"Dimmer":{"v":0.5,"palette":null,"source":"Programmer","tracked":false}}}])
        );
        w.set(1, tx, None, &e, &[]).unwrap();
        assert_eq!(w.len(), 0);
        e.set_fader(1, 1.0, 0.2).unwrap();
        e.tick(0.2);
        w.send(&e, 0.2);
        assert!(drain(&rx).is_empty(), "unwatched");
    }

    #[test]
    fn ipc_59_two_clients_watch_different_lines() {
        let mut e = engine();
        e.patch(fixture(5, "dimmer", 2, 10)).unwrap();
        e.set_programmer_value(5, "Dimmer", 1.0).unwrap();
        e.tick(0.025);
        let mut w = Watchers::default();
        let (tx1, rx1) = sync_channel(16);
        let (tx2, rx2) = sync_channel(16);
        w.set(1, tx1, Some(View::Dmx(1)), &e, &[]).unwrap();
        w.set(2, tx2, Some(View::Dmx(2)), &e, &[]).unwrap();
        w.send(&e, 0.025);
        let (m1, m2) = (drain(&rx1), drain(&rx2));
        assert_eq!((m1.len(), m2.len()), (1, 1));
        assert_eq!(m1[0]["type"], "output_dmx");
        assert_eq!(m1[0]["line"], 1);
        assert_eq!(m2[0]["line"], 2);
        let ch = |m: &Value| -> Vec<u64> {
            m["channels"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c.as_u64().unwrap())
                .collect()
        };
        let (c1, c2) = (ch(&m1[0]), ch(&m2[0]));
        assert_eq!((c1.len(), c2.len()), (512, 512));
        assert_eq!(&c1[..3], &[255, 255, 0]);
        assert_eq!(c2[9], 255);
        assert_eq!(c2.iter().filter(|&&c| c > 0).count(), 1);
        w.send(&e, 0.2);
        assert!(
            drain(&rx1).is_empty() && drain(&rx2).is_empty(),
            "nothing changed"
        );
        e.set_fader(1, 0.0, 0.2).unwrap();
        e.tick(0.3);
        w.send(&e, 0.3);
        assert_eq!(drain(&rx1).len(), 1, "line 1 changed");
        assert!(drain(&rx2).is_empty(), "line 2 did not");
    }

    #[test]
    fn ipc_59_one_view_at_a_time_and_an_unknown_line_is_refused() {
        let e = engine();
        let mut w = Watchers::default();
        let (tx, rx) = sync_channel(16);
        w.set(1, tx.clone(), Some(View::Grid), &e, &[]).unwrap();
        w.set(1, tx.clone(), Some(View::Dmx(1)), &e, &[]).unwrap();
        assert_eq!(w.len(), 1);
        w.send(&e, 0.0);
        let m = drain(&rx);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0]["type"], "output_dmx");
        let err = w
            .set(1, tx.clone(), Some(View::Dmx(9)), &e, &[])
            .unwrap_err();
        assert!(err.contains('9'), "{err}");
        assert!(
            w.set(1, tx, Some(View::Dmx(9)), &e, &[9]).is_ok(),
            "a node's line is known before the engine has a frame for it"
        );
    }

    /// A wash with a master Dimmer on 1 and three RGBW cells (2–5, 6–9, 10–13).
    const CELLS: &str = r#"{ "manufacturer": "T", "model": "Cells", "source": "channel-list",
      "modes": [ { "name": "13ch", "footprint": 13,
        "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ],
        "cells": [
          { "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
            "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true, "cie": [0.3127, 0.3290] } ] },
          { "colour": { "system": "RGB", "components": [ { "channel": 6 }, { "channel": 7 }, { "channel": 8 } ] },
            "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 9, "emitter": true, "cie": [0.3127, 0.3290] } ] },
          { "colour": { "system": "RGB", "components": [ { "channel": 10 }, { "channel": 11 }, { "channel": 12 } ] },
            "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 13, "emitter": true, "cie": [0.3127, 0.3290] } ] } ] } ] }"#;

    #[test]
    fn ipc_58_59_a_cell_is_an_entry_of_its_own() {
        let mut e = Engine::new(Settings::default());
        let profile: Profile = serde_json::from_str(CELLS).unwrap();
        e.add_profile("cells", profile).unwrap();
        e.patch(fixture(9, "cells", 1, 1)).unwrap();
        e.set_programmer_value(9, "Dimmer", 0.5).unwrap();
        let red = TargetColour {
            r: 1.0,
            g: 0.0,
            b: 0.0,
        };
        e.set_item_value(Item::cell(9, 2), "Colour", red).unwrap();
        e.tick(0.0);
        let v = json_of(&entries(&e));
        let ids: Vec<&Value> = v.as_array().unwrap().iter().map(|x| &x["id"]).collect();
        assert!(
            ids.contains(&&json!(9)) && ids.contains(&&json!("9.2")),
            "{v}"
        );
        let cell = v
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == "9.2")
            .unwrap();
        assert_eq!(
            cell["i"], 0.5,
            "the cell's Dimmer (full) times the fixture's"
        );
        assert_eq!(cell["source"], "Programmer", "the master Dimmer gives it");
        assert_eq!(cell["rgb"], json!([1.0, 0.0, 0.0]));
        let mut w = Watchers::default();
        let (tx, rx) = sync_channel(16);
        w.set(1, tx, Some(View::Grid), &e, &[]).unwrap();
        w.send(&e, 0.0);
        let m = drain(&rx);
        let rows = m[0]["fixtures"].as_array().unwrap();
        let row = rows.iter().find(|r| r["id"] == "9.2").unwrap();
        assert_eq!(
            row["values"]["Colour"]["v"],
            json!({"r":1.0,"g":0.0,"b":0.0})
        );
        let own = rows.iter().find(|r| r["id"] == 9).unwrap();
        assert_eq!(
            own["values"]["Colour"]["v"],
            json!({"r":1.0,"g":1.0,"b":1.0}),
            "the fixture's own Colour (it drives every cell, LIB-07); the cell's is on its own row"
        );
    }

    #[test]
    fn ipc_59_a_disconnect_ends_it() {
        let e = engine();
        let mut w = Watchers::default();
        let (tx, rx) = sync_channel(16);
        w.set(1, tx, Some(View::Grid), &e, &[]).unwrap();
        assert_eq!(w.len(), 1);
        drop(rx);
        w.send(&e, 0.0);
        assert_eq!(w.len(), 0);
    }
}
