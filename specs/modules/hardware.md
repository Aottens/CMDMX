# Module spec: hardware (HW)

Owner: `subprojects/hardware/` (PCBs, firmware, enclosure and flight-case CAD, BOM, build guide).
Status: **draft, waiting for owner approval (gate 1)**. Written 2026-10-25 from the owner's
answers to the eleven open questions of `docs/research/hardware-desk.md` (HW-01; answers under
*Owner decisions*). Phase 3, run as a parallel track beside phase 1 (DECISIONS 2026-10-25).

References: `docs/research/hardware-desk.md` (parts, prices, §5a fader strips);
`specs/modules/control-surfaces.md` (CTL, the general control layer this desk plays through);
`specs/modules/output-protocols.md` (OUT-09/31, serial ENTTEC Pro nodes); Avolites Tiger Touch II
(layout reference only; names are trademarks of their owners).

## Purpose
Open build plans for a real lighting desk that runs CMDMX: the size and feel of an
Avolites Tiger Touch II, one touch screen, motorised playback faders, four isolated DMX ports and
Art-Net/sACN on the network, built from parts a maker can buy, in quality a professional accepts.
The desk surface is a USB-MIDI device on the general control layer (CTL) and the DMX ports are
serial ENTTEC Pro nodes (OUT-31), so **the engine and the app do not change for the hardware**.
Every proof of concept is tested on the owner's Mac with today's app.

## Definitions
- **Surface:** everything the operator touches except the screen: fader strips, programmer panel,
  main controller. One USB-MIDI device towards the computer.
- **Strip:** a small PCB with two faders, their keys, LEDs and its own RP2350 MCU. Two designs:
  the **playback strip** (motorised faders, two keys per fader) and the **preset strip** (plain
  faders, one key per fader).
- **Main controller:** the PCB with the MCU that talks USB-MIDI to the computer and RS485 to the
  strips and the programmer panel.
- **Programmer panel:** keypad, programmer keys, attribute-group keys, macro keys and the four
  wheels.
- **DMX board:** a separate PCB with its own MCU and four isolated DMX outputs.
- **Wire map:** the fixed table of which MIDI message each physical control sends and receives
  (HW-20).
- **Reference desk:** the build the build guide describes; variants are documented options.

## Acceptance criteria

### Scope of the reference desk
- **HW-01** The reference desk is one console in the size class of the Tiger Touch II, with one
  15.6" 1080p capacitive (PCAP) touch screen, the surface (HW-10..19) and the DMX board (HW-40..44)
  built in. A second screen is an external monitor, not part of the reference build.
- **HW-02** Compute: the reference desk is a **Linux** build on an x86 mini-PC of the Intel N100/N150
  class (16 GB RAM, NVMe SSD, two display outputs, Ethernet), named as a class with minimum specs,
  not as one SKU. It must run the daemon with 16 universes (4 on the DMX ports, the rest on
  Art-Net/sACN) and the UI without dropping a 40 Hz frame. Until the cross-platform UI exists
  (IDEA-30) the desk is developed and tested on a Mac; the enclosure also fits a Mac mini so a
  Mac desk stays a documented variant.
- **HW-03** Quality before price: the reference BOM uses professional-grade parts (motorised
  Bourns PSM faders, optical encoders, isolated DMX). A cheaper part is only a documented variant
  when it fits the same footprint and connector; the BOM has no hard price cap and is published
  with indicative prices and their date.
- **HW-04** Network: Art-Net/sACN leave through the computer's Ethernet on a locking etherCON jack
  on the rear panel.

### Layout
- **HW-05** Playback section: 10 motorised playback faders, each with two RGB keys (bound to
  `upper` and `lower`, CTL-05), and a row of 10 plain preset faders, each with one RGB key. Fader
  pitch 33 mm, 100 mm travel.
- **HW-06** Grand master: one plain 100 mm fader, never motorised, with a DBO key next to it.
- **HW-07** Programmer panel: four smooth (non-detented) optical encoder wheels, each with a push
  key; a numeric keypad with the CTL-05 keypad tokens; the CTL-05 programmer keys; eight
  attribute-group keys (I P C G B E S FX); Go, Back, Stop, Release; Page ◀ and ▶; Fan; ten macro
  keys. Keys for functions the CTL-05 catalogue does not have yet (e.g. Record) are laid out and
  send their wire-map message, but stay unbound until a CTL criterion adds the action.
- **HW-08** All keys are MX-compatible RGB switches in hot-swap sockets with reverse-mount SK6812
  Mini-E LEDs under clear relegendable caps (paper legend insert).
