# Church checklist (PAT-11, VS2-13, VS4-01) — template

The checklist for running a Sunday service on ColourmotionDMX, with Lightkey as fallback on the
same Mac (DECISIONS 2026-10-27). Only the owner can run it; copy it to
`tests/hardware/runs/<date>-church.md` and tick it there. The service run itself is VS4-03.
Never run the Demo show on the church rig (its fixtures on 9..150 light real lamps).

Live on the rig (2026-10-25 run): 101..118, four of 301..308, 401..406 (1 kW fresnels on dimmers),
501..502, 901..904 (Shehds 19x15, part of the steady look, no movement), 1001..1004. Parked in
Lightkey: 119, 120, the other four Lumipars, 601..602, 701 hazer, 801..802, 1201. Not hung: 201..204,
1101..1102. Steps below use only the live fixtures; ask the owner before adding others.

## Setup
- Show *Kerk* made once by `cargo run -p cmdmx-daemon --example church -- ~/Documents/ColourmotionDMX/Lightkey/BGS_Shows.tsv`
  (PAT-15..18; writes `~/Documents/ColourmotionDMX/Shows/Kerk.cmdmx.json` and the user profiles, never overwrites)
- At home (Art-Net, no lamps): daemon with `--show-dir` on a copy holding `state.json`
  `{"last_show":"Kerk"}`, so it starts on *Kerk* and not on Demo (SHW-10)
- At the church: ColourmotionDMX installed on the church Mac from the versioned DMG (VS4-07); the
  Eurolite is chosen on the Output screen and stored in the show (OUT-31/37)
- Lightkey stays installed on the same Mac as fallback (quit it before the daemon takes the Eurolite;
  the port is exclusive)

