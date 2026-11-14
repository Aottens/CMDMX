# App show checklist (SHW-16, SHW-17, WEB-36) — template

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
- [ ] Open Kerk again while it is open. It is refused: "Kerk is already open: its file has not changed outside CMDMX".
- [ ] A show from a newer version is refused:
      - In Terminal, run `sed 's/"schema_version": 1/"schema_version": 9/' /tmp/cmdmx-shw-c/Kerk.cmdmx.json > /tmp/cmdmx-shw-c/Toekomst.cmdmx.json`.
      - Leave the Show screen and come back. The list shows Toekomst.
      - Open it. The warning says it was made by a newer CMDMX, and Kerk stays open.

## Rename, delete and reveal (SHW-22)
- [ ] Select a show that is not open, type a new name next to *Rename*, click *Rename*. The list shows the new name;
      in Finder the file (and its `.bak`, if any) has it too.
- [ ] Select the open show and rename it. The status bar shows the new name at once, Undo still works, and the old
      file is gone from the folder.
- [ ] Select a show that is not open, click *Delete*: it reads "Delete <name> — it goes to the Trash"; click again. The
      list loses it and it is in the Trash (`<show-dir>/Trash/` with `--show-dir`).
- [ ] Delete the open show: refused, "<name> is open: open another show first".
- [ ] *Show in Finder* opens the shows folder with the file selected.

## A file changed outside CMDMX (SHW-23)
- [ ] With Kerk open, copy another show's file over `Kerk.cmdmx.json` in Finder or Terminal. Leave the Show screen and
      come back: a message says the file was changed outside CMDMX.
- [ ] Open Kerk: the copied show runs, and a message names `Kerk before open <date> <time>`, which holds the show that ran.
- [ ] Copy a file over Kerk again, then change the show (record a group). Within a few seconds a message says it was
      kept as `Kerk outside <date> <time>`; that file holds the copy, and `Kerk.cmdmx.json` holds the change.

## A report of the show (SHW-25)
- [ ] With Kerk open, click *Report* under the show's name: the browser opens `Kerk <date> <time>.html` from the *Reports*
      folder next to the shows, titled *Kerk — <date> <time>*.
- [ ] It holds Fixtures, Groups, Palettes and Playbacks in that order; a parked fixture says `yes`, a cue list lists its cues
      with their times, and a legend with `&` or `<` reads as typed.
- [ ] *File › Print* shows it on A4 portrait without cut-off columns; *Save as PDF* writes a PDF.
- [ ] Lock the console (the app hides the Show screen) and send `show.report` over IPC: the engine still answers the path.

## Taking over a show (WEB-36)
A walk on a show the reader did not program, with only the chapter *Taking over a show* open: *Demo* or a show someone
else made, in a temp `--show-dir`. Note what the chapter did not answer.

- [ ] Reads in five minutes.
- [ ] Which show is open, and open another, from the chapter's steps.
- [ ] What is on the handles and pages, and what a list holds (tap, View); what a group holds.
- [ ] Which lamp a fixture is (Patch, Highlight).
- [ ] Run it; a late change with Update.
- [ ] Make it dark five ways, one at a time, and find each by the chapter's order: DBO, Grand Master, the playback's
      fader, a group master on another page (`LOCKED`), an output off on the Output screen.
- [ ] What Undo reaches after another operator's edits; what to do about a forgotten lock code.

## Restart (SHW-10)
- [ ] Stop the daemon (Ctrl+C) and start it again with the same command. The app reconnects, the
      status bar says "Kerk", and 101 is still named `Links`.
- [ ] Stop the daemon, run `rm /tmp/cmdmx-shw-c/state.json`, and start it again. Demo opens.

## The file
- [ ] Open `/tmp/cmdmx-shw-c/Kerk.cmdmx.json` in a text editor. It is readable JSON with
      `schema_version` 1, `meta`, `settings`, `profiles`, `patch`, `groups` and `playbacks`.