- **HW-09** The fader count stays configuration: the surface never assumes `faders_per_page`; the
  profile (HW-21) binds strips to slots.

### Fader strips (§5a)
- **HW-10** Two strip PCB designs, each carrying two faders: the **playback strip** (two Bourns
  PSM motorised faders, motor drivers, capacitive touch sense, four RGB keys) and the **preset
  strip** (two plain 100 mm faders, two RGB keys). The reference desk has 5 of each.
- **HW-11** Every strip is smart: an RP2350 on the strip reads its faders (12-bit or better),
  runs the motor PID and touch sense (playback strip), scans its keys and drives its LEDs. Analog
  signals never leave the strip.
- **HW-12** Strips, the programmer panel and the main controller talk over **RS485** (half duplex,
  one transceiver per board) on one keyed, locking cable per board that carries 12 V, GND and the
  bus, daisy-chained. Each board makes its own 10 V (motors), 5 V (LEDs) and 3.3 V locally.
- **HW-13** Each strip has a position address set by a DIP switch or strap (strip 1 = faders 1–2,
  …); two strips with the same address are both refused by the main controller and named in its
  status output (HW-23).
- **HW-14** A still fader never sends: a value is sent only when it moved by more than a deadband
  (default 0.25 % of travel, set per strip in firmware), so motor faders and noise make no
  feedback loops.
- **HW-15** Full travel is exact: each fader is calibrated at both end stops (a stored per-fader
  calibration, re-run from the status console), so bottom sends exactly 0 and top exactly 16383.
- **HW-16** Touch: while a playback fader is touched its motor is off and incoming positions are
  ignored; on release the strip takes its own position as target until the next position arrives,
  so the fader never jumps back to a stale value.
- **HW-17** Motor protection: a motor that has not reached its target within 1 s is switched off
  until the next target arrives, and the strip reports a `motor stall` on fader n once.
- **HW-18** A strip powers up with motors off and sends no fader value until it has read its real
  fader positions; the first values it sends are those positions.
- **HW-19** LED dimming: one plain knob on the surface, read by the main controller, sets the
  brightness of all key LEDs locally from off to full. It sends nothing to the computer; a dark
  room never depends on the software.

### Main controller and USB-MIDI mapping on the general control layer
- **HW-20** The main controller is one class-compliant USB-MIDI device named `CMDMX Desk` (no
  driver on macOS or Linux), on the open-hardware USB vendor id of pid.codes (`0x1209`) with a PID
  requested for the project (prototypes use a pid.codes test PID). Its wire map, all on MIDI
  channel 1:

  | Control | Sends | Receives (feedback, CTL-17) |
  | --- | --- | --- |
  | Playback fader 1–10 | `cc14` CC 0–9 (LSB CC 32–41) | same `cc14`: motor target |
  | Preset fader 1–10 | `cc14` CC 10–19 (LSB CC 42–51) | ignored (no motor) |
  | Grand master | `cc14` CC 20 (LSB CC 52) | ignored |
  | Wheel 1–4 | relative CC 64–67, `twos_complement` (CTL-11) | ignored |
  | Playback key upper 1–10 / lower 1–10 | note 0–9 / note 10–19 | same note: LED, velocity = colour |
  | Preset key 1–10 | note 20–29 | same note: LED |
  | Macro key 1–10 | note 30–39 | same note: LED |
  | Wheel push 1–4 | note 40–43 | same note: LED |
  | DBO, Go, Back, Stop, Release, Page ◀, Page ▶, Fan | note 44–51 | same note: LED |
  | Attribute groups I P C G B E S FX | note 52–59 | same note: LED |
  | Keypad and programmer keys | note 60 upward, in the order of the table in `subprojects/hardware/firmware/wire-map.md` | same note: LED |

  Keys send note on (velocity 127) at press and note off at release. Feedback velocity selects a
  colour from a fixed palette published in `wire-map.md` (0 = off, 127 = white), so the profile
  sets colours with `on_value`/`off_value` (CTL-18).
- **HW-21** Proofs of concept are bound with Learn (CTL-25) and need no software change. When the
  main controller board works (HW-63), a software task adds a read-only `CMDMX Desk` profile
  (CTL-30) in `subprojects/engine/profiles/` with the default bindings of HW-05..07 and the wire
  map of HW-20, marked as motorised for the playback faders (CTL-13) and pickup for the preset
  faders and grand master. That task is a CTL criterion in a software PR, never a hardware PR.
- **HW-22** Wheels: the MCU decodes the optical encoder's quadrature and sends the accumulated
  steps once per USB frame (1 ms), clamped to ±63 per message, with a steps-per-turn divider set in
  firmware (default: one MIDI step per 4 quadrature counts).
