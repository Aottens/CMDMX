# Gap review, the stand-in (REL-26, 2026-11-05, at v0.5.0)

Someone who never saw the show takes over shortly before it starts. Findings are in `docs/research/everyday-gaps.md`, *Fourth review*.
Claude drove it: the app under background control, then full-screen control with a real mouse for the walk, a WebSocket client to program the show and to leave a master down, a listener on port 6454. The owner took no step.

Setup: `main` at v0.5.0, debug daemon with `--ipc-port`, `--show-dir` and `--profiles-dir` in a temp folder, `--no-sacn --no-serial`. A copy of the owner's *Kerk*, untouched (schema 6, 56 fixtures, no groups, palettes or playbacks), programmed over IPC: six groups, two without a legend; palettes Warm, Blauw, Rood; list Dienst on 1.1 with six cues, two with a legend; memories 1.2 and 1.3, a chase on 1.4, master Zaal on 1.8; page 2 with 2.1 and 2.2. Debug bundle with the same port, `-consoleMode NO -keysLive NO`.

## Walk
- [x] First look: groups, palettes and strips on screen. `Group 5`, `1.3` and most cues say nothing about what they do (L4). Softkey Undo reads "Edit cue" (L7).
- [x] Group Fresnels: "6 selected", no selected tile in sight on page 1 or 2 of the Fixtures window (L2). Tiles read `P29`, `MH6`; Patch has the types (L3).
- [x] Tap on 1.2, View: Q1 holds 403 and 404 at 100 %. Clear; the notice "Group 3 Fresnels" stays (L8).
- [x] Dienst to 100 %: Q1. Go to Q2 (3 s, channel 187 rising to 204), Q3, Q4 (187 at 255).
- [x] `403 @ 60` Enter, Update: softkey Dienst · Q4 ticked, Store. Channel 187 at 153, kept after two Clears.
- [x] DBO: amber in the status bar and on the key, channel 187 at 0; off again, 153.
- [x] Lock from the status bar: the prompt. Cancelled. The manual has the master code.
- [x] Master Zaal to 0 % and Disabled: pars at 0, strip `LOCKED 0%`, nothing on page 2 (L1). Active Playbacks listed only Dienst earlier, with the master at 100 %.
- [x] Show screen: files dropped in the folder appear. `Toekomst`, schema 99: "made by a newer CMDMX (schema 99; this one reads up to 11)", Kerk keeps running. `Gast`, after two clicks: all at 0, list on Q1 waiting, master still `LOCKED 0%`, no message (L1).
- [x] Help menu: "Help isn't available for CMDMX." (L6).

## Not run
- Highlight on the stage: the listener read only channels 1 to 60 at that moment, so the key's amber look is all that was seen.
- Flash, Swop, the chase running, and a long list.

## Open points
- The findings L1 to L8, with their proposals → `docs/research/everyday-gaps.md`
