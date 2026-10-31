# Selection orders checklist (PRG-71..83, UI-33/34) — template

The selection orders home run (SEL-07), checked by hand because there is no Swift test target
(CLAUDE.md). Copy to `tests/manual/runs/<date>-SEL-07.md` and tick it there. Claude drives the
setup steps and listens to the Art-Net frames; the owner does the learning steps in the app and
says what feels wrong. Each step message shows done/total.

Setup: two terminals and a fresh shows folder so the real one stays untouched.
- `rm -rf /tmp/cmdmx-sel`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --no-serial --show-dir /tmp/cmdmx-sel`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`.
- Art-Net listener on universe 1. Demo rig: RGBW pars 201–208 (Dimmer, R, G, B). Every step
  uses those eight pars; "201 → 208" means the pars light one after the other in that order.

## Order on a chase (PRG-71, 72, 77, UI-33)
- [ ] Setup (Claude): `201 Thro 208 Enter`; the buttons show 1–8. Selection ▸ **Order** ▸
      **Reverse** (owner): the numbers read 8 … 1 on 201 … 208. Shapes ▸ **Dimmer chase**:
      Art-Net Dimmer 208 → 201, one par at a time.
- [ ] Stop FX. Order ▸ **Shuffle**, Dimmer chase again: the chase follows the shuffled numbers on
      the buttons (Claude: the Art-Net order equals the selection order). Shuffle again gives
      another order; **Revert** puts 1–8 back on 201–208, **Sort** keeps them there.

## Patterns with Next (PRG-73..75, UI-33)
- [ ] Stop FX, Clear the programmer, `201 Thro 208 Enter`. Selection ▸ **Pattern** ▸ **Odd/Even**:
      the tag reads `GRID ODD/EVEN · 1/2`. `@ 100 Enter`: Art-Net 201, 203, 205, 207 at 255.
      **Fix +1** (Next): tag `2/2`; `@ 30 Enter`: 202, 204, 206, 208 at 77.
- [ ] `3`, Pattern ▸ **1 in n**: tag `GRID 1 IN 3 · 1/3`, buttons show 1 2 3 1 2 3 1 2. `@ 100`;
      Next, `@ 60`; Next, `@ 20`: Art-Net Dimmer 255 153 51 255 153 51 255 153 on 201–208.
      Next once more wraps to `1/3`.

## Outside in and Inside out on Fan and a Dimmer chase (PRG-75..77)
- [ ] `201 Thro 208 @ 50 Enter`, Pattern ▸ **Outside in**, **All**: tag `GRID OUTSIDE IN · ALL`,
      buttons 1 2 3 4 4 3 2 1. Fan ▸ **Line**, turn the Dimmer wheel up: the ends and the
      middle move apart, mirrored (Claude: 201 = 208, 202 = 207, 203 = 206, 204 = 205, rising or
      falling from the ends to the middle).
- [ ] Fan off, Clear the programmer, `201 Thro 208 Enter`, Outside in, All, **Dimmer chase**:
      Art-Net 201+208, 202+207, 203+206, 204+205, again. Stop FX, **Inside out**, All, Dimmer
      chase: 204+205 first, outwards to 201+208.

## A group with its grid (PRG-79)
- [ ] Stop FX, Clear, `201 Thro 208 Enter`, Outside in; Record, tap an empty group: the group is
      stored. Clear, Clear. Tap that group: the tag reads `GRID OUTSIDE IN · ALL` and the buttons
      1 2 3 4 4 3 2 1; Dimmer chase runs from both ends inwards as before.

## A cue that replays the order (PRG-80, PBK-15)
- [ ] Setup (Claude): Stop FX, Clear; a cue list with cue 1 = 201–208 at 0 and cue 2 = 201–208 at
      100 recorded with Outside in on; cue 2 fade 1 s, overlap 0 %. Go cue 1, then Go cue 2:
      Art-Net 201+208 rise 0–1 s, 202+207 1–2 s, 203+206 2–3 s, 204+205 3–4 s.

## The cue's order edited (PRG-81, UI-34)
- [ ] Playback Options ▸ Times on cue 2 ▸ **Fixture order** shows steps 1 2 3 4 4 3 2 1.
      **Edit**; Auto increment on: tap 201 (1), 202 (2). Auto increment off: tap 203, 204 (both
      3); **Next step**; tap 205, 206 (both 4). **Done**: 207 and 208 follow after.
      Go cue 1, Go cue 2: Art-Net 201 first, then 202, then 203+204, then 205+206, then the rest.
- [ ] **Undo**: Fixture order reads 1 2 3 4 4 3 2 1 again; Go cue 1, Go cue 2 replays Outside in.
      An Edit closed with **Cancel** changes nothing.

Claude's background clicks do not fire the cells' tap gestures, so the owner taps the Fixture order cells.

## Open points
