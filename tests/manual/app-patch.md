# App patch checklist (PAT-09, PAT-10, PAT-25, LIB-22, LIB-35) — template

No Swift test target exists (CLAUDE.md), so the Patch screen of `specs/modules/patch.md` PAT-10
is checked by hand. Copy to `tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: fetch the QLC+ set once (`subprojects/engine/scripts/fetch-qlcplus.sh`). Two terminals.
Daemon: `rm -rf /tmp/cmdmx-patch && cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-patch` (a fresh Demo; wait for
`library: … profiles`). App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open
.build/CMDMX.app`. Optional: an Art-Net monitor on universe 1.
The demo show (IPC-14) uses universe 1 channels 1–150.

## Getting there (PAT-10)
- [ ] The workspace rail on the left has Program and Patch; Patch fills the window with the patch
      table, Program brings back the fixtures/groups windows, attribute bar and faders
- [ ] The table lists the demo show sorted by number: 1–8, 101–104, 201–208, 211–216, 221–226,
      301–306, with name, profile, mode, universe.address (e.g. `1.45` for 201) and footprint;
      the RGB+Lime and Showtec pars say "virtual dimmer"

## Add panel (LIB-22, PAT-10 amended)
- [ ] The Add panel is a full-width strip above the patch table, the Selected panel at the right;
      it has one search field and the columns Manufacturer, Model and Mode
- [ ] With an empty search the Manufacturer column lists every manufacturer with its number of
      models, and "In this show" on top with the number of profiles patched in the demo
- [ ] Type `phantom`: the manufacturers narrow to those with a matching model, counts are the
      matches; Model lists every match prefixed with its manufacturer, each with its number of modes
- [ ] Click Showtec: Model shows only Showtec's matches without the prefix; type `phantom 25`,
      then change it to `par 56`: Showtec stays selected while it matches; type `xyzzy`: no
      manufacturer matches, Showtec is deselected and Model says "No match"
- [ ] Pick a model: its modes appear with their channel counts, the first is picked, and the pick
      line reads "<manufacturer> <model>" with "<mode> · <n> channels · QLC+" (or "user profile");
      a user profile in the Model column carries a `USER` tag
- [ ] Click "In this show": Model lists the demo's profiles; pick one, its modes come from the show
- [ ] Edit profile… and New from channel list… are there and disabled (VS2-26)
- [ ] Keyboard: from the search field ↓ goes into the columns, ↑↓ move within a column, ←→ move
      between columns, Return adds (same as clicking Add)
- [ ] Type fast (`showtecparxx` then back to `showtec`): the lists end on the last query, no
      flicker back to an older result

## Import GDTF (LIB-34, LIB-35, IPC-45)
Before this section, download one fixture from GDTF-share (`gdtf-share.com`, needs the owner's free
account; pick a real manufacturer file, e.g. a Robe, Martin or Chauvet fixture, not a `Generic@`
one) into `~/Downloads`. Restart the daemon with `--profiles-dir /tmp/cmdmx-patch/profiles` added,
so the imports stay out of the real user profiles folder.
- [ ] The Add panel has **Import GDTF…** beside New from channel list…; it opens a file panel that
      only lets `.gdtf` files be picked
- [ ] Pick `tests/fixtures/gdtf/Generic@RGBW8@test.gdtf`: a sheet shows "Generic Generic RGBW
      8-bit" with one line per channel (mode Default, channels 1–4, the GDTF name, what it became:
      the three RGB colour channels and the White emitter) and the warnings below; Done closes it and the Add panel has Generic
      › Generic RGBW 8-bit picked, `USER` tag, mode Default · 4 channels
- [ ] In the sheet, every Became cell reads in full: long text wraps onto more lines, no "…";
      hovering a Became or GDTF name cell shows its full text. Escape closes the sheet like Done
      (reopen it by importing again for the next check) (LIB-35 amended, VS2-43)
- [ ] Import the same file again: the sheet says it replaced the existing user profile; the Model
      column still lists it once
- [ ] Pick the GDTF-share file: the sheet lists its modes and channels; write the file name and any
      warnings in the run file; the profile is picked, and adding one fixture of it puts a row in
      the patch table with the expected footprint. If the import fails (the `gdtf` crate rejects an
      emitter without `DominantWaveLength` or a wheel slot without `Color`), the error names the
      file and the problem: note it under Open points and try a second file
- [ ] Pick any non-GDTF file renamed to `.gdtf` (e.g. copy a `.txt`): an error naming the file is
      shown, no sheet, the Add panel is unchanged
- [ ] While the GDTF-share import runs, the Art-Net output (or the faders) keeps moving: output
      never waits for an import

## Add (PAT-01..03, PAT-09)
- [ ] Type `showtec par 56` in the Add panel's search: Showtec LED Par 56 is listed; pick it, its
      modes appear; pick "6 Channel"
- [ ] Count 6, universe 1, address 201, number 1221, Add: 1221–1226 appear at 1.201, 1.207, …
      1.231, footprint 6, "virtual dimmer"; the Fixtures window (Program) shows them too
- [ ] Add 2 more with address and number empty: they get the next free numbers (1227, 1228) and
      the first free channels (1.151, 1.157)
- [ ] Add 2 at address 140: refused, the panel shows the warning ("… would overlap 226 at 1.140");
      the table is unchanged
- [ ] Add one with number 201: refused ("number 201 is already in use"), nothing changes

## Edit a selected row (PAT-04..06, 13, PAT-09/10)
- [ ] Select 1221, Move to universe 1 address 300: the row shows 1.300; Move it onto 1.45:
      refused with the overlap warning