- **HW-23** The main controller also presents a USB serial status console (composite device) that
  lists the strips and panel it found by address, their firmware version, stall and duplicate
  address reports, and runs fader calibration (HW-15). Nothing on it is needed to play a show.

### Show-proof rules
- **HW-30** Every MCU (main controller, each strip, the panel, the DMX board) runs its hardware
  watchdog (timeout ≤ 500 ms). A board that resets rejoins without disturbing the others and
  follows HW-18 again.
- **HW-31** A missing or silent strip (no answer on RS485 for 100 ms) is "no change": its controls
  send nothing, all other controls keep working, and the main controller reports it on the status
  console (HW-23); when it answers again it rejoins with HW-18.
- **HW-32** When the USB host is lost (cable out, computer crashed or rebooting) motors stay where
  they are and all key LEDs show a fixed "disconnected" pattern (dim red); nothing moves until the
  host is back and has sent feedback (CTL-16/21).
- **HW-33** Firmware follows the engine's show-proof rules: no panic paths in runtime code (Rust:
  the crate-level `deny` of `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`); errors are
  values, logged once on the status console; the scan loop never stops.
- **HW-34** Power: a locking mains inlet (Neutrik powerCON TRUE1), a Mean Well PSU with a 12 V
  internal rail, and a UPS that keeps the whole desk running long enough for a clean shutdown
  (Mean Well DRC-class charger with battery, or an external USB UPS for the Mac variant).
- **HW-35** Linux compute (when IDEA-30 makes the Linux build run): root filesystem on NVMe, never
  an SD card; read-only root with show files on a writable data partition; the hardware watchdog
  armed through systemd with daemon and UI restarted by systemd; power-loss signal from the UPS
  flushes the show and shuts down cleanly; the computer powers on by itself when mains returns.
- **HW-36** Every connector a cable can be kicked out of locks: powerCON, etherCON, Neutrik XLR,
  locking internal cables with strain relief for screen, USB and strip buses.

### DMX board
- **HW-40** A separate DMX board with its own RP2350 drives four DMX512 outputs on Neutrik 5-pin
  female XLRs, each galvanically isolated (ADM2587E class, ≥ 2.5 kV) with TVS protection on the
  data lines.
- **HW-41** Towards the computer it is one USB device with four serial (CDC ACM) ports, each
  speaking the ENTTEC DMX USB Pro protocol (label 6 send DMX; label 3 get widget parameters and
  label 10 serial number answered), with a USB product string that passes
  `enttec::is_pro_product_string` (`CMDMX DMX512 Pro`). The daemon uses its ports as four serial
  nodes (OUT-31), one per universe; no engine change.
- **HW-42** It is independent of the surface: its own USB connection to the computer and its own
  power regulation, so a hanging surface never touches DMX output.
- **HW-43** Output holds the last good frame: each port keeps sending its last received frame at
  a continuous rate (default 40 Hz, full DMX512 timing generated in hardware/PIO) when frames stop
  arriving; after power-up a port sends nothing until its first frame.
- **HW-44** A port whose frames stop does not affect the other three ports.

### Enclosure and flight case
- **HW-50** Desk body: birch ply sides and base with 2020 aluminium extrusion, 2–3 mm laser-cut
  aluminium front plates with engraved or UV-printed legends, the screen section tilted 15–25°,
  a removable bottom plate for service, and all rear connectors on one rear plate.
- **HW-51** The body is parametric in the number of strips, so a wider or narrower desk is a
  parameter change, not a redesign.
- **HW-52** The flight case is published as drawings (outer dimensions, foam plan) using standard
  Penn Elcom / Adam Hall hardware, so a case maker or a maker can build it.
- **HW-53** CAD is published as native files plus open exchange formats (STEP for parts, DXF for
  laser-cut plates); PCBs in KiCad.

### Licence
- **HW-55** PCBs, enclosure and case CAD and the BOM are licensed **CERN-OHL-S v2**; the build
  guide and photos **CC BY-SA 4.0**; firmware follows the software licence (IDEA-21). Each
  licence text is in `subprojects/hardware/` and every design file names its licence. The owner
  may sell PCBs and kits; so may others under the same licence.

