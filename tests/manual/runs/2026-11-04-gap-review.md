# One operator's evening — gap review after v0.4.0 (2026-11-04)

The run behind the third review in `docs/research/everyday-gaps.md` (REL-26), at home without lamps. Claude drove the app with a real mouse and real keys by
full-screen control; the owner approved it. A listener on port 6454 read Art-Net.

Setup: `main` at v0.4.0 plus #235, debug bundle, app with `HOME` in a temp folder and `-consoleMode NO`, first `-keysLive NO`, then a restart with `-keysLive YES`, against a daemon with
`--show-dir`, `--profiles-dir` and `--qlcplus-dir` and `--no-sacn --no-serial`. No `--demo`: it lets fader 1 breathe, which would have spoiled the readings. Window full screen from the green button.

## Program from an empty show
- [x] Show, New `Dienst`: asks "output changes now"; an empty Patch, empty handles.
- [x] Search a dimmer: none to be had (K1). New from channel list: Generic, Dimmer, 1ch, one Dimmer channel; Save. Fields filled by Tab; clicks straight after typing lost the field, the tool's pace.
- [x] Patch: 6 dimmers at 1.1, 4 Generic RGBW (numbers 11 to 14, virtual dimmer) at 1.7, 2 Eurolite LED TMH-17 (21, 22) at 1.23. Adding with Number 11 still typed: refused (K10).
- [x] Groups by keypad and tiles, named by Set Legend: Front, Pars, Heads.
- [x] Colour palettes 1 and 2 from the picker, on Pars. Art-Net: 7, 8, 9 = 128, 104, 47.
- [x] With Keys off, `@ 50` and Return do nothing, also after a click on the line (K2). With Keys on: `@50`, Return gives Pars 50 %.
- [x] Record on 1.1, then Record, a tap on 1.1, Append twice: three cues. Q1 Pars warm 50, Front 30; Q2 Front 70; Q3 Pars blue full, Heads full.
- [x] Times through Edit Times, one cue at a time: Q1 3 s, Q2 5 s.

## Run it
- [x] Go with the fader at 0: Q1 is live, Art-Net has no change, "fader 0 %" in amber (K12).
- [x] Fader to 87 %, Go: Q2, Front from 58 to 155 over 5 s (70 % × 87 %). Pars stay: the list tracks.
- [x] Late change: `Group 1`, `@60`, Update, a tap on 1.1, Clear: Front at 133, no softkeys needed.
- [x] Space runs Q3, with Keys on. A further Go at the end does nothing.
- [x] Back: Q2 with its 5 s fade.
- [x] Fixture 3 `@100` recorded on 1.2. Flash: channel 3 to 255 and back to 133. Fader: 221 on top of the list.
- [x] `Group 3`, Locate: pan and tilt 128, dimmer 255 (strobe 0 is open on this head). LOC on the tiles; Clear ends it.
- [x] Faders of 1.1 and 1.2 to 0 and 1.1 up: Q2 still live, Front at 136.
- [x] `kill -9` of the app earlier, with a programmer: on return the programmer and selection were there.
- [x] `kill -9` of the daemon and a new one by hand, 26 s later: all channels 0, faders 0, list at the start, programmer empty; Messages "Engine offline", "Engine connected" (K7).
- [x] Fader up: Q1 fires with its 3 s. `Group 2`, F3, a new colour, Update on palette 1: channels 7, 8, 9 change at once, on the running cue, softkeys none.
- [x] A tap on 1.2, one cue: nothing opens (B1). Right-click on Q3, View Cue: Front 60 % tracked, colours unreadable in their cell (K8).
- [x] Page 2: the page buttons show no mark, the Go block stays on 1.1 (J7).
- [x] Release with the fader up: 18 channels to 0 in one frame (K6). Space: Q1 in 3 s.
- [x] Two Space presses 0.4 s apart at Q1: Q2 skipped, Q3 at once (K4). Legend `Preek` on Q2 by right-click.
- [x] `Group 3` then softkey All with an empty programmer: the selection empties, the softkeys change, the second tap arms Set Legend (K9). After Esc the hint is still there (IDEA-40).

## Not run
Console mode (`-consoleMode NO` holds it), a release build, lamps, the bundled engine's restart note (REL-25) and the takeover of IDEA-37 (a debug bundle holds no engine). Position palettes, effects, chases, Lock, DBO, the Grand Master and group masters were not part of this evening.
The show file and Art-Net log stayed in a temp folder.

## Open points
- The third review's points K1 to K12 and the batch proposals → IDEA-35
- A refused number stays in its field: not re-run; proposed in batch B → IDEA-36
- The first screen had the strips cut off with the Dock → IDEA-39
- A hint stays after Esc and through an engine restart → IDEA-40
