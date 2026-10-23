# App control checklist (CTL-31, CTL-41..43) — template

No Swift test target exists (CLAUDE.md), so the Control screen, learn mode on the main screen and
the status-bar Control dot of `specs/modules/control-surfaces.md` CTL-41..43 are checked by hand.
The engine side (devices, mappings, Learn, OSC, the unit order) is covered by the VS7-02/03 and
VS1-07 tests; this run checks that the app shows and drives it. Copy to
`tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: two terminals, and a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-control`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --show-dir /tmp/cmdmx-control`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app`
  (or the copy in `~/Applications` when Claude drives the run).
- No hardware: Claude runs a small CoreMIDI script that makes virtual sources (`Test Pad`,
  `BCF2000`, `X-Touch-Ext`) and sends notes, CCs and pitch bends, and sends OSC from the shell to
  port 8000. The BCF2000 itself is checked later in `tests/hardware/bcf2000.md`.

Steps marked **(owner)** are the ones to do yourself; Claude does the rest.

## Control screen (CTL-41)
- [ ] **(owner)** The rail has **Control** right after Output; it opens the Control screen
- [ ] With `Test Pad` running, the device table lists it `Disabled` (grey dot); tick Enabled: the
      dot turns green and the mapping dropdown shows its (empty) mapping
- [ ] Move a CC on `Test Pad`: the last input at the top updates live (`Test Pad · CC 7 ch 1 · 64`)
- [ ] Learn on the Add row with action `fader 1`, move CC 7: the message zone says
      `Learned: Test Pad CC 7 ch 1 → fader 1` and the bindings table shows the row
- [ ] Moving CC 7 now moves fader 1 on screen
- [ ] Edit the row in place: action `fader 2` + Enter, then CC 7 moves fader 2; tick invert: the
      fader moves the other way; type action `fader x`: refused (`unknown action "fader x"`) and
      the field snaps back
- [ ] Remove the row: it is gone; CC 7 does nothing
- [ ] Export writes a `.json` file; delete the mapping (two-step confirm); Import that file: the
      mapping is back; Import again: asked to replace, Replace keeps one copy

## Mackie unit order (CTL-31)
- [ ] With virtual `BCF2000` and `X-Touch-Ext` enabled (both get the Mackie Control profile) and
      8 faders per page: BCF2000 fader 1 moves slot 1; X-Touch-Ext fader 1 moves nothing (its
      faders 9–16 are past the page)
- [ ] **(owner)** Move `X-Touch-Ext` above `BCF2000` with the ▲▼ arrows in the device table: now
      X-Touch-Ext fader 1 moves slot 1 and BCF2000 nothing; after a daemon restart the order is kept
- [ ] A device plugged in after the daemon started appears within 2 s (CTL-16)

## OSC panel (CTL-35, 41)
- [ ] OSC is off; tick Enabled: port 8000, feedback port 9000 shown, no error
- [ ] Send `/cmdmx/fader/1 0.5` from the shell: fader 1 goes to 50 %, the client appears under
      clients heard with its address and seconds since heard
- [ ] Set port to one that is in use (Claude holds UDP 8123): the OSC error shows in amber; set it back to 8000: cleared
- [ ] Learn `go` from the Add row, send `/1/push1 1`: `Learned: OSC /1/push1 → go`, and the binding
      is in the OSC mapping (not in the mapping shown)

## Learn mode on the main screen (CTL-42)
- [ ] **(owner)** Press **Learn** on the Control screen, go to the main screen and click fader 3:
      the fader does not move, the message zone says `Move a control for fader 3 … (Esc cancels)`
- [ ] Move CC 8 on `Test Pad`: `Learned: … → fader 3`; learn mode stays on
- [ ] **(owner)** Click Go in the Go block, then press note 60: learned to `go`; the cue list did
      not go when you clicked
- [ ] Click page 2, the GM and Clear in turn, each followed by an input: learned `page 2`, `gm`,
      `key clear` (the pages bar has numbered buttons, no ▶)
- [ ] **(owner)** Esc: learn mode ends, the message clears, the screen's buttons work again

## Status bar (CTL-43)
- [ ] With `Test Pad` enabled the bar shows a green `Control` dot
- [ ] Quit the virtual `Test Pad`: the dot turns amber; hover says `Test Pad: not connected`;
      start it again: green within 2 s
- [ ] An OSC port error turns the dot amber/red with the error in the tooltip

## Still works
- [ ] Patch, Output, Show and the workspaces (F1–F4) still switch from the rail
