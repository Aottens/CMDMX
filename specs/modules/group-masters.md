# Module spec: group masters (GRM)

Owner: `cmdmx-core::playback` (a group master is a playback of kind `GroupMaster`, like the
speed masters), `cmdmx-core::engine` (the tick, after the merge), `cmdmx-daemon` (IPC), the
strips and Playback Options in `subprojects/app`.
Status: **approved (owner, 2026-10-21; the six questions at the end answered as recommended, DECISIONS 2026-10-21).** VS3-06 builds GRM-01..14, 19 in the core; GRM-20 added by the owner 2026-10-22 (VS3-07).
Behavioural reference: Avolites Titan v19 manual, *Running the Show → Playback controls →
Fixture Group Masters* (https://manual.avolites.com/docs/running-the-show/playback-controls):
a group saved onto a fader handle masters the intensity of its fixtures, in one of the modes
Scale (scales 100–1000 %), HTP, Limit, Take Over or Disabled; plus the Titan analysis doc linked
in `PROJECT.md` ("Masters": "Een fixture group op een fader wordt een Group Master met modi
Scale (tot 1000 %), HTP, Limit, Take Over of Disabled"). The speed masters of `cuelists.md`
(CUE-35..37) are the model for a master on a handle. Builds on `programmer.md` (groups PRG-02,
07; programmer values PRG-12..15), `playbacks.md` (handles, paging, Flash, Latch, PBK-06, 07, 14,
26..28, 30), `cuelists.md` (kinds, handle buttons, masters CUE-25, 35..41, 55), `engine-merge.md`
(ENG-01, 06, 07) and `showfile.md`.

## Purpose
A group master is a fader that masters the intensity of one group's fixtures, whatever puts
that intensity there: the programmer, memories, cue lists, chases. The church's use: the six
front spots 301–306 as group 6 on handle 1.8, `Locked` (PBK-07), in Scale mode. The
service cue list brings them up and down as it likes; pulling 1.8 down takes them out for a
video from any page, and pushing it back brings them back to wherever the cue list has them
now. (Today the front spots are a playback on 1.8, so HTP keeps them up whatever the fader
does; a group master can also take them down.)

## Definitions
- **Group master:** a playback of kind `GroupMaster` that holds no cues and refers to one
  group by id (`programmer.md` groups). Like a speed master it lives on a handle, pages, locks,
  copies, moves and deletes as a playback (CUE-37), and never enters the merge.
- **Master level** *g*: the handle's effective level (fader, or 1.0 while Flash is held or
  latched, PBK-26, 28), 0.0–1.0.
- **Intensity output** of a fixture: the value of each of its `Htp` attributes after the merge
  (ENG-01..07) and before the Grand Master (ENG-06). For a fixture with a virtual dimmer
  (LIB-04) that is the virtual `Dimmer`, so mastering it also scales its colour and emitters.
- **Mastered output:** the intensity output after every group master that holds the fixture
  (GRM-06). The Grand Master and DBO (PBK-30) then apply to the mastered output, as now.
- **Pass:** the master level moves from *a* to *b* in one tick; a value *v* is passed when it
  lies between *a* and *b*, both included (GRM-10).
- Timing as in `playbacks.md`: from the tick's timestamp only (ENG-11). Church examples use the
  demo rig of `programmer.md`, front spots 301–306 in group 6.

## Acceptance criteria

### A group master on a handle
- **GRM-01** `master.record {page, slot, kind: "Group", group}` puts a group master for that
  group on an empty handle (a used handle, or a group that does not exist, is refused with a
  warning, as CUE-35). A group may have more than one group master (Titan: a group button can be
  copied to any number of handles). In the app: Record, the Group Master softkey of the Record
  context (next to BPM and Rate Master, IPC-26), a tap on a group cell, then a tap on an empty
  handle (Titan: save the group button onto a handle).
- **GRM-02** A group master refers to its group: re-recording the group (PRG-07) changes which
  fixtures it masters from the next tick; a fixture removed from the patch drops out of it. A
  group master whose group has no fixtures masters nothing and is kept.
- **GRM-03** Group masters follow CUE-37: never in the merge, never killed or released, no
  priority, playback group or release options; they page, lock, Copy, Move and Delete as
  playbacks (PBK-07, 14). Recording, Append or Insert onto its handle is refused ("Handle 1.8
  holds the master of group 6"). Deleting a group master ends its mastering from the next tick.
- **GRM-04** Options (`master`): `mode` `Scale` (default), `Htp`, `Limit`, `TakeOver`;
  `scale` 100, 200, 400, 600 or 1000 % (default 100, used by `Scale` only); `enabled` (default
  true; Titan's *Disabled*, GRM-11). A change applies from the next tick, without a fade.
- **GRM-05** A new group master starts at the level at which it changes nothing: 1.0 in
  `Scale`, `Limit` and `TakeOver`, 0.0 in `Htp`. Its level is saved in the show (GRM-13), so a
  daemon restart or opening the show never changes the stage (owner question 1).

### What the modes do to the output (per fixture, per `Htp` attribute, per tick)
- **GRM-06** With *v* the intensity output, the masters that hold the fixture apply in this
  order:
  1. every `Scale` master: *v* × *g* × `scale` / 100, clamped to 1.0 (several multiply);
  2. every `Htp` master: max(*v*, *g*) (the highest wins);
  3. every `Limit` master: min(*v*, *g*) (the lowest wins);
  4. then the Grand Master and DBO as now (ENG-06, PBK-30).

  So a limit is always a hard limit, and an HTP master is scaled by the Grand Master like every
  other intensity. `TakeOver` masters do not act on the output (GRM-10). A fixture in no group
  master is unchanged. Front spots at 80 % from the cue list, Scale master 1.8 at 50 % → 40 %;
  with scale 200 % at 75 % → 100 % (clamped).
- **GRM-07** **Scale** (Titan: proportional control): at 100 % scale, fader full = unchanged,
  0 = out; at a higher scale the fader can lift an intensity above its recorded level (200 %:
  fader 50 % = unchanged). A Scale master at 0 takes its fixtures out but they stay where the
  playbacks and the programmer have them: pushing it up brings them back to their current
  level.
- **GRM-08** **HTP** (Titan: override level if higher than current output): the master level is
  a floor for its fixtures' intensity, so an HTP group master works as a submaster for the group
  without recording a playback. During another playback's Swop (PBK-27) it adds nothing
  (the Swop shows only the swopped playback's intensity).
- **GRM-09** **Limit** (Titan: sets a hard limit): the master level is a ceiling for its
  fixtures' intensity, whatever the programmer or a playback asks.
- **GRM-10** **Take Over** (Titan: place fixture and its intensity in programmer when level
  matched): the master does not act on the output. When the master level passes a fixture's
  current `Dimmer` output (after the masters of GRM-06, before the Grand Master), that fixture's
  `Dimmer` is put in the programmer at the master level (PRG-12: touched, so it is recorded and
  shown as a programmer value) and from then on follows the master level while the fixture is in
  the programmer; the selection does not change. A fixture taken out of the programmer (Clear or Off,
  PRG-27, 29) is picked up again on the next pass. A fixture without a `Dimmer` (real or virtual) is
  skipped. Front spots at 60 % from the cue list, Take Over master at 0 pushed up: nothing
  changes until it passes 60 %, then the spots are in the programmer and follow the fader.
- **GRM-11** **Disabled** (`enabled` false; Titan: ignore fader, locked at the current level):
  fader moves and Flash are ignored and the master keeps acting with its mode and the level it
  had when it was disabled. Enabling it again makes the fader count from the next tick; in
  `Scale`, `Htp` and `Limit` the stage jumps to the fader's level (owner question 3).

- **GRM-20** A change of `mode` puts the master at the level at which its new mode changes
  nothing (GRM-05: 0.0 for `Htp`, else 1.0): the fader and the saved level both move there, so
  switching a mode never changes the stage by itself (owner, 2026-10-22, VS3-07 run: Scale at
  100 % switched to HTP lit the whole group at full). A change of `scale` or `enabled` alone
  keeps the level (GRM-11).

### Buttons
- **GRM-12** Handle buttons (CUE-25, 55): a group master's buttons by `handle_buttons`:
  `BestPractice` and `Titan` — upper none, lower **Flash** (Titan *Flash Master*: the master
  level is 1.0 while held, PBK-26); `Custom` — kind `GroupMaster` may have Flash, Swop or none on
  each button, where Swop is Flash on this master plus PBK-27 on every playback (Titan *Swop
  fixtures* in phase 1: the group at the master's look, everything else out). `playback.flash`,
  `playback.swop` and Latch (PBK-28) work on a group master (CUE-55 refuses them only on speed
  masters); on a `TakeOver` or disabled master they are refused with a warning.

### Show file (extends `showfile.md`, CUE-38)
- **GRM-13** Schema version 5 (SHW-04 migration from 4, which changes nothing else): `kind`
  may be `GroupMaster`, with `master` `{"group":6,"mode":"Scale","scale":100,"enabled":true,
  "level":1.0}`; `level` is the master level the daemon last had (the fader, not a held Flash),
  the one live level that is saved (SHW decision 3; owner question 1), and the autosave
  (SHW-10) picks up a fader move like any change.
- **GRM-14** Lenient load (SHW-07), each with one warning naming it: a group master whose group
  is not in the show is kept and masters nothing (GRM-02); a `mode`, `scale` or `level` out of
  range is set to its default (GRM-04, 05); a group master with cues loads without them.

### IPC (extends `ipc.md`, CUE-40/41, IPC-26/28)
- **GRM-15** Snapshot `playbacks` entries of a group master carry `kind: "GroupMaster"`,
  `master` as GRM-13 and `buttons` (IPC-28); the live `level` is the fader as for every
  playback. `master.record` takes `kind: "Group"` with `group` (no `number`); `playback.options`
  takes `master {mode, scale, enabled}` (each optional, IPC-27); `fader.set` sets the level.
  A group master's stored change sends the PBK-35 `playbacks` delta.

### App (amends `ui.md` UI-22, PBK-37, CUE-44, 46)
- **GRM-16** The strip of a group master shows "Group 6" (the group's legend once groups have names, UI-12) and the meta
  line "GRP · SCALE 100 %", "GRP · HTP", "GRP · LIMIT", "GRP · TAKE OVER", with "LOCKED" added
  while disabled (amended by the owner 2026-10-22, VS3-07 run: written "GRP·SCALE 100%" so it
  fits a 92 pt strip; while disabled the line reads "LOCKED 50%" in amber with the locked level,
  and the level bar keeps following the fader so it can be set before enabling again); the level bar is the master level. A `Scale` or `Limit` master below 100 %, and
  an `Htp` master above 0, show the level in amber, so a dark or lit group is explained (Titan:
  the handle's LED flashes at zero).
- **GRM-17** Playback Options on a group master shows Handle and **Master** tabs (CUE-46): mode
  (Scale, HTP, Limit, Take Over), scale (100–1000 %, only in Scale) and a Disabled toggle.
- **GRM-18** `tests/manual/app-group-masters.md` walks: the front spots as a Scale master on 1.8
  `Locked` while the service cue list runs (out for the video, back to the list's level, on
  another page), scale 200 %, an HTP master as a submaster, a Limit master under a full
  programmer value, Take Over picking up at the spots' level, Disabled, Flash, two masters on one
  fixture, read on Art-Net, and a daemon restart with the masters and their levels back.

### Safety and determinism
- **GRM-19** Every refused group-master action returns a warning and changes nothing (PBK-43);
  nothing a group master does can stop the tick or the output (ENG-12); same patch and same
  timestamped actions → identical frames (ENG-11). Mastering a tick allocates nothing beyond what
  the tick does now (ENG-13): the masters of a fixture are found without building a list per
  fixture per tick.

## Scenario test format
Scenarios gain an optional `groups` list (`[{"id":6,"fixtures":[1,2]}]`, as the show file) and
playback entries of kind `GroupMaster` with `master` as GRM-13 (`level` sets the start level);
`fader` actions move the master. A group masters scenario per mode,
`tests/scenarios/GRM-nn-*.json`:

```json
"groups": [ { "id": 6, "fixtures": [ 1 ] } ],
"playbacks": [
  { "id": 1, "values": { "1": { "Dimmer": 0.8 } } },
  { "id": 2, "page": 1, "slot": 8, "kind": "GroupMaster", "cues": [],
    "master": { "group": 6, "mode": "Scale", "scale": 100, "level": 1.0 } } ],
"actions": [ { "t": 0.0, "fader": { "playback": 1, "level": 1.0 } },
             { "t": 0.5, "fader": { "playback": 2, "level": 0.5 } } ],
"expect": [ { "t": 0.1, "universe": 1, "channels": { "1": 204 } },
            { "t": 0.6, "universe": 1, "channels": { "1": 102 } } ]
```

Recording a group master, its options, copy/move/delete, the show file and its migration are
unit tests on `cmdmx-core` (`tests/group_masters.rs`, `tests/showfile.rs`); everything that
reaches the frame (GRM-05..12) is a scenario file, Take Over also a unit test on the programmer
contents.

## Build tasks (`BACKLOG.md`, sized after approval)
1. **VS3-06** core: kind `GroupMaster`, `master.record` for groups, options, mastering in the
   tick (GRM-06..11), buttons (GRM-12), schema 5 with migration and lenient load, scenarios
   (GRM-01..14, 19). Size M.
2. **VS3-07** IPC and the app: Record softkey, strips, Master tab, `tests/manual/app-group-masters.md`
   run (GRM-15..18). Size M.

## Out of scope (phase 1 unless noted)
Flash Fixtures, Timed Flash and Timed Flash Master with Edit Times (Titan key-profile options);
group masters on executor or touch buttons; a Playback master, Flash/Swop masters and the
Preset master (Pearl extras); the Scene Master (phase 2, Titan analysis); Size masters (phase 2);
Release All Masters; masters on LTP attributes; per-cell masters (P1-13); fader pickup on a
physical surface (CTL).

## Owner decisions (2026-10-21: each as recommended)
1. **The level is saved in the show** (GRM-05, 13), against the rule that levels are live
   state. Without it a restart puts every Scale master at 0 and takes its group out (the front
   spots dark after a daemon crash). Alternative: start every master at its no-effect level
   after a restart. Recommendation: save it.
2. **The programmer is mastered too** (GRM-06 masters the intensity output, the programmer
   included, as the Grand Master does). The Titan manual does not say; a fixture programmed at
   full under a group master at 0 then stays dark, which the amber level of GRM-16 explains.
   Alternative: master only playbacks. Recommendation: master everything.
3. **Disabled locks the last effect** (GRM-11), as Titan's "locked at the current fader level";
   re-enabling jumps to the fader. Alternatives: Disabled = no effect at all; or re-enable only
   after the fader passes the locked level (pickup). Recommendation: as written.
4. **Order Scale → HTP → Limit** when masters of different modes share a fixture (GRM-06). Titan
   does not document it; this order keeps Limit a hard limit. Fine?
5. **Default buttons** (GRM-12): lower Flash (Flash Master), upper none; Swop as a `Custom`
   choice. Titan's other button functions (Flash Fixtures, Timed Flash) stay out for now.
6. **Several masters per group** are allowed (GRM-01), as on Titan. Alternative: one per group.
