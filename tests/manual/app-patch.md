# App patch checklist (PAT-09, PAT-10) — template

No Swift test target exists (CLAUDE.md), so the Patch screen of `specs/modules/patch.md` PAT-10
is checked by hand. Copy to `tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: fetch the QLC+ set once (`subprojects/engine/scripts/fetch-qlcplus.sh`). Two terminals.
Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn` (wait for
`library: … profiles`). App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open
.build/ColourmotionDMX.app`. Optional: an Art-Net monitor on universe 1.
The demo show (IPC-14) uses universe 1 channels 1–150.

## Getting there (PAT-10)
- [ ] The workspace rail on the left has Program and Patch; Patch fills the window with the patch
      table, Program brings back the fixtures/groups windows, attribute bar and faders
- [ ] The table lists the demo show sorted by number: 1–8, 101–104, 201–208, 211–216, 221–226,
      301–306, with name, profile, mode, universe.address (e.g. `1.45` for 201) and footprint;
      the RGB+Lime and Showtec pars say "virtual dimmer"

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
- [ ] Select 101 (a wash), set its Pan in Program, come back, Change Mode: the button asks to
      confirm first; confirming with the same mode keeps the value, an unknown mode is refused
- [ ] Remove 1222: the button asks to confirm; after confirming the row is gone, its channels
      (1.207–212) go to 0 on Art-Net, and it leaves any group it was in

## Still works
- [ ] A second app window (or a restart of the app) shows the same patch: the table comes from the
      snapshot and the `fixtures` deltas (PAT-09)
- [ ] Stop and restart the daemon: the banner shows, the app reconnects and the table is the demo
      show again (the patch is not saved until the show file, P1-09)
