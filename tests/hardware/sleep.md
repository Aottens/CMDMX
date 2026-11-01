# Sleep checklist — a real sleep during a running fade (SPF-03, UI-35, OUT-38, 41)

Gate 3: only the owner ticks these. Copy to `tests/hardware/runs/<date>-SPF-03.md` for a run.

OUT-41 expects the fade clock to stand still during sleep, so a fade continues where it stopped. This run
confirms that or the criterion is amended. Fill in the measured values: they are the result.

Setup at home (never the real rig):
- Eurolite on USB, chosen on the Output screen, a dimmer on 1.185 on its XLR; Art-Net on as well.
- Mac on mains. Note whether the network is Ethernet or Wi-Fi: ______
- Lightkey quit (it holds the serial port and sends Art-Net itself).
- The bundled app from `scripts/bundle-app.sh`, engine with `--show-dir /tmp/…`, so the real shows stay
  untouched. A show with that dimmer and a playback with one cue: dimmer to 100 % in a **60 s** fade.
- Claude starts this listener in the background and reads its log after the wake. It uses the wall clock,
  which keeps counting during sleep:

```python
import socket, time
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEPORT, 1)
s.bind(("", 6454))
last = time.time(); shown = 0.0
while True:
    p = s.recv(600); t = time.time()
    if p[:8] != b"Art-Net\0" or p[8:10] != b"\x00\x50" or p[14] != 0: continue
    if t - last > 1: print(f"{time.strftime('%X')} GAP {t - last:.1f} s", flush=True)
    last = t
    if t - shown >= 1: print(f"{time.strftime('%X')} ch185={p[18 + 184]}", flush=True); shown = t
```

## Display (UI-35)
- [ ] Console on: `pmset -g assertions` lists `CMDMX console mode` under the app's pid as a display-sleep
      assertion, and `CMDMX DMX output` under the engine's pid (OUT-38)
- [ ] Console on, *Turn display off* set to 1 minute in System Settings, hands off for 2 minutes: the display
      stays on
- [ ] Console off: the first assertion is gone, `CMDMX DMX output` stays; hands off: the display goes off
      after a minute, the Mac stays awake, and the dimmer keeps its level
- [ ] Green button out of full screen and back: the assertion follows

## Sleep from the Apple menu during a running fade (OUT-41)
Note the wall clock at each step.
- [ ] Go on the playback at ______ ; about 20 s in, at ch185 = ______ , choose Sleep in the Apple menu at ______
- [ ] During sleep (30 s), the Eurolite's LEDs do: ______ ; the dimmer does: ______
- [ ] Wake at ______ . Without any action the dimmer follows again after ______ s, at level: ______
- [ ] Art-Net: the listener's gap is ______ s (sleep plus return of the network), first ch185 after it = ______
- [ ] The fade continues from where it stopped (first value after the gap about equal to the last value before
      it, full 40 s later), or else it jumped to ______ : amend OUT-41
- [ ] Output screen: Art-Net, sACN and the Eurolite are all Ok, no action was needed (OUT-12, OUT-21); messages
      during the return: ______
- [ ] The app is connected without a restart, and the frame rate after the wake is normal (no burst, OUT-40)

## Closed lid without an external display (OUT-41; skip on a desktop Mac)
- [ ] Same fade, lid closed for 30 s: the Mac sleeps despite the assertion; during sleep the dimmer does: ______
- [ ] Lid open: output returns without any action, the gap is ______ s

## After the run
- [ ] The manual (`running.md`, *The Mac stays awake*) and README say what the lamps do during sleep and what
      a fade does, with the values above; OUT-41 amended if the clock did not stand still

## Open points
- none