## Patch (as written by the one-off)
| Number | Name | Profile | Mode | Address | Footprint |
| --- | --- | --- | --- | --- | --- |
| 101 | P1 | Showtec/LED Par 56 | 6 Channel | 1.1 | 6 |
| 102 | P2 | Showtec/LED Par 56 | 6 Channel | 1.7 | 6 |
| 103 | P3 | Showtec/LED Par 56 | 6 Channel | 1.13 | 6 |
| 104 | P4 | Showtec/LED Par 56 | 6 Channel | 1.19 | 6 |
| 105 | P5 | Showtec/LED Par 56 | 6 Channel | 1.25 | 6 |
| 106 | P6 | Showtec/LED Par 56 | 6 Channel | 1.31 | 6 |
| 107 | P7 | Showtec/LED Par 56 | 6 Channel | 1.37 | 6 |
| 108 | P8 | Showtec/LED Par 56 | 6 Channel | 1.43 | 6 |
| 109 | P9 | Showtec/LED Par 56 | 6 Channel | 1.49 | 6 |
| 110 | P10 | Showtec/LED Par 56 | 6 Channel | 1.55 | 6 |
| 111 | P11 | Showtec/LED Par 56 | 6 Channel | 1.61 | 6 |
| 112 | P12 | Showtec/LED Par 56 | 6 Channel | 1.67 | 6 |
| 113 | P13 | Showtec/LED Par 56 | 6 Channel | 1.73 | 6 |
| 114 | P14 | Showtec/LED Par 56 | 6 Channel | 1.79 | 6 |
| 115 | P15 | Showtec/LED Par 56 | 6 Channel | 1.85 | 6 |
| 116 | P16 | Showtec/LED Par 56 | 6 Channel | 1.91 | 6 |
| 117 | P17 | Showtec/LED Par 56 | 6 Channel | 1.97 | 6 |
| 118 | P18 | Showtec/LED Par 56 | 6 Channel | 1.103 | 6 |
| 119 | P35 | Showtec/LED Par 56 | 6 Channel | 1.250 | 6 |
| 120 | P36 | Showtec/LED Par 56 | 6 Channel | 1.256 | 6 |
| 201 | MH1 | Showtec/Phantom 25 LED Spot | 11 Channel | 1.109 | 11 |
| 202 | MH2 | Showtec/Phantom 25 LED Spot | 11 Channel | 1.120 | 11 |
| 203 | MH3 | Showtec/Phantom 25 LED Spot | 11 Channel | 1.131 | 11 |
| 204 | MH4 | Showtec/Phantom 25 LED Spot | 11 Channel | 1.142 | 11 |
| 301 | P19 | Pro-Lights/Lumipar6UQ | 4 Channel | 1.153 | 4 |
| 302 | P20 | Pro-Lights/Lumipar6UQ | 4 Channel | 1.157 | 4 |
| 303 | P21 | Pro-Lights/Lumipar6UQ | 4 Channel | 1.161 | 4 |
| 304 | P22 | Pro-Lights/Lumipar6UQ | 4 Channel | 1.165 | 4 |
| 305 | P23 | Pro-Lights/Lumipar6UQ | 4 Channel | 1.169 | 4 |
| 306 | P24 | Pro-Lights/Lumipar6UQ | 4 Channel | 1.173 | 4 |
| 307 | P25 | Pro-Lights/Lumipar6UQ | 4 Channel | 1.177 | 4 |
| 308 | P26 | Pro-Lights/Lumipar6UQ | 4 Channel | 1.181 | 4 |
| 401 | P27 | Generic/PAR Spot | 1 Channel | 1.185 | 1 |
| 402 | P28 | Generic/PAR Spot | 1 Channel | 1.186 | 1 |
| 403 | P29 | Generic/PAR Spot | 1 Channel | 1.187 | 1 |
| 404 | P30 | Generic/PAR Spot | 1 Channel | 1.188 | 1 |
| 405 | P31 | Generic/PAR Spot | 1 Channel | 1.189 | 1 |
| 406 | P32 | Generic/PAR Spot | 1 Channel | 1.190 | 1 |
| 501 | P33 | Showtec/Compact Par 7 Tri | 3 Channel | 1.191 | 3 |
| 502 | P34 | Showtec/Compact Par 7 Tri | 3 Channel | 1.194 | 3 |
| 601 | P43 | Generic/PAR Wash (RGBW) | 4 Channel | 1.217 | 4 |
| 602 | P44 | Generic/PAR Wash (RGBW) | 4 Channel | 1.221 | 4 |
| 701 | F1 | Stairville/Hz-200 DMX | Default | 1.232 | 2 |
| 801 | P40 | beamZ/BTF50Z Mini Fresnel | 3 Channel | 1.234 | 3 |
| 802 | P41 | beamZ/BTF50Z Mini Fresnel | 3 Channel | 1.237 | 3 |
| 901 | MH5 | Shehds/LED Beam+Wash 19x15W RGBW Zoom Lighting | 24 Channel | 1.265 | 24 |
| 902 | MH6 | Shehds/LED Beam+Wash 19x15W RGBW Zoom Lighting | 24 Channel | 1.289 | 24 |
| 903 | MH7 | Shehds/LED Beam+Wash 19x15W RGBW Zoom Lighting | 24 Channel | 1.313 | 24 |
| 904 | MH8 | Shehds/LED Beam+Wash 19x15W RGBW Zoom Lighting | 24 Channel | 1.337 | 24 |
| 1001 | P37 | Generic/PAR Spot (RGBL) | 4 Channel | 1.361 | 4 |
| 1002 | P38 | Generic/PAR Spot (RGBL) | 4 Channel | 1.365 | 4 |
| 1003 | P39 | Generic/PAR Spot (RGBL) | 4 Channel | 1.369 | 4 |
| 1004 | P45 | Generic/PAR Spot (RGBL) | 4 Channel | 1.373 | 4 |
| 1101 | MH9 | beamZ/Fuze 75S Spot | 16 Bit | 1.464 | 13 |
| 1102 | MH10 | beamZ/Fuze 75S Spot | 16 Bit | 1.477 | 13 |
| 1201 | P42 | Generic/PAR Spot (RGB) | 3 Channel | 1.490 | 3 |

## On the desk (dimmers 185..190, RGBL par at 361..364)
- [ ] The Patch screen lists 56 fixtures, 101..1201, no clash warnings when *Kerk* opens
- [ ] `401 Thro 406 Enter`, Dimmer up: the six dimmers on 185..190 follow, in order 401 = 185
- [ ] `1001 Enter`: Red, Green and Blue light the right emitters, Lime drives the 4th channel (364) (PAT-16, Lightkey says Lime)
- [ ] With 401..406 fading on a fader, the dimmers fade smoothly without steps (OUT-20, on the right fixtures)
- [ ] Unplug the Eurolite mid-run: one `Degraded` line, ticking goes on; replug: output resumes (OUT-12)

