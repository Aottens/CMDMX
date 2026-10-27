# App output checklist (OUT-36, OUT-37) — template

No Swift test target exists (CLAUDE.md), so the protocol dots in the status bar and the Output
screen of `specs/modules/output-protocols.md` OUT-36/37 are checked by hand. The engine side
(defaults, checks, reconcile, undo) is covered by the VS6-01/02 tests; this run checks that the
app shows and drives it. Copy to `tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: two terminals, and a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-output`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --show-dir /tmp/cmdmx-output`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`
  (or the copy in `~/Applications` when Claude drives the run).
- The Eurolite USB-DMX512 PRO MK2 on USB with the LED par on its XLR (`/dev/cu.usbserial-AQ01FA9F`).
- A new show has Art-Net #1 (0:0:0), sACN #2 (universe 1) and serial `auto` #3 on universe 1.

Claude listens to the daemon's Art-Net and sACN during the run and reports what goes out where a
step says so. Steps marked **(owner)** are the ones to do yourself; Claude does the rest.

## Status bar (OUT-36)
- [ ] The status bar shows `Art-Net 1` and `sACN 1` with green dots and `Serial 1` amber (auto
      finds no ENTTEC Pro, OUT-23); hovering Serial lists `#3 universe 1: no interface`
- [ ] **(owner)** Patch one fixture on universe 2 (Patch screen): the message zone says `Universe 2
      now goes out on Art-Net 0:0:1 and sACN 2`, the bar shows `Art-Net 2` and `sACN 2`
- [ ] Restart the daemon with `--no-sacn`: the sACN dot turns grey; hovering lists both sACN nodes
      with `--no-sacn`. Restart without it again

## Output screen (OUT-37)
- [ ] **(owner)** The rail has **Output** under Patch; it fills the window with the node table,
      sorted by universe then id: universe 1 Art-Net `0:0:0 (0)` at 255.255.255.255, sACN 1
      multicast priority 100, Serial `auto`; universe 2 Art-Net `0:0:1 (1)`, sACN 2
- [ ] Each row has an Enabled tick and a state dot with its detail (serial: `no interface`)
- [ ] Untick universe 2's sACN: its dot goes grey "off", the bar shows `sACN 1`, Claude sees sACN 2
      stop while Art-Net 0:0:1 runs on; tick it again: sACN 2 resumes
- [ ] Edit universe 2's Art-Net port address to `0:0:5` + Enter: the row shows `0:0:5 (5)`, Claude
      sees Art-Net 0:0:5 instead of 0:0:1. Type `0:0:0` (in use by #1): refused in the message
      zone (`… already sends universe 1`), the field snaps back to `0:0:5`
- [ ] Edit sACN 1's priority to `150`, its unicast to `127.0.0.1`, then empty again: the row shows
      `150` and multicast; type priority `300`: refused, snaps back
- [ ] Undo (⌘Z) twice: the edits come back off one by one ("Undone: Edit output")
- [ ] Add: universe 3, protocol sACN, Add: a row sACN 3 multicast priority 100 appears, enabled
- [ ] Remove that row: the button asks `Confirm` first; after the second click the row is gone
- [ ] Patch a fixture on universe 3 (its Art-Net and sACN nodes appear), then remove both: a
      warning row above the table `Universe 3: 1 fixtures, no output`; Add sACN on universe 3 and
      the warning row goes

## Serial port (OUT-31/37) — also ticks `tests/hardware/output.md` OUT-31
- [ ] The serial row's dropdown lists `auto` and `/dev/cu.usbserial-AQ01FA9F · FT232R USB UART`
      (plus Bluetooth ports, if any); Refresh asks again
- [ ] **(owner)** Choose the Eurolite: the serial dot turns green, the par follows the fader
- [ ] Quit and restart the daemon: the app reconnects, the Output screen still shows the Eurolite
      chosen, the dot is green and the par lights again without any edit (OUT-13/31)
- [ ] **(owner)** Unplug the Eurolite and Refresh: the dropdown still shows the port, marked
      `(not connected)`; the dot turns amber with `port not found`; plug it in: green again

## Still works
- [ ] Patch, Show and the workspaces (F1–F4) still switch from the rail; Output toggles off again
