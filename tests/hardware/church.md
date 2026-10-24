# Church checklist (PAT-11, VS2-13; VS4-01 grows it into the service checklist) — template

Only the owner can run this; copy it to `tests/hardware/runs/<date>-church.md` and tick it there.
Never run the Demo show on the church rig (its fixtures on 9..150 light real lamps).

## Setup
- Show *Kerk* made once by `cargo run -p cmdmx-daemon --example church -- ~/Documents/ColourmotionDMX/Lightkey/BGS_Shows.tsv`
  (PAT-15..18; writes `~/Documents/ColourmotionDMX/Shows/Kerk.cmdmx.json` and the user profiles, never overwrites)
- Daemon with the Eurolite: `cargo run -p cmdmx-daemon -- --serial /dev/cu.usbserial-AQ01FA9F`, then open *Kerk* on the Show screen
- Lightkey ready on the same Mac as fallback (quit it before the daemon takes the Eurolite)

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
