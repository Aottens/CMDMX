//! Merge from another show (SHW-28..31). Everything is planned from the open state first; only
//! then, and not on a dry run, is anything changed, so a refusal changes nothing.

use std::collections::{BTreeMap, BTreeSet};

use super::Engine;
use crate::effect::Entry;
use crate::merge::{self, LeftOut, Placed, PlacedKind, Plan, Request};
use crate::palette::{self, Cell, Palette, PaletteId};
use crate::patch::FixtureId;
use crate::playback::{Cue, GroupMaster, Master, Playback, Values, PAGES};
use crate::programmer::{Grid, GroupId, Item};
use crate::show::{Show, ShowPalette};
use crate::value::Stored;
use crate::CoreError;

fn bad(msg: String) -> CoreError {
    CoreError::InvalidCommand(msg)
}

/// SHW-28: the effective map by fixture id, other id -> open id (`None` = unmapped).
struct Map {
    ids: BTreeMap<FixtureId, Option<FixtureId>>,
    /// The other show's fixture numbers by id.
    numbers: BTreeMap<FixtureId, u32>,
}

/// SHW-31: what was dropped.
#[derive(Default)]
struct Tally {
    fixtures: BTreeSet<u32>,
    values: u32,
}

impl Map {
    fn to(&self, item: Item) -> Option<Item> {
        let to = self.ids.get(&item.fixture).copied().flatten()?;
        Some(Item {
            fixture: to,
            ..item
        })
    }

    /// `count` things hang on the item: they are counted when it has no place in the open show.
    fn item(&self, item: Item, count: usize, tally: &mut Tally) -> Option<Item> {
        let to = self.to(item);
        if to.is_none() {
            tally.values += u32::try_from(count).unwrap_or(u32::MAX);
            tally.fixtures.extend(self.numbers.get(&item.fixture));
        }
        to
    }

    fn items(&self, items: &[Item], tally: &mut Tally) -> Vec<Item> {
        (items.iter())
            .filter_map(|&i| self.item(i, 1, tally))
            .collect()
    }

    fn values(
        &self,
        values: &Values,
        pals: &BTreeMap<PaletteId, PaletteId>,
        t: &mut Tally,
    ) -> Values {
        let mut out = Values::new();
        for (&item, attrs) in values {
            if let Some(to) = self.item(item, attrs.len(), t) {
                let attrs = attrs.iter().map(|(a, s)| (a.clone(), follow(*s, pals)));
                out.insert(to, attrs.collect());
            }
        }
        out
    }

    fn fixture_values<V: Clone>(
        &self,
        fixtures: &BTreeMap<Item, BTreeMap<String, V>>,
        t: &mut Tally,
    ) -> BTreeMap<Item, BTreeMap<String, V>> {
        (fixtures.iter())
            .filter_map(|(&i, attrs)| Some((self.item(i, attrs.len(), t)?, attrs.clone())))
            .collect()
    }

    fn entries(
        &self,
        entries: &[Entry],
        pals: &BTreeMap<PaletteId, PaletteId>,
        t: &mut Tally,
    ) -> Vec<Entry> {
        let mut out = entries.to_vec();
        // PAT-26 as in `Entry::retire`: an entry the map emptied goes; one without members stays.
        out.retain_mut(|e| {
            let had = !e.members.is_empty();
            e.members = self.items(&e.members, t);
            e.palette = e.palette.and_then(|p| pals.get(&p).copied());
            !(had && e.members.is_empty())
        });
        out
    }

    fn palette(
        &self,
        p: &Palette,
        pals: &BTreeMap<PaletteId, PaletteId>,
        t: &mut Tally,
    ) -> Palette {
        Palette {
            fixtures: self.fixture_values(&p.fixtures, t),
            effects: self.entries(&p.effects, pals, t),
            ..p.clone()
        }
    }
}

/// SHW-30: a reference follows its palette; one to a palette the other show lacks is its fallback.
fn follow(stored: Stored, pals: &BTreeMap<PaletteId, PaletteId>) -> Stored {
    match stored {
        Stored::Reference(mut r) => match pals.get(&r.palette) {
            Some(&to) => {
                r.palette = to;
                Stored::Reference(r)
            }
            None => stored.without_palette(r.palette),
        },
        value => value,
    }
}

