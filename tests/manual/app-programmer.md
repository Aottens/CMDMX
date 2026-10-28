# App programmer checklist (IPC-14..19, IPC-46, UI-31, PRG via the app) — template

No Swift test target exists (CLAUDE.md), so the app side of `specs/modules/ipc.md` IPC-15..17
is checked by hand. Copy to `tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: two terminals. Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn`.
App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`.
Optional: an Art-Net monitor (or Wireshark on `en0`, filter `artnet`) to watch universe 1.
Church rig addresses (IPC-14): washes 101–104 at 9/18/27/36 (Dimmer, Pan coarse+fine, Tilt,
Zoom, R G B, White), RGBW pars 201–208 at 45, 50, … 80 (Dimmer, R G B, White), RGB+Lime pars
211–216 at 85, 89, … 105 (R G B, Lime), Showtec RGB pars 221–226 at 109, 115, … 139 (R G B, Mode,
Speed, Strobe), spots 301–306 at 145–150.

## Windows (IPC-15)
- [ ] Fixtures window lists 1–8 and the church rig (101–104, 201–208, 211–216, 221–226, 301–306) with their profile
- [ ] Click 203, 201, 202: they turn blue with order numbers 1, 2, 3; click 201 again: it deselects (PRG-01)
- [ ] Groups window shows groups 1–4 with fixture counts 30 / 4 / 20 / 6; clicking group 2 selects 101–104 in order (PRG-02)
- [ ] Record, then click an empty group cell (or group 4): the group now holds the current selection (PRG-07)

## Attribute bar and wheels (IPC-16)
- [ ] Select 101: tabs I, P, C and B are available, the others greyed; pick P: wheels Pan and Tilt appear (PRG-17)
- [ ] Drag the Pan wheel up: its value turns red and rises; Art-Net channels 10/11 follow; the fixture gets a red strip (PRG-19/20)
- [ ] Hold Option while dragging: Pan moves in tiny steps (channel 11 only); Shift: big steps (PRG-21)
- [ ] Select 101 and 301, Fix +1 / Fix +1: only one fixture is outlined at a time; with 301 targeted the tab jumps to I (PRG-04/18)
- [ ] Select 201–206, Odd/Even: 201, 203, 205 outlined; Fix +1: 202, 204, 206; All: all six again (PRG-05/06)
- [ ] Select 201, turn Dimmer up, then click 202: the selection starts over with just 202; 201 keeps its value (PRG-03)
- [ ] Clear: the selection goes; Clear again: the programmer empties, red strips and red values go, the playbacks show through (PRG-29)

## Command line (IPC-17)
- [ ] Type `201 Thro 205 Not 203 @ 50` and press Return: 201, 202, 204, 205 selected and at 50 % on Art-Net (45, 50, 60, 65 ≈ 128, the RGBW par Dimmer channels) (PRG-08/09)
- [ ] Type `Group 4` + Return: the front spots are selected (PRG-08)
- [ ] Type `201 Banana` + Return: the message line shows the refusal; nothing changes (PRG-11)
- [ ] While typing in the field, letters and Space do not trigger anything else (IPC-17)

## Colour (IPC-18/19, VS2-06)
- [ ] Select group 3, type `@ 100`, pick C: buttons HSI, RGB and CMY appear (HSI blue) and wheels Hue, Saturation, White, Lime
- [ ] Saturation up to full, then Hue to 240°: all pars turn blue on Art-Net (RGBW par 201 at 45–49, RGB+Lime 211 at 85–88, Showtec 221 at 109–111)
- [ ] Click RGB, then CMY: the output does not change and the wheels show the same colour in that model (PRG-30)
- [ ] The White wheel only changes the RGBW pars (channel 49); the Lime wheel only the RGB+Lime pars (channel 88)
- [ ] Grand master to 0: the RGB+Lime and Showtec pars go fully dark, Lime included
- [ ] Clear, select 101, `@ 100`: the wash shows white (R, G, B at 255 on channels 14–16, White 0)

## Kelvin, CTO/CTB and auto-mix (IPC-46, UI-31, VS2-46)
Auto-mix needs an auto-mixing type: switch it on for the RGBW Par first (`app-profile-editor.md` H).
- [ ] Select 201, `@ 100`, pick C: the model switch shows HSI RGB / CMY K two by two; click K: one Kelvin
      wheel reading about "6500 K" (white); the White wheel has a "+AUTO" tag under its name
- [ ] With auto-mix on, Art-Net 45–49: White (49) carries the white, R G B (46–48) drop to what is left
- [ ] Turn the Kelvin wheel down towards 3200 K: the reading follows in steps of 10 K; R rises
      over B on Art-Net and White still takes the common part
- [ ] In the colour picker, two rows CTO and CTB (⅛ ¼ ½ Full) sit above the gel book: ½ CTO
      warms the reading (about 6500 → 3800 K); ½ CTB cools it back; works in HSI and RGB too
- [ ] With nothing selected, ½ CTO: the message line shows the refusal; with only 301 (a dimmer): the same
- [ ] Raise the White wheel by hand on an auto-mixing par: its value is the direct part, added on top
      of the mix on channel 49 (ENG-22)
- [ ] The RGB+Lime pars (no auto-mix) show no "+AUTO" tag on Lime

## Still works
- [ ] On-screen faders 1–8 and GM still drive channels 1–8 (IPC-09); a programmer value on 1–8 beats its fader (PRG-13)
- [ ] Stop and restart the daemon: banner, reconnect, the app rebuilds fixtures, groups and an empty programmer (IPC-08/10)
