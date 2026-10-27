# App show checklist (SHW-16, SHW-17) — template

No Swift test target exists (CLAUDE.md), so the status bar and Show screen of
`specs/modules/showfile.md` SHW-16/17 are checked by hand. Copy to
`tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: two terminals, and a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-shw-c`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-shw-c`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`.

Optional: an Art-Net monitor on universe 1.

## Status bar (SHW-16)
- [ ] The status bar starts with the show name "Demo" (no longer "Demo show"). Next to it is the
      autosave age: "not saved yet", and within 5 s "saved 1 s ago", counting up every second.
- [ ] In Patch, name fixture 101 `Links`. Within 5 s the age starts again from 0 s.
- [ ] Move a fader, select a few fixtures and turn a wheel. The age keeps counting up: live state
      is not a show change.
- [ ] In Terminal, run `chmod a-w /tmp/cmdmx-shw-c`, then name 102 `Rechts`.
      - Within 5 s the age turns amber.
      - The message line shows "autosave of Demo failed: …" once.
      - Wait 10 s: the message does not come back.
- [ ] Run `chmod u+w /tmp/cmdmx-shw-c`. Within 5 s the age is no longer amber and starts again
      from 0 s.

## Show screen (SHW-17)
- [ ] In the rail, "Show" is enabled, with "screen" under it like Patch. Click it: the Show screen
      replaces the windows, as Patch does. A workspace (F1–F4) brings the windows back.
- [ ] The screen shows the open show "Demo" and the list of shows: Demo, with its modified time.
- [ ] Save As `Kerk`.
      - There is no confirm: what is on stage does not change.
      - The status bar and the screen say "Kerk".
      - The list shows Demo and Kerk.
      - `ls /tmp/cmdmx-shw-c` shows `Kerk.cmdmx.json`.
- [ ] Save As `kerk`, then Save As `a/b`. Each is refused, and the warning shows on the screen.
- [ ] New `Leeg`.
      - The first click arms the button and asks to confirm ("New Leeg — output changes now").
      - Confirm. The status bar says "Leeg", the Fixtures window and the Patch table are empty, and
        Art-Net is all 0.
- [ ] Select Kerk in the list and click Open.
      - It asks to confirm. Confirm.
      - Kerk is back, with 101 named `Links` and 102 named `Rechts`.
      - Output is as before.
- [ ] Arm Open, then click another row. The button is no longer armed.
- [ ] Open Kerk again while it is open. It is refused: "Kerk is already open".
- [ ] A show from a newer version is refused:
      - In Terminal, run `sed 's/"schema_version": 1/"schema_version": 9/' /tmp/cmdmx-shw-c/Kerk.cmdmx.json > /tmp/cmdmx-shw-c/Toekomst.cmdmx.json`.
      - Leave the Show screen and come back. The list shows Toekomst.
      - Open it. The warning says it was made by a newer CMDMX, and Kerk stays open.

## Restart (SHW-10)
- [ ] Stop the daemon (Ctrl+C) and start it again with the same command. The app reconnects, the
      status bar says "Kerk", and 101 is still named `Links`.
- [ ] Stop the daemon, run `rm /tmp/cmdmx-shw-c/state.json`, and start it again. Demo opens.

## The file
- [ ] Open `/tmp/cmdmx-shw-c/Kerk.cmdmx.json` in a text editor. It is readable JSON with
      `schema_version` 1, `meta`, `settings`, `profiles`, `patch`, `groups` and `playbacks`.
