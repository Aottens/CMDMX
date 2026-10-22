# Output checklist (OUT-03/04/06/09/20..24, 29..32) — template

Setup: Mac on the same subnet as an Art-Net/sACN monitor (e.g. sACNView, Art-Net
monitor app, or a node), Eurolite USB-DMX512 PRO MK2 on USB with a dimmer or LED
par on its XLR.

Since VS6-02 the output nodes are show data (OUT-13): there are no `--serial`,
`--artnet-target`, `--sacn-unicast` or `--sacn-priority` flags any more (OUT-32).
Until the Output screen exists (VS6-03), a node is edited over IPC with
`output.add` / `output.edit` / `output.remove` (Claude sends these during the run),
e.g. `{"v":0,"cmd":"output.edit","id":3,"port":"/dev/cu.usbserial-AQ01FA9F"}`.
The ids are in the snapshot's `outputs`; a new show has Art-Net #1, sACN #2 and
serial `auto` #3 on universe 1.

## Serial (Eurolite MK2)
- [ ] `ls /dev/cu.usbserial-*` shows the Eurolite (the owner's MK2: `/dev/cu.usbserial-AQ01FA9F`, product string "FT232R USB UART")
- [ ] Daemon started with the serial node on `auto`: one line `serial #3: no ENTTEC Pro interface found; FTDI "FT232R USB UART" at /dev/cu.usbserial-… not used (choose it on the Output screen …)`; the par stays dark, nothing is sent to the FTDI port (OUT-09/23)
- [ ] `output.edit` the serial node to the path from `ls`, with `--demo`: log `serial #3: universe 1 → /dev/cu.usbserial-…` and an ENTTEC Pro line, no `Degraded` lines; the par follows the breathing fader smoothly, no flicker, no stepping (OUT-20/31)
- [ ] An LED fixture held at a static level (e.g. 30 %) does not flicker; nothing else lights up (OUT-24)
- [ ] A fader dragged 0→100→0 % over ~8 s fades the LED fixture without flicker and without steps you would not see on Lightkey (OUT-24, ~33 Hz own timer)
- [ ] Quit and restart the daemon: the show still names the Eurolite's path and the par lights again without any edit (OUT-13/31)
- [ ] Start the daemon with the Eurolite unplugged: the node is `Degraded` "port not found" (one log line), the daemon keeps ticking; plug it in: within ~1 s it is Ok and the par follows (OUT-21/31)
- [ ] Restart with `--no-serial`: the par stays dark, the node shows `Disabled` "--no-serial", the show file still names the port (OUT-32)
- [ ] Unplug the Eurolite mid-run: one `Degraded` line, the daemon keeps ticking, Art-Net/sACN unaffected; replug: output resumes without restart (OUT-12)

## Art-Net and sACN
- [ ] Art-Net monitor shows universe 0:0:0 (line 1) packets at ~40 Hz with the same values (OUT-03)
- [ ] `output.edit` the Art-Net node's `target` to the monitor's IP (unicast); packets still arrive, log `art-net #1: universe 1 → 0:0:0 at <ip>` (OUT-04/30)
- [ ] Restart with `--no-artnet`; the monitor shows no more ColourmotionDMX Art-Net packets, sACN continues (OUT-32)
- [ ] sACN monitor shows universe 1, priority 100, source name "ColourmotionDMX" at ~40 Hz, same values as Art-Net (OUT-06/07/17)
- [ ] `output.edit` the sACN node's `priority` to 150; the monitor shows priority 150 with no gap in the sequence numbers (OUT-07/29)
- [ ] `output.edit` the sACN node's `unicast` to `127.0.0.1`; packets go to 127.0.0.1:5568 (OUT-06)
- [ ] Restart with `--no-sacn`; no more sACN packets, Art-Net continues (OUT-32)

## Several universes (P1-18)
- [ ] Patch a fixture on universe 2: one warning `Universe 2 now goes out on Art-Net 0:0:1 and sACN 2`; the monitors show Art-Net 0:0:1 and sACN 2 carrying its values while universe 1 keeps going without a dropped frame (OUT-26/29)
- [ ] `output.edit` universe 2's sACN node `enabled: false` and back to `true`: universe 2's sACN stops and resumes; Art-Net 0:0:1 and universe 1's nodes never drop a frame (sequence numbers run on) (OUT-29)
- [ ] Undo the patch add: universe 2's nodes are gone again and its packets stop (OUT-29, UNDO-04)