- [ ] Renumber 1221 to 1301: the row moves in the sorted table; on Program, `1301 Enter` selects it
- [ ] Name it `Front left`: the name shows in the table
- [ ] Select 101 (a wash, one mode): Mode is a dropdown showing only its mode, and Change stays
      disabled (PAT-10 amended, VS2-16)
- [ ] Add one Shehds LED Beam+Wash 19x15W in 16 Channel, select it and set its Pan in Program;
      back on Patch the Mode dropdown lists 16 Channel and 24 Channel with 16 Channel picked; pick
      24 Channel: Change asks to confirm first; after confirming the row shows 24 Channel with
      footprint 24 (or the overlap warning) and Pan is kept
- [ ] Remove 1222: the button asks to confirm; after confirming the row is gone, its channels
      (1.207–212) go to 0 on Art-Net, and it leaves any group it was in

## Multi-select (PAT-25, PAT-19..24)
Start from a fresh Demo (restart the daemon with the `rm -rf` above) and add 1221–1226 (Showtec
LED Par 56, 6 Channel) at 1.201 as in *Add*.
- [ ] Click 1221, Shift-click 1226: six rows are selected and the panel reads "Selected · 6
      fixtures"; Cmd-click 1223 drops it (5 fixtures), Cmd-click it again adds it back; click one
      row: back to one fixture ("Selected · 1224"); Cmd-A in the table selects every row
- [ ] With 1221–1226 selected, a plain click on 1223 (inside the selection) leaves only 1223
      selected, in one click (macOS 26 Table workaround, VS2-28 run); the No column shows 1221,
      not "1.221" (no thousands separator)
- [ ] Select 1221–1226: Move shows universe 1 address 201 (the first row's); Move to 2.101:
      the rows show 2.101, 2.107 … 2.131, back to back, still selected; Undo brings them back to
      1.201 … in one step (PAT-20)
- [ ] Select 1221, 1223, 1225 (Cmd-click), Move to 1.40: refused with the overlap warning naming
      the first clash; nothing moved (PAT-19)
- [ ] Select 1221–1226, Renumber from 1301: they become 1301–1306 in the same order and stay
      selected; Renumber them from 201: refused ("… already in use"), nothing changes (PAT-21)
- [ ] Name is prefilled with the first row's name and shows the hint "Par 1 numbers them Par 1,
      Par 2, …"; set `Par 1`: Par 1 … Par 6; set `Front`: all six are named Front (PAT-23)
- [ ] With the six pars selected the Mode dropdown lists only 6 Channel (the Par 56's one mode),
      picked, with Change disabled. Add two Shehds LED Beam+Wash 19x15W in 16 Channel at 2.1 (1401,
      1402), select both: 16 and 24 Channel listed, 16 picked; pick 24 Channel, Change asks to
      confirm, then refused (1401 would overlap 1402, addresses are kept); move 1402 to 2.201 and
      change both again: both show 24 Channel, footprint 24 (PAT-22)
- [ ] Add one Shehds LED Beam+Wash 19x15W in 16 Channel, select it and set its Pan in Program;
      back on Patch the Mode dropdown lists 16 Channel and 24 Channel with 16 Channel picked; pick
      24 Channel: Change asks to confirm first; after confirming the row shows 24 Channel with
      footprint 24 (or the overlap warning) and Pan is kept
- [ ] Remove 1222: the button asks to confirm; after confirming the row is gone, its channels
      (1.207–212) go to 0 on Art-Net, and it leaves any group it was in

## Multi-select (PAT-25, PAT-19..24)
Start from a fresh Demo (restart the daemon with the `rm -rf` above) and add 1221–1226 (Showtec
LED Par 56, 6 Channel) at 1.201 as in *Add*.
- [ ] Click 1221, Shift-click 1226: six rows are selected and the panel reads "Selected · 6
      fixtures"; Cmd-click 1223 drops it (5 fixtures), Cmd-click it again adds it back; click one
      row: back to one fixture ("Selected · 1224"); Cmd-A in the table selects every row
- [ ] Select 1221–1226: Move shows universe 1 address 201 (the first row's); Move to 2.101:
      the rows show 2.101, 2.107 … 2.131, back to back, still selected; Undo brings them back to
      1.201 … in one step (PAT-20)
- [ ] Select 1221, 1223, 1225 (Cmd-click), Move to 1.40: refused with the overlap warning naming
      the first clash; nothing moved (PAT-19)
- [ ] Select 1221–1226, Renumber from 1301: they become 1301–1306 in the same order and stay
      selected; Renumber them from 201: refused ("… already in use"), nothing changes (PAT-21)
- [ ] Name is prefilled with the first row's name and shows the hint "Par 1 numbers them Par 1,
      Par 2, …"; set `Par 1`: Par 1 … Par 6; set `Front`: all six are named Front (PAT-23)
- [ ] With the six pars selected the Mode dropdown lists the Par 56 modes with 6 Channel picked and
      Change disabled; pick another mode: Change asks to confirm, then all six change or the panel
      shows the refusal (PAT-22)
- [ ] Select a Shehds together with a par: Mode
      shows only modes both have (none → empty dropdown, Change disabled), nothing preselected when
      their modes differ; Edit profile… is disabled (two profiles) and enabled again with only pars
      selected
- [ ] Select three pars, Remove reads "Remove 3 fixtures", asks to confirm; after confirming the
      three rows are gone and the panel says "Select a row"; Undo brings all three back in one step
      (PAT-24)

## Still works
- [ ] A second app window (or a restart of the app) shows the same patch: the table comes from the
      snapshot and the `fixtures` deltas (PAT-09)
- [ ] Stop and restart the daemon: the banner shows, the app reconnects and the table is the patch
      as you left it, because the show is autosaved and reopened (SHW-10/11)