## At the church (each type once)
- [ ] 101..120 Showtec LED Par 56: colour from the HSI wheel; 120 is the par at 256
- [ ] 201..204 Phantom 25: with Dimmer up the beam is on (Shutter defaults to open, PAT-16); Pan/Tilt move; Colour Wheel and Gobo channels step through the wheel
- [ ] 201..204 Phantom 25 slots (typed from the Highlite manual 40185 in the VS2-26 run, `tests/manual/runs/2026-10-24-VS2-26.md`): with Test in the profile editor each Colour Wheel and Gobo slot shows the named colour or gobo on the real lamp; correct the slot colours (nominal sRGB guesses) to what the lamp shows, and set the Pan/Tilt defaults if wanted
- [ ] 301..308 Lumipar6UQ: RGB + White
- [ ] 501..502 Compact Par 7 Tri: RGB
- [ ] 601..602 PAR Wash (RGBW): RGB + White
- [ ] 701 Hz-200 hazer: Haze and Fan Speed respond (only when hazing is allowed)
- [ ] 801..802 BTF50Z: Dimmer, Warm White, Cold White
- [ ] 901..904 Shehds: Zoom (channel 6) zooms the beam; Pan/Tilt; Colour drives all three heads
- [ ] 1001..1004 PAR Spot (RGBL); 1004 (P45) is the par at 373
- [ ] 1101..1102 Fuze 75S: beam on with Dimmer up; 16-bit Pan/Tilt smooth; Focus focuses
- [ ] 1201 PAR Spot (RGB)
- [ ] Back to Lightkey works (fallback ready)
- [ ] The iPad's TouchOSC drives Lightkey as before (settings restored after the VS4-04 run at home)
- [ ] TouchOSC with the ColourmotionDMX layout on the church wifi: faders follow without the lag
      felt on the home wifi (VS4-04 run, ping 32 ms average, peaks 84 ms)

## The service
A normal Sunday: songs, a welcome by the service leader, speaker and band taking turns, now and
then a video. No movement, no haze. The steady look ("sfeer") stays on all service; only the front
light changes.

Front fresnels (owner fills in once, then it is the same every week):
- Band front (4): ________ (of 401..406)
- Speaker front: ________ (of 401..406)

### Programmed before Sunday (at home on Art-Net, in *Kerk*)
- [ ] One or more steady looks on the stage: 101..118, the live Lumipars, 501..502, 901..904
      (fixed position) and 1001..1004; one look per theme or beamer background colour, each with a
      legend that says which (e.g. "Sfeer blauw")
- [ ] No steady look holds any of 401..406, so front light only comes from the front playbacks
- [ ] Playback *Front band* (the four band fresnels) and playback *Front spreker* on their own
      faders on page 1, with legends
- [ ] Switching from one steady look to another is one action (a Go or a fader) and fades, no snap
- [ ] The show is saved and copied to the church Mac; it opens there with the same playbacks

### Before the service (on the church Mac, ~30 min before)
- [ ] Lightkey quit; ColourmotionDMX started; it opens *Kerk* (not Demo); the Output dot is green
- [ ] The steady look of today up: every live fixture of it lights; nothing from 119/120 or other
      parked fixtures
- [ ] *Front band* up and down, *Front spreker* up and down: only their fresnels follow, smoothly
- [ ] Fallback rehearsed: quit ColourmotionDMX, start Lightkey, it lights the rig. Time from quit to
      light: ____ s. Then Lightkey quit and ColourmotionDMX started again, output back
- [ ] Grand Master at full, DBO off

### During the service
- [ ] Songs: steady look on, *Front band* full
- [ ] Welcome by the service leader: *Front band* out, *Front spreker* in; the steady look does not move
- [ ] Back to the band: *Front spreker* out, *Front band* in
- [ ] Speaker again: as the welcome
- [ ] Video (if any): both front faders out, the steady look stays; front back in afterwards
- [ ] Front-out: with both front faders at 0, all of 401..406 are dark (nothing left from a look)
- [ ] A change of steady look (if the theme asks for it) fades without a snap or flicker
- [ ] No flicker or steps seen all service; no warning in the message line
- [ ] If anything fails: fallback to Lightkey, and write down when and what: ________

### Not a standard Sunday: a theatre piece
- [ ] Single fresnels on their own: `401 Enter` (or another of 401..406), Dimmer up in the
      programmer, on top of the playbacks; Clear lets go of them

### After the service
- [ ] All faders down, ColourmotionDMX quit; Lightkey ready for whoever runs the next service
- [ ] Anything that felt slow or awkward written under *Open points* of the run file

