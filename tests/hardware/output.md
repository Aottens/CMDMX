# Output checklist (OUT-03/04/06/09) — template

Setup: Mac on the same subnet as an Art-Net/sACN monitor (e.g. sACNView, Art-Net
monitor app, or a node), Eurolite USB-DMX512 PRO MK2 on USB with a dimmer or LED
par on its XLR.

- [ ] `ls /dev/cu.usbserial-*` shows the Eurolite; `system_profiler SPUSBDataType` shows product string "Eurolite DMX512 Pro"
- [ ] Daemon log reports the interface as **ENTTEC Pro**, not Open DMX (OUT-09)
- [ ] With the demo fader breathing, the par follows smoothly; no flicker, no stepping
- [ ] Art-Net monitor shows universe 0 (line 1) packets at ~40 Hz with the same values (OUT-03)
- [ ] Restart the daemon with `--artnet-target <monitor-ip>` (unicast); packets still arrive (OUT-04)
- [ ] Restart with `--no-artnet`; the monitor shows no more ColourmotionDMX packets (OUT-15)
- [ ] sACN monitor shows universe 1, priority 100, source name "ColourmotionDMX" at ~40 Hz, same values as Art-Net (OUT-06/07/17)
- [ ] Restart with `--sacn-priority 150`; the monitor shows priority 150 (OUT-07/16)
- [ ] Restart with `--sacn-unicast 127.0.0.1`; packets go to 127.0.0.1:5568 (OUT-06/16)
- [ ] Restart with `--no-sacn`; no more sACN packets, Art-Net continues (OUT-16)
- [ ] Unplug the Eurolite mid-run: daemon logs one `Degraded` line, keeps ticking, Art-Net/sACN unaffected; replug: output resumes without restart (OUT-12)
