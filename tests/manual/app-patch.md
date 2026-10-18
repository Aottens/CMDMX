# App patch checklist (PAT-09, PAT-10) — template

No Swift test target exists (CLAUDE.md), so the Patch screen of `specs/modules/patch.md` PAT-10
is checked by hand. Copy to `tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: fetch the QLC+ set once (`subprojects/engine/scripts/fetch-qlcplus.sh`). Two terminals.
Daemon: `rm -rf /tmp/cmdmx-patch && cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-patch` (a fresh Demo; wait for
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
- [ ] Select 101 (a wash, one mode): Mode is a dropdown showing only its mode, and Change stays
      disabled (PAT-10 amended, VS2-16)
- [ ] Add one Shehds LED Beam+Wash 19x15W in 16 Channel, select it and set its Pan in Program;
      back on Patch the Mode dropdown lists 16 Channel and 24 Channel with 16 Channel picked; pick
      24 Channel: Change asks to confirm first; after confirming the row shows 24 Channel with
      footprint 24 (or the overlap warning) and Pan is kept
- [ ] Remove 1222: the button asks to confirm; after confirming the row is gone, its channels
      (1.207–212) go to 0 on Art-Net, and it leaves any group it was in

## Still works
- [ ] A second app window (or a restart of the app) shows the same patch: the table comes from the
      snapshot and the `fixtures` deltas (PAT-09)
- [ ] Stop and restart the daemon: the banner shows, the app reconnects and the table is the patch
      as you left it, because the show is autosaved and reopened (SHW-10/11)