### Prototype order
- **HW-60** Proofs of concept come in this order, each one tested on the owner's Mac with today's
  app before the next starts:
  1. **USB-MIDI box** (HW-61): firmware on a Raspberry Pi Pico 2 on a breadboard.
  2. **Playback strip** PCB and its firmware (motor, touch, RS485).
  3. **Main controller** PCB (USB-MIDI, RS485 master, status console).
  4. **Preset strip** PCB.
  5. **DMX board** PCB and firmware.
  6. **Programmer panel** PCB (keypad, keys, four wheels).
  7. **Enclosure** and front plates, then the **flight case**.
- **HW-61** The USB-MIDI box: one Pico 2 (RP2350) with two plain faders, four RGB keys and one
  optical encoder, enumerating as `CMDMX Desk` with the HW-20 wire map for those controls, HW-14
  deadband, HW-18 boot rule, HW-30 watchdog and HW-32 disconnected pattern. Bound with Learn on
  the Mac, it moves faders 1–2, presses `upper`/`lower` and turns wheel 1, and its key LEDs follow
  the engine's feedback.
- **HW-62** Each step ships a checklist in `tests/hardware/` (gate 3) that only the owner ticks.
- **HW-63** "Main controller works" (for HW-21) means step 3 passed its checklist with at least
  one playback strip connected.

### Firmware
- **HW-70** Firmware is written in Rust (embassy on the RP2350), one cargo workspace in
  `subprojects/hardware/firmware/`, separate from the engine workspace. Pure logic (deadband,
  calibration, encoder accumulation, wire map, RS485 framing, ENTTEC Pro framing) lives in a
  `no_std` crate that is unit-tested on the host with `cargo test`; tests carry the criterion id
  (`hw_14_still_fader_sends_nothing`).
- **HW-71** Hardware PRs never touch `subprojects/engine` or `subprojects/app`.

## Tests
- Firmware unit tests on the host (HW-70) for HW-14, 15, 16, 17, 18, 20, 22, 31, 41, 43.
- Owner checklists in `tests/hardware/` (gate 3) per prototype step (HW-62): box, playback strip,
  main controller, preset strip, DMX board (checked with an Art-Net/DMX monitor and a fixture),
  programmer panel, enclosure.

## Out of scope
The cross-platform UI (IDEA-30); a second built-in screen; a built-in Art-Net/sACN node; 1,000-nit
sunlight panels in the reference build (a documented option, same cut-out); desk lamps (XLR
littlites); scribble strips or displays per fader; DMX input; MIDI DIN ports on the desk.

## Owner decisions (HW-01, 2026-10-25)
1. Reference desk: Tiger Touch II size class, one touch screen, 4 physical DMX universes, the rest
   on Art-Net/sACN (the owner looked at Sapphire Touch and Quartz; one screen and Tiger Touch II
   size won). Must run several universes easily and without faults.
2. Linux is the reference compute (macOS needs a paid touch driver; no touch is not acceptable).
   Development and proofs of concept run on the Mac; the cross-platform UI comes later (IDEA-30).
3. Layout: 10 playback faders with two keys + 10 preset faders with one key, four wheels, keypad.
4. Motorised playback faders, plain preset faders and grand master.
5. Four isolated DMX ports on an own MCU (ENTTEC Pro compatible).
6. Wheels: optical encoders, as professional desks use; quality before price, no budget variant
   required.
7. Plans published, the owner may sell kits later.
8. Licence: CERN-OHL-S v2 (hardware), CC BY-SA 4.0 (build guide).
9. No sunlight requirement; a 1,000-nit panel is an option in the guide.
10. Strips: two PCB designs, 33 mm pitch, two faders per strip, smart (own RP2350), RS485.
11. Prototype order: the USB-MIDI box first.

## Build tasks (`BACKLOG.md`, added when this spec is approved)
Build tasks are `HWB-nn` so they never read as a criterion `HW-nn`; HW-01 (this spec) keeps its id.
- **HWB-01** USB-MIDI box firmware on a Pico 2 + checklist (HW-20 for its controls, 14, 18, 22,
  30, 32, 33, 61, 62, 70).
- **HWB-02** Playback strip PCB + strip firmware + checklist (HW-10..18, 30, 31).
- **HWB-03** Main controller PCB + firmware + checklist (HW-12, 13, 19..23, 31, 32, 63).
- **HWB-04** Preset strip PCB + checklist (HW-10, 11, 14, 15, 18).
- **HWB-05** DMX board PCB + firmware + checklist (HW-40..44).
- **HWB-06** Programmer panel PCB + checklist (HW-07, 08, 22).
- **HWB-07** Enclosure and front plates CAD (HW-01, 04, 34, 36, 50, 51, 53).
- **HWB-08** Flight case drawings (HW-52).
- **HWB-09** Build guide and BOM, licence files (HW-02, 03, 35, 55).
