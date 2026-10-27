# App undo checklist (UNDO-18..20) — template

No Swift test target exists (CLAUDE.md), so the Undo and Redo keys, softkey G, ⌘Z / ⇧⌘Z and the
messages of `specs/modules/undo.md` UNDO-18..19 are checked by hand; UNDO-20 lists what this run
walks. The engine side (what a step is, what comes back) is covered by the VS5-01/02 tests; this
run checks that the app shows and drives it. Copy to `tests/manual/runs/<date>-<task>.md` and
tick it there.

Setup: two terminals, and a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-undo`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-undo`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`
  (or the copy in `~/Applications` when Claude drives the run).
- Demo show: church rig (washes 101–104, pars 201–226, front spots 301–306), demo playbacks on
  pages 1 and 2.

Output cannot be seen without lamps: Claude listens to the daemon's Art-Net on universe 1 during
the run and reports levels where a step says so. The second client is Claude's own WebSocket on
127.0.0.1:7319.

Who does what (owner, 2026-10-22): **[Claude]** steps Claude does by computer use and IPC;
**[Jij]** steps the owner does by hand, one per message.

## The keys at rest (UNDO-18)
- [ ] [Claude] Fresh start: the programmer summary shows **Undo** and **Redo** next to Clear, both
      disabled; softkey G in the Main context reads "Undo" over "nothing" and is disabled. ⌘Z and
      ⇧⌘Z: "Nothing to undo" / "Nothing to redo" in the message zone, nothing changes.

## Program, Clear twice, Undo twice (UNDO-01, 02, 18, 19)
- [ ] [Jij] Type `Fixture 201 Thro 204 @ 80 Enter`. Hover Undo: the tooltip names the step; softkey
      G shows the same label under "Undo". Clear: the programmer empties, Undo now reads "Clear".
      Clear again: the selection goes (or nothing happens if Clear already took both; note which).
- [ ] [Jij] Press Undo (the key): the message zone reads "Undone: Clear", the selection or values
      are back. ⌘Z: "Undone: …" again, 201–204 at 80 % in the programmer with the selection.
      Claude reads 201–204 at 80 %.
- [ ] [Claude] Redo is enabled and labelled; ⇧⌘Z: "Redone: Clear", the programmer empties again.
      ⌘Z brings it back. Clear (a new step): Redo is disabled again (UNDO-07).

## Record over a palette, Undo (PAL)
- [ ] [Claude] Pars 201–204 to red, Record, tap colour palette 1 (Replace): it now holds red.
- [ ] [Jij] Undo (softkey G this time, Main context): "Undone: Record palette …", palette 1 holds
      what it held before (its colour swatch is back). Redo puts red back; Undo again.

## Delete a playback, Undo (PBK-14, UNDO-10)
- [ ] [Jij] Delete a demo playback on page 1 (Delete, tap its handle, confirm): the strip goes.
      Undo: "Undone: Delete playback 1.x", the strip is back on its handle, not running, with its
      legend and options.

## Patch move, Undo (PAT, UNDO-12)
- [ ] [Claude] Patch screen: move par 226 to 1.400. Claude reads output at 1.400 when 226 is up.
- [ ] [Jij] Still on the Patch screen, click in a text field (e.g. the address field) and type,
      then ⌘Z: the text field undoes its typing, the engine does not undo. Click outside the field,
      ⌘Z: "Undone: Patch move 226", 226 is back on its old address in the table.

## Live actions are not undone (UNDO-04, 09)
- [ ] [Claude] Make an edit (Fixture 301 @ 50 Enter). Then raise a demo fader and press GO on a
      list: Undo still reads the edit's label.
- [ ] [Jij] Undo: the programmer edit goes; the fader stays up and the list stays on its cue
      (Claude reads the playback's output unchanged).

## Console mode (UNDO-18)
- [ ] [Jij] In console mode (full screen), make an edit and press ⌘Z: it undoes. ⇧⌘Z redoes.

## A second client (UNDO-15)
- [ ] [Claude] Make an edit in the app. Claude sends `{"cmd":"undo"}` over its own connection:
      the app's programmer follows, its Undo/Redo labels update (no message in the app: the
      message belongs to the client that pressed Undo).

## Daemon restart empties the history (UNDO-13)
- [ ] [Claude] Make an edit; Undo is enabled. Stop the daemon (Ctrl-C) and start it again with
      the same command: the app reconnects, Undo and Redo are disabled, softkey G reads "nothing".
      The edit is still in the show (autosave) but cannot be undone.
