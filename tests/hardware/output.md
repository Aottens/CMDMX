# Output checklist (OUT-03/04/06/09/20..23) — template

Setup: Mac on the same subnet as an Art-Net/sACN monitor (e.g. sACNView, Art-Net
monitor app, or a node), Eurolite USB-DMX512 PRO MK2 on USB with a dimmer or LED
par on its XLR.

- [ ] `ls /dev/cu.usbserial-*` shows the Eurolite (the owner's MK2: `/dev/cu.usbserial-AQ01FA9F`, product string "FT232R USB UART")
- [ ] Daemon started without `--serial`: one line `serial: no ENTTEC Pro interface found; FTDI "FT232R USB UART" at /dev/cu.usbserial-… not used (start with --serial …)`; the par stays dark, nothing is sent to the FTDI port (OUT-09/23)
- [ ] Restart with `--serial <path from ls> --demo`: log `serial: ENTTEC Pro "--serial" at <path>`, no `Degraded` lines; the par follows the breathing fader smoothly, no flicker, no stepping (OUT-20/22)
- [ ] An LED fixture held at a static level (e.g. 30 %) with `--serial` does not flicker; nothing else lights up (OUT-24)
- [ ] A fader dragged 0→100→0 % over ~8 s fades the LED fixture without flicker and without steps you would not see on Lightkey (OUT-24, ~33 Hz own timer)
- [ ] With `--serial <path>`, start the daemon with the Eurolite unplugged: one `Degraded` line, daemon keeps ticking; plug it in: within ~1 s `Ok again` and the par follows (OUT-21/22)
- [ ] Restart with `--no-serial`: the par stays dark (OUT-22)
- [ ] Art-Net monitor shows universe 0 (line 1) packets at ~40 Hz with the same values (OUT-03)
- [ ] Restart the daemon with `--artnet-target <monitor-ip>` (unicast); packets still arrive (OUT-04)
- [ ] Restart with `--no-artnet`; the monitor shows no more ColourmotionDMX packets (OUT-15)
- [ ] sACN monitor shows universe 1, priority 100, source name "ColourmotionDMX" at ~40 Hz, same values as Art-Net (OUT-06/07/17)
- [ ] Restart with `--sacn-priority 150`; the monitor shows priority 150 (OUT-07/16)
- [ ] Restart with `--sacn-unicast 127.0.0.1`; packets go to 127.0.0.1:5568 (OUT-06/16)
- [ ] Restart with `--no-sacn`; no more sACN packets, Art-Net continues (OUT-16)
- [ ] Unplug the Eurolite mid-run (with `--serial <path>`): daemon logs one `Degraded` line, keeps ticking, Art-Net/sACN unaffected; replug: output resumes without restart (OUT-12)