/// The palettes a cue refers to through the items that have a place in the open show.
fn refs(cue: &Cue, map: &Map) -> BTreeSet<PaletteId> {
    let stored = cue.stored_items().filter(|(i, _)| map.to(**i).is_some());
    let stored = stored.flat_map(|(_, attrs)| attrs.values());
    let from_values = stored.filter_map(|s| match s {
        Stored::Reference(r) => Some(r.palette),
        Stored::Value(_) => None,
    });
    from_values
        .chain(cue.effects.iter().filter_map(|e| e.palette))
        .collect()
}

/// Entries compared without their ids and palette pointers.
fn effect_key(entries: &[Entry]) -> Vec<serde_json::Value> {
    (entries.iter())
        .map(|e| {
            let mut e = e.clone();
            (e.id, e.palette) = (0, None);
            serde_json::to_value(e).unwrap_or_default()
        })
        .collect()
}

/// SHW-30: the same window, number and values after the map.
fn identical(open: &Palette, other: &Palette, map: &Map) -> bool {
    let mapped = map.palette(other, &BTreeMap::new(), &mut Tally::default());
    open.window == mapped.window
        && open.number == mapped.number
        && open.global == mapped.global
        && open.shared == mapped.shared
        && open.shared_cells == mapped.shared_cells
        && open.fixtures == mapped.fixtures
        && effect_key(&open.effects) == effect_key(&mapped.effects)
}

/// Everything a merge changes, ready to put in.
struct Prepared {
    plan: Plan,
    groups: Vec<(GroupId, Vec<Item>, Option<Grid>, String)>,
    palettes: Vec<Palette>,
    playbacks: Vec<Playback>,
}

impl Engine {
    /// SHW-28..31: put the chosen groups, palettes and playbacks of `other` into this show, on
    /// free places, with what they use. Refused as a whole, changing nothing, when the map or
    /// the request is bad or something does not fit; `dry_run` plans without changing anything.
    /// One undo step is the caller's (`History::edit`).
    pub fn merge(
        &mut self,
        other: &Show,
        request: &Request,
        dry_run: bool,
    ) -> Result<Plan, CoreError> {
        let prepared = self.prepare_merge(other, request)?;
        if !dry_run {
            for (id, items, grid, legend) in prepared.groups {
                self.groups.insert(id, items);
                if let Some(grid) = grid {
                    self.group_grids.insert(id, grid);
                }
                if !legend.is_empty() {
                    self.group_legends.insert(id, legend);
                }
            }
            // Entry ids of the other show could be ones this show runs: fresh ones (FX-29).
            for mut palette in prepared.palettes {
                self.renumber_entries(&mut palette.effects);
                self.add_palette(palette)?;
            }
            for mut playback in prepared.playbacks {
                for cue in &mut playback.cues {
                    self.renumber_entries(&mut cue.effects);
                }
                self.add_playback(playback);
            }
        }
        Ok(prepared.plan)
    }

    fn renumber_entries(&mut self, entries: &mut [Entry]) {
        for e in entries {
            e.id = self.effect_next_id;
            self.effect_next_id = self.effect_next_id.saturating_add(1);
        }
    }

    /// SHW-28: validate the effective map.
    fn merge_map(&self, other: &Show, request: &Request) -> Result<Map, CoreError> {
        let theirs: BTreeMap<u32, _> = (other.patch.fixtures.iter())
            .map(|f| (f.number, f))
            .collect();
        let mut effective = merge::proposed_map(self, other);
        effective.extend(request.map.iter().map(|(&k, &v)| (k, v)));
        let mut users: BTreeMap<u32, u32> = BTreeMap::new();
        let mut map = Map {
            ids: BTreeMap::new(),
            numbers: (other.patch.fixtures.iter())
                .map(|f| (f.id, f.number))
                .collect(),
        };
        for (&src, &to) in &effective {
            let from = theirs
                .get(&src)
                .ok_or_else(|| bad(format!("fixture {src} is not in {}", other.meta.name)))?;
            let target = match to {
                None => None,
                Some(n) => {
                    let t = self.fixture_by_number(n).ok_or_else(|| {
                        bad(format!("fixture {src}: this show has no fixture {n}"))
                    })?;
                    if t.profile != from.profile {
                        return Err(bad(format!(
                            "fixture {src}: fixture {n} uses another profile"
                        )));
                    }
                    if let Some(prev) = users.insert(n, src) {
                        return Err(bad(format!(
                            "fixtures {prev} and {src} both map to fixture {n}"
                        )));
                    }
                    Some(t.id)
                }
            };
            map.ids.insert(from.id, target);
        }
        Ok(map)
    }

