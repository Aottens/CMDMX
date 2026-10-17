# App programmer checklist (IPC-14..17, PRG via the app) — template

No Swift test target exists (CLAUDE.md), so the app side of `specs/modules/ipc.md` IPC-15..17
is checked by hand. Copy to `tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: two terminals. Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn`.
App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app`.
Optional: an Art-Net monitor (or Wireshark on `en0`, filter `artnet`) to watch universe 1.
Church rig addresses (IPC-14): washes 101–104 at 9/14/19/24 (Dimmer, Pan coarse+fine, Tilt,
Zoom), pars 201–208 at 29–36, pars 221–226 at 37–42, spots 301–306 at 43–48.

## Windows (IPC-15)
- [ ] Fixtures window lists 1–8 and the church rig (101–104, 201–208, 221–226, 301–306) with their profile
- [ ] Click 203, 201, 202: they turn blue with order numbers 1, 2, 3; click 201 again: it deselects (PRG-01)
- [ ] Groups window shows groups 1–4 with fixture counts 24 / 4 / 14 / 6; clicking group 2 selects 101–104 in order (PRG-02)
- [ ] Record, then click an empty group cell (or group 4): the group now holds the current selection (PRG-07)

## Attribute bar and wheels (IPC-16)
- [ ] Select 101: tabs I, P and B are available, the others greyed; pick P: wheels Pan and Tilt appear (PRG-17)
- [ ] Drag the Pan wheel up: its value turns red and rises; Art-Net channels 10/11 follow; the fixture gets a red strip (PRG-19/20)
- [ ] Hold Option while dragging: Pan moves in tiny steps (channel 11 only); Shift: big steps (PRG-21)
- [ ] Select 101 and 301, Fix +1 / Fix +1: only one fixture is outlined at a time; with 301 targeted the tab jumps to I (PRG-04/18)
- [ ] Select 201–206, Odd/Even: 201, 203, 205 outlined; Fix +1: 202, 204, 206; All: all six again (PRG-05/06)
- [ ] Select 201, turn Dimmer up, then click 202: the selection starts over with just 202; 201 keeps its value (PRG-03)
- [ ] Clear: the selection goes; Clear again: the programmer empties, red strips and red values go, the playbacks show through (PRG-29)

## Command line (IPC-17)
- [ ] Type `201 Thro 205 Not 203 @ 50` and press Return: 201, 202, 204, 205 selected and at 50 % on Art-Net (29, 30, 32, 33 ≈ 128) (PRG-08/09)
- [ ] Type `Group 4` + Return: the front spots are selected (PRG-08)
- [ ] Type `201 Banana` + Return: the message line shows the refusal; nothing changes (PRG-11)
- [ ] While typing in the field, letters and Space do not trigger anything else (IPC-17)

## Still works
- [ ] On-screen faders 1–8 and GM still drive channels 1–8 (IPC-09); a programmer value on 1–8 beats its fader (PRG-13)
- [ ] Stop and restart the daemon: banner, reconnect, the app rebuilds fixtures, groups and an empty programmer (IPC-08/10)