    fn prepare_merge(&self, other: &Show, request: &Request) -> Result<Prepared, CoreError> {
        if request.groups.is_empty() && request.palettes.is_empty() && request.playbacks.is_empty()
        {
            return Err(bad("nothing chosen to merge".into()));
        }
        let map = self.merge_map(other, request)?;
        let groups: BTreeMap<GroupId, _> = other.groups.iter().map(|g| (g.id, g)).collect();
        let pal_list: Vec<&Palette> = (other.palettes.palettes.iter())
            .filter_map(|p| match p {
                ShowPalette::Palette(p) => Some(p),
                ShowPalette::Unreadable(_) => None,
            })
            .collect();
        let by_number: BTreeMap<u32, &Palette> = pal_list.iter().map(|p| (p.number, *p)).collect();
        let by_id: BTreeMap<PaletteId, &Palette> = pal_list.iter().map(|p| (p.id, *p)).collect();
        let at: BTreeMap<(u16, u16), &Playback> = (other.playbacks.playbacks.iter())
            .map(|p| ((p.page, p.slot), p))
            .collect();

        // What is taken: number -> brought along (SHW-30).
        let mut take_groups: BTreeMap<GroupId, bool> = BTreeMap::new();
        for &g in &request.groups {
            if !groups.contains_key(&g) {
                return Err(bad(format!("{} has no group {g}", other.meta.name)));
            }
            take_groups.insert(g, false);
        }
        let mut take_palettes: BTreeMap<u32, bool> = BTreeMap::new();
        for &n in &request.palettes {
            if !by_number.contains_key(&n) {
                return Err(bad(format!("{} has no palette {n}", other.meta.name)));
            }
            take_palettes.insert(n, false);
        }
        let mut take_playbacks = BTreeMap::new();
        for s in &request.playbacks {
            let pb = at.get(&(s.page, s.slot)).ok_or_else(|| {
                bad(format!(
                    "{} has nothing on {}.{}",
                    other.meta.name, s.page, s.slot
                ))
            })?;
            take_playbacks.insert((s.page, s.slot), *pb);
        }
        for pb in take_playbacks.values() {
            if let Some(Master::Group(m)) = &pb.master {
                if !groups.contains_key(&m.group) {
                    return Err(bad(format!("{} has no group {}", other.meta.name, m.group)));
                }
                take_groups.entry(m.group).or_insert(true);
            }
            for cue in &pb.cues {
                for id in refs(cue, &map) {
                    if let Some(p) = by_id.get(&id) {
                        take_palettes.entry(p.number).or_insert(true);
                    }
                }
            }
        }

        // SHW-30: a brought one that the open show has identically is linked, not placed.
        let mut gmap: BTreeMap<GroupId, GroupId> = BTreeMap::new();
        let mut pals: BTreeMap<PaletteId, PaletteId> = BTreeMap::new();
        take_groups.retain(|&g, &mut brought| {
            let same = brought
                && groups.get(&g).is_some_and(|og| {
                    let items = map.items(&og.items, &mut Tally::default());
                    self.groups.get(&g) == Some(&items)
                });
            if same {
                gmap.insert(g, g);
            }
            !same
        });
        take_palettes.retain(|n, &mut brought| {
            let linked = by_number
                .get(n)
                .zip(self.palette(*n))
                .filter(|(o, open)| brought && identical(open, o, &map));
            if let Some((o, open)) = linked {
                pals.insert(o.id, open.id);
            }
            linked.is_none()
        });

        // SHW-29: places. Groups, then palettes, then playbacks, each by source number.
        let mut tally = Tally::default();
        let mut placed = Vec::new();
        let mut used_groups = BTreeSet::new();
        let mut new_groups = Vec::new();
        for (&g, &brought) in &take_groups {
            let free = |n: &GroupId| !self.groups.contains_key(n) && !used_groups.contains(n);
            let to = Some(g)
                .filter(free)
                .or_else(|| (1..).find(free))
                .ok_or_else(|| bad("no free group number".into()))?;
            used_groups.insert(to);
            gmap.insert(g, to);
            let og = groups.get(&g).ok_or(CoreError::UnknownGroup(g))?;
            let items = map.items(&og.items, &mut tally);
            new_groups.push((to, items, og.grid, og.legend.clone()));
            placed.push(placed_item(PlacedKind::Group, g, to, brought));
        }

        let mut used_numbers = BTreeSet::new();
        let mut used_cells = BTreeSet::new();
        let mut slots = Vec::new();
        for (k, (&n, &brought)) in take_palettes.iter().enumerate() {
            let p = by_number.get(&n).ok_or(CoreError::UnknownPalette(n))?;
            let number_free = |n: &u32| self.palette(*n).is_none() && !used_numbers.contains(n);
            let cell_free = |c: &Cell| !used_cells.contains(c) && self.check_free_cell(*c).is_ok();
            let keep = Cell::new(p.window, p.handle);
            let (number, cell) = if number_free(&p.number) && cell_free(&keep) {
                (p.number, keep)
            } else {
                let number = (1..=palette::MAX_NUMBER)
                    .find(number_free)
                    .ok_or(CoreError::NoFreePaletteNumbers)?;
                let handle = (1..=u16::MAX)
                    .find(|&h| cell_free(&Cell::new(p.window, h)))
                    .ok_or_else(|| bad("no free palette cell".into()))?;
                (number, Cell::new(p.window, handle))
            };
            used_numbers.insert(number);
            used_cells.insert(cell);
            let id = self
                .palette_next_id
                .saturating_add(u32::try_from(k).unwrap_or(0));
            pals.insert(p.id, id);
            slots.push((p, id, number, cell));
            placed.push(placed_item(PlacedKind::Palette, n, number, brought));
        }
        let mut new_palettes = Vec::new();
        for (p, id, number, cell) in slots {
            let mut np = map.palette(p, &pals, &mut tally);
            (np.id, np.number, np.window, np.handle) = (id, number, cell.window, cell.handle);
            new_palettes.push(np);
        }

        let fpp = self.settings.faders_per_page;
        let mut used_slots = BTreeSet::new();
        let mut new_playbacks = Vec::new();
        for (k, (&(page, slot), pb)) in take_playbacks.iter().enumerate() {
            let free = |&(pg, s): &(u16, u16)| {
                (1..=PAGES).contains(&pg)
                    && (1..=fpp).contains(&s)
                    && self.playback_at(pg, s, false).is_none()
                    && !used_slots.contains(&(pg, s))
            };
            let home = Some((page, slot)).filter(free).or_else(|| {
                (page..=PAGES)
                    .flat_map(|pg| (1..=fpp).map(move |s| (pg, s)))
                    .find(free)
            });
            let (pg, s) = home.ok_or_else(|| bad(format!("no free slot for {page}.{slot}")))?;
            if let Some(kind) = pb.master.as_ref().and_then(Master::kind) {
                let number = pb.master.as_ref().map_or(0, Master::number);
                if self.master_in_use(kind, number) {
                    return Err(bad(format!(
                        "the speed master on {page}.{slot} is already in use"
                    )));
                }
            }
            used_slots.insert((pg, s));
            let mut np = (*pb).clone();
            np.id = self
                .playback_next_id
                .saturating_add(u32::try_from(k).unwrap_or(0));
            (np.page, np.slot) = (pg, s);
            for cue in &mut np.cues {
                cue.values = map.values(&cue.values, &pals, &mut tally);
                cue.dormant = map.values(&cue.dormant, &pals, &mut tally);
                cue.order = (cue.order.iter())
                    .filter_map(|(&i, &n)| Some((map.to(i)?, n)))
                    .collect();
                cue.effects = map.entries(&cue.effects, &pals, &mut tally);
            }
            if let Some(Master::Group(m)) = &mut np.master {
                m.group = gmap.get(&m.group).copied().unwrap_or(m.group);
                m.level = GroupMaster::neutral_level(m.mode); // SHW-31, GRM-05
            }
            new_playbacks.push(np);
            placed.push(Placed {
                kind: PlacedKind::Playback,
                from: format!("{page}.{slot}"),
                to: format!("{pg}.{s}"),
                brought: false,
            });
        }

        Ok(Prepared {
            plan: Plan {
                placed,
                left_out: LeftOut {
                    fixtures: tally.fixtures.into_iter().collect(),
                    values: tally.values,
                },
            },
            groups: new_groups,
            palettes: new_palettes,
            playbacks: new_playbacks,
        })
    }
}

fn placed_item(kind: PlacedKind, from: u32, to: u32, brought: bool) -> Placed {
    Placed {
        kind,
        from: from.to_string(),
        to: to.to_string(),
        brought,
    }
}
