# Research: hardware for the open-hardware maker desk (IDEA-23)

Status: research only, phase 3 (hardware). No acceptance criteria, no code. Written 2026-10-23.

This note collects concrete parts for the CMDMX maker desk described in IDEA-23 (`IDEAS.md`):
two 15" touch screens, playback faders under them, keys per fader, encoders, a flight case,
show-proof. The architecture stays as decided: the desk surface is one more controller profile
on the general MIDI + OSC control layer (CTL, VS7), it shows up as a USB-MIDI device, the
engine does not change, and the fader count is configuration (`faders_per_page`).

**About prices.** All prices are rough EU street prices including VAT, October 2026, meant for
order-of-magnitude planning. Memory prices rose sharply in 2025–2026 (Raspberry Pi raised the
Pi 5 several times; LPDDR4 is up roughly sevenfold in a year according to the Raspberry Pi
blog), so anything with RAM in it moves month to month. Re-check before buying.

**Reference consoles (descriptive only; names are trademarks of their owners).** The layout
target sits between the Avolites Titan Mobile (a compact surface used with a laptop; about
€2,400–4,000 new depending on year and dealer) and the Avolites Tiger Touch (one large touch
screen, 10 playback faders with keys, encoders; about €7,000+ used). The ChamSys MagicQ MQ250M
(two screens, playback faders, about €19,000 new at Thomann) shows what a two-screen desk costs
in the market. A €1,000–2,500 build with two screens and 10–15 playbacks is a real gap.

---

## 1. Compute

The compute board runs the daemon and the UI on two 1080p touch screens. The daemon is light
(Rust, deterministic tick); the load is the UI on two screens.

| Candidate | Rough price | Two 1080p screens | Linux GPU / drivers | Power loss | Availability |
|---|---|---|---|---|---|
| **Mac mini M4** (16 GB / 256 GB) | ~€720 at launch (2024), ~€970 list by mid-2026; newer models ~€1,070; refurbished M4 lower | Yes, up to three displays (HDMI + Thunderbolt/USB-C) | n/a (macOS; runs today's SwiftUI app) | APFS is robust; "start up after power failure" setting; internal PSU | Apple store, all retailers; models change yearly |
| **Raspberry Pi 5** 8 GB / 16 GB + NVMe HAT | 8 GB ~$175 (≈ €190), 16 GB ~$305 (≈ €320) after the April 2026 rises; NVMe HAT ~€15–30 | Yes, two micro-HDMI 4Kp60 | Best ARM story: VideoCore VII with upstream Mesa (V3D), Wayland (labwc) default in Raspberry Pi OS | Needs UPS/supercap HAT and a read-only root; boots from NVMe | Official resellers in NL/EU (Kiwi Electronics, Pimoroni, The Pi Hut) |
| **RK3588 boards**: Orange Pi 5 Plus, Radxa Rock 5B+ (16 GB) | ~€150–300, RAM-dependent | Hardware yes (2× HDMI 2.1 on 5 Plus, HDMI + USB-C DP on Rock 5B+) | Weakest: GPU (Panthor) landed in Linux 6.10, mainline HDMI only from 6.13 on and still maturing; vendor BSP kernels are old. Radxa is usually ahead on mainline | NVMe on board; same UPS needs as the Pi | AliExpress / vendor stores; board revisions change often |
| **x86 mini-PC, Intel N100/N150** (e.g. Beelink Mini S13, NiPoGi E1; 16 GB / 500 GB) | ~€170–260 (2025 deal prices; likely higher now) | Yes, dual HDMI 4K60 | Mature: Intel i915/Xe in mainline for years; any distro works; also runs Windows | NVMe SSD; "restore on AC loss" in BIOS; iTCO hardware watchdog | Amazon.de / bol.com; many white-label vendors, specific models come and go |

**Pros and cons in short**

- **Mac mini:** zero porting, runs today's app, very quiet, very reliable. But: macOS has **no
  native touchscreen support** (see §2), it is the most expensive, it ties the desk to Apple,
  and it does not suit a fully open build. Good for build step 2 in IDEA-23.
- **Pi 5:** cheapest community-friendly option with the best documented Linux graphics on ARM,
  huge maker base, hardware watchdog. But: needs NVMe HAT, UPS and active cooling; RAM price
  hikes removed much of its price edge; two 1080p screens are fine for a 2D UI, but heavy
  visualisers would struggle.
- **RK3588:** most power per euro on ARM, NVMe and 2.5 GbE on board. But: Linux display/GPU
  support is the least mature and depends on board-specific kernels; risky for "show-proof".
- **N100/N150:** cheapest x86, mature drivers, standard PC boot and watchdog, runs any OS the
  cross-platform UI ends up on. But: white-label quality varies (fans, BIOS), and models
  disappear quickly, so a build guide must name a "class" rather than one SKU.

**Recommendation.** For build step 2, the **Mac mini** (whatever the current base model is).
For the cheaper, Apple-free step 3, prefer an **x86 N100/N150 mini-PC** over the Pi: same
price class after the RAM hikes, mature GPU drivers, hardware watchdog and NVMe out of the
box, and nothing to solder. Keep the **Pi 5 (8 GB) + NVMe** as the "maker classic" variant.
Do not build the reference design on RK3588 until mainline display support is boring.

## 2. Touch screens

Two 15–15.6" 1080p capacitive (PCAP) panels, ideally the same model so they look and calibrate
alike.

| Candidate | Rough price | Brightness | Notes |
|---|---|---|---|
| **ViewSonic TD1655** (portable, USB-C, 15.6" 1080p IPS, 10-point touch) | ~€300 | 250 nits | Branded, documented, USB-C video + touch on one cable. Plastic shell must come off or be hidden |
| **Generic portable touch monitors** (Arzopa, UPERFECT, Kenowa type; USB-C + mini-HDMI) | ~€150–220 | 300–350 nits (seller claims) | Cheapest; quality and firmware vary per batch; good for prototypes |
| **Waveshare 15.6" capacitive touch LCD** (1080p IPS, HDMI + USB touch, toughened glass, bare with metal back) | ~€170–240 | ~300 nits | Made for makers: bare panel with driver board, screw mounts, Raspberry Pi docs. Best fit for a flush front plate |
| **Industrial open-frame PCAP** (Faytech FT156…HB, 2U CaphView CVM15WG) | ~€600–850 | 1,000 nits | High brightness, VESA, 12–24 V DC, long availability. For sunlight/outdoor builds only |

**Portable monitor vs bare panel.** A portable monitor is one USB-C cable and cheap, but its
case, buttons and kickstand fight the enclosure, and the model changes every year. A bare panel
with driver board (Waveshare type, or an eDP panel plus HDMI–eDP board and a separate PCAP glass)
mounts behind a cut-out in the front plate with a bezel, which is what a desk needs. Recommend
the **bare-panel route** for the published design, with a portable monitor as the prototype.

**Mounting.** Panel behind a laser-cut aluminium or ply bezel, rubber gasket, M3 standoffs; tilt
the screen section 15–25° (a hinged lid like a Tiger Touch is nicer but harder to build). Leave
the driver board reachable for service. Keep the USB touch cable and HDMI cable locked
(strain relief inside the case).

**Brightness and stage use.** Indoors (church, theatre) 250–350 nits is plenty; the real
problem is the opposite: screens must dim very low in a dark room. Many portable monitors do
not go dark enough, so the UI should have its own dim overlay. Sunlight needs ~1,000 nits and
an anti-glare surface, which only the industrial panels deliver.

**Touch on two screens, per platform.**
- **macOS has no native touchscreen support.** A USB touch panel often acts as a single mouse
  pointer at best, and does not know which screen it belongs to. Two touch screens on a Mac
  need a commercial driver such as **UPDD (Touch-Base)**, which supports macOS on Apple Silicon
  and multi-monitor setups; licence in the order of €90–150 (check current price). This is a
  real cost and a dependency for the Mac desk and must be tested early.
- **Linux (Wayland)** supports multi-touch natively; each touch device must be mapped to its
  output (by udev rule or compositor config). Two identical panels report the same USB IDs, so
  mapping goes by USB port path. Solvable, but it belongs in the build guide.

**Recommendation.** Two **Waveshare-class 15.6" bare PCAP panels** for the reference design;
industrial 1,000-nit panels as an outdoor option. Prototype with any USB-C portable monitor.

## 3. Faders

Count for planning: **10–15 playback faders plus one grand master** (configurable through
`faders_per_page`; a 10 + 1 layout is used in the BOMs below).

| Candidate | Rough price | Type | Notes |
|---|---|---|---|
| **ALPS RSA0N11M9** (100 mm, 10 kΩ linear, motor, touch-sense track) | ~€20–35 (grey market), €50–70 at distributors historically | Motorised | The pro-audio standard (Yamaha, Behringer X-Touch class). Spares are easy to find; distributor stock is irregular |
| **Bourns PSM01-081A/082A-103B2** (100 mm, 10 kΩ linear, motor, touch-sense lever) | ~£14–20 at RS (≈ €17–24), lower per tray | Motorised | Available new from RS/Mouser/DigiKey with proper datasheets; best choice for a published BOM |
| **Plain 100 mm faders** (ALPS RS60 series, Bourns PTB/PTL type) | ~€5–12 | Plain | Cheap, simple, robust; needs pick-up ("catch") logic after a page change |

**Motor driver ICs.** Each motorised fader needs an H-bridge: **TB6612FNG** (dual, 1.2 A, ~€2)
or **DRV8833** (dual, ~€1.5); one IC drives two faders. The motors usually run at about 10 V,
so the surface needs a 10 V rail. Position control is a PID loop on the fader's own wiper; the
open-source *Control-Surface-Motor-Fader* project (tttapa) shows a working PID with touch
sensing over I²C and a tuning script.

**Touch-sense.** The metal fader knob plus the touch track is a capacitive sensor. It is needed
so the motor lets go when an operator grabs a fader. Use a capacitive controller (MPR121,
12 channels, I²C, ~€5) or the MCU's own touch peripheral. Note: the **Teensy 4.x has no
`touchRead`** (the Teensy 3.x did), so a Teensy 4.1 build needs an MPR121 or similar.

**Cost for 11 faders (10 + GM):** motorised (Bourns) ≈ 11 × €20 + 6 drivers + 1 MPR121 ≈
**€250–300**; plain ≈ **€60–130**. The grand master can stay plain even on a motorised desk
(it is never moved by a page change).

**Recommendation.** **Motorised Bourns PSM** for the playback faders: page changes are clean
and match the "a lost surface never changes levels" rule better than catch logic. Plain faders
remain a supported "budget" variant; the control layer already needs pick-up for the BCF2000.
Grand master: plain 100 mm.

## 4. Keys and encoders

**Per-fader keys.** A Titan-style playback has Go/Flash and Select (Tiger Touch: two keys per
fader; up to three is common). Keys need RGB to show playback state from feedback (CTL).

| Candidate | Rough price | Notes |
|---|---|---|
| **Cherry MX RGB / Gateron or Kailh MX-compatible "RGB" switches** + SK6812 Mini-E reverse-mount LED + clear/relegendable keycap | switch ~€0.40–1, LED ~€0.10, relegendable cap ~€1–2 | Standard footprint, hot-swap sockets possible, huge choice of caps. Tall |
| **Kailh Choc v1/v2 (low profile)** + SK6812 Mini-E + Choc caps | switch ~€0.50–1, caps ~€1 | Flat, looks like a console; fewer legend-cap options; Choc v2 accepts MX-stem caps |
| **Lit tact switches / LED push buttons** (e.g. 12 mm square illuminated tact, E-Switch/APEM type) | ~€2–6 | Console-like feel, panel-mount, fewer RGB options, pricier per key |

**Legends.** Clear keycaps with a printed paper insert ("relegendable" caps, as used on
X-keys-style pads) let users label keys; laser-etched caps suit fixed functions. Avoid printed
custom caps in the reference BOM (small runs are expensive).

**LED drivers.** **SK6812 Mini-E** (WS2812-compatible protocol, reverse-mount through the
switch window) is the default: one data line per chain. The protocol is timing-critical;
on an RP2040/RP2350 the PIO handles it, on a Teensy 4.1 the WS2812Serial / OctoWS2811 libraries
use DMA so the CPU is not blocked. A dedicated matrix driver (**ISSI IS31FL3741**, I²C, up to
117 RGB LEDs) avoids timing issues and allows global dimming, at the cost of a matrix layout.
Global dimming matters: lit keys must go dark in a dark room.

**Encoders for attribute wheels.** Titan-style desks use smooth (non-detented) encoders.
- **Bourns PEC11R / ALPS EC11** with push, detented or non-detented: ~€2–4. Fine for menus
  and a budget wheel; mechanical contacts wear and bounce.
- **Optical encoder** (Grayhill 62A / 61C class) with push: ~€30–50. Smooth, long life, no
  bounce; the right choice for attribute wheels.
- **Magnetic angle sensor** (AS5600, ~€2) under a free-spinning knob on a bearing: smooth and
  cheap, needs a little mechanical design; push needs a separate switch.

**Recommendation.** MX-compatible RGB switches with relegendable caps (easy to source,
hot-swap) and SK6812 Mini-E LEDs; **three or four smooth wheels** (optical, or AS5600 for the
budget build) plus a few PEC11R detented encoders for menus.

## 5. Microcontroller and I/O

The surface MCU reads faders, keys and encoders, drives motors and LEDs, and appears as a
**USB-MIDI** device (class compliant, no driver on macOS or Linux).

| Candidate | Rough price | USB-MIDI | Analog | Notes |
|---|---|---|---|---|
| **Teensy 4.1** (600 MHz Cortex-M7) | ~€35–40 | Yes, composite (MIDI + serial + HID) in the Teensyduino core | 18 analog pins (2 ADCs) | Mature MIDI library, huge DIY-controller base; no touchRead on 4.x; single source (PJRC) |
| **Raspberry Pi Pico 2 (RP2350)** / RP2040 | ~€5–7 | Yes (TinyUSB) | Only 3–4 ADC inputs → external mux/ADC | Cheapest; PIO is ideal for LED timing and encoders; many vendors and the bare chip for a custom PCB |
| **ESP32-S3** | ~€8–15 | Yes (TinyUSB) | ~20 ADC channels, but ADC is noisy | Wi-Fi/BLE unused here; native capacitive touch pins; noisy ADC is a minus for faders |

**Multiplexing and ADC.** 11 faders fit a Teensy's own ADCs; on an RP2350 use **74HC4051**
8:1 muxes or an external ADC (**MCP3208**, 12-bit, 8 ch, SPI, ~€4). Keys go in a diode matrix
or on shift registers / I/O expanders (74HC165, MCP23017). Encoders: PIO (RP2350) or interrupt
pins (Teensy).

**Latency.** USB full-speed MIDI has 1 ms frames; scanning 11 faders + 40 keys at 1 kHz is easy
on all three. End-to-end latency is dominated by the daemon tick, not the surface. Use
hysteresis/deadband on faders so a still fader never sends MIDI (stops jitter feedback loops
with motorised faders).

**Show-proof notes.** Enable the MCU's hardware watchdog. On boot, the surface sends nothing
until it has read real fader positions (a reset must not send fader "0"). The daemon already
treats a lost surface as "no change"; the firmware must match (motors stay put, LEDs show
"disconnected").

**Recommendation.** **RP2350 on a custom PCB** for the published design (cheap, second-source
chip, PIO for LEDs and encoders) with an external ADC; **Teensy 4.1** for the first prototype
because its MIDI and motor-fader examples get a working surface fastest. Both stay "a controller
profile" on CTL; no engine change.

### 5a. Modular fader strips (owner's proposal, 2026-10-23)

Rather than one big surface PCB: small identical **fader-strip boards** (faders, keys, LEDs,
motor drivers, touch sense) that plug into one **main controller board**, which is the single
USB-MIDI device towards the compute. 10 faders = 5 boards of 2 faders; 15 faders = 8 boards
(one with a single fader populated). The fader count stays configuration (`faders_per_page`); the
main controller simply reports how many strips it found.

**Cost indication (owner, 2026-10-23):** for an unrelated board of the owner's (about
200 × 50 mm), 5 fully SMD-assembled JLCPCB boards cost about **€65 in total**, including parts,
assembly, shipping and import duties. That board says nothing about the size or design of a
fader strip; it only shows that small runs of assembled boards are cheap. The faders, switches
and caps on the strips are the real cost.

**Design points for the strip:**
- **Size and pitch.** The strip size follows from the layout, not the other way round: a 100 mm
  fader body is about 128 mm long, plus 2–3 keys above it; two faders side by side at a
  console-like pitch of 30–35 mm need roughly 60–70 mm of width. Pick the fader pitch and keys
  per fader first, then size the board.
- **Smart or dumb strip.**
  - *Dumb:* only faders, switches, LEDs and an I/O expander/ADC, read by the main MCU over a
    ribbon cable. Cheapest, but motor PID loops and many analog lines travel over the cable, and
    motor noise gets onto the ADC lines.
  - *Smart (recommended):* one **RP2350** per strip runs the motor PID, touch sense, key scan and
    LEDs locally, and talks to the main controller over a digital bus. Analog stays on the strip,
    and a strip that hangs is reset by its own watchdog without touching the others. The extra
    chip costs about €1–2 in an assembled JLCPCB run.
- **Bus to the main controller.** **RS485** (half-duplex UART, one transceiver per board) or
  **CAN** (RP2350 has none built in; needs an MCP2515/MCP251863-class part) are robust over
  a desk-length cable with motor noise. I²C is fine on one board but fragile across connectors and
  next to fader motors, so avoid it between boards.
- **Connector.** One keyed, locking cable per strip carrying 12 V, GND and the bus (e.g. a
  JST-XH/PH or a shrouded IDC header, daisy-chained). Each strip makes its own 10 V (motors),
  5 V (LEDs) and 3.3 V locally with small bucks/LDOs; 12 V distribution keeps currents low.
- **Addressing.** Position in the chain (a strap or DIP switch per strip, or auto-enumeration
  along the daisy chain) so the main controller knows which strip is fader 1–2, 3–4, …
- **Through-hole parts.** Motorised faders and MX switches are through-hole. Either use
  JLCPCB's THT assembly (extra cost) or hand-solder them; Kailh **hot-swap sockets** for MX
  switches are SMD and can be machine-placed, so switches just push in.
- **Show-proof.** The main controller treats a missing or silent strip as "no change": its
  faders send nothing, the others keep working. Strips power up with motors off until they have
  a target from the main controller.

This is its own design track (strip PCB, main controller PCB, firmware for both) inside phase 3,
and it fits the build path: strips plus main controller are step 1 (a USB-MIDI surface for
today's Mac app).

## 6. DMX and network outputs

**Art-Net / sACN** come from the compute board's Ethernet. On the desk this means an **etherCON**
panel jack (Neutrik NE8FDP, ~€6–9) wired to the compute board, ideally through a small
built-in switch if both a LAN and a lighting network are wanted.

**Built-in DMX XLR ports.** Options:

| Option | Rough price per port | Notes |
|---|---|---|
| **MAX485 / THVD1450 (non-isolated)** | ~€1–3 | Cheap, works; a ground fault or a miswired fixture can damage the desk |
| **ADM2587E (isolated, integrated isoPower DC-DC, 2.5 kV)** | ~€8–12 | One chip per port, no separate isolated supply; needs a careful 4-layer layout for emissions (ADI has an eval board) |
| **ISO3082 / ISO1410 + isolated DC-DC module** | ~€6–10 | TI alternative; two parts per port |

Plus a Neutrik 5-pin XLR female (NC5FD, ~€4–6), termination, and TVS protection on A/B.

**Who generates DMX?** Not the compute board directly (Linux/macOS UART timing for the DMX break
is not show-proof; same reason the project never treats an interface as Open DMX). Two clean
ways:
1. A **second small MCU** (or the surface MCU) speaks the **ENTTEC Pro label-6 protocol** over
   USB serial and generates DMX timing in hardware. The daemon already drives that protocol via
   `--serial <path>` (OUT-09/23), so built-in ports need **no engine change**. A separate MCU
   keeps output alive if the surface firmware hangs; this is the preferred split.
2. A built-in **Art-Net/sACN node** board (e.g. a DMXking- or ENTTEC-class node mounted inside).
   Simple, off-the-shelf, more expensive (~€150–300 for two ports).

**Recommendation.** Ethernet (etherCON) is the base; **two isolated XLR ports (ADM2587E)** driven
by a dedicated ENTTEC-Pro-compatible MCU as an option on the PCB.

## 7. Power and show-proofing

| Item | Candidates | Rough price | Notes |
|---|---|---|---|
| Mains inlet | **Neutrik powerCON TRUE1** (NAC3MPX) or **locking IEC C14** (Schurter / IEC-Lock) | €8–15 | Locking, cannot be kicked out. powerCON TRUE1 also allows a through-out |
| PSU (Pi/N100 build) | **Mean Well LRS-100-12** or **IRM-60-12** + DC-DC to 5 V / 10 V | €20–35 | Mean Well is a safe, well-documented choice; 12 V main rail, buck to 5 V (Pi), 10 V (motors), 3.3 V |
| UPS (Pi build) | **Geekworm UPS_Scap** (supercap, GPIO power-loss signal) or **SupTronics X1208** (21700 Li-ion + NVMe on one HAT) | €30–50 | Supercap: no battery ageing, minutes of hold-up at light load, enough for a clean shutdown. Li-ion: longer runtime |
| UPS (DC desk, any compute) | **Mean Well DRC-100** (DIN rail PSU with battery charger, 13.8 V) + small 12 V SLA/LiFePO4 | €60–100 + battery | Whole desk on battery for minutes; signal to the OS for shutdown |
| UPS (Mac mini build) | small line-interactive UPS (APC Back-UPS / Eaton 3S class, USB) | €70–120 | macOS supports USB HID UPS natively and can shut down on low battery |

**Software side (Linux builds).**
- **NVMe, never SD**, for the root filesystem.
- **Read-only root** (Raspberry Pi OS has an overlay filesystem option in `raspi-config`;
  generic distros: overlayroot or a read-only `/` with a writable data partition). Show files
  live on the data partition; the daemon already autosaves via temp file + atomic rename.
- **Hardware watchdog** (Pi: BCM watchdog; N100: iTCO) armed through systemd
  (`RuntimeWatchdogSec`), plus systemd restarts for daemon and UI.
- **Power-loss input** (UPS GPIO/USB) triggers: flush show file, then clean shutdown.
- **Auto-power-on** after mains returns (BIOS "restore on AC loss", Pi default, Mac setting).

**Mac build.** No user-accessible hardware watchdog; rely on `launchd` KeepAlive for the daemon,
"start up automatically after a power failure", and the USB UPS.

**Recommendation.** powerCON TRUE1 inlet, Mean Well PSU, supercap UPS for the Pi build,
DRC-100 + battery for an N100 desk, external USB UPS for the Mac desk. NVMe, read-only root
and watchdog are mandatory in the build guide, not options.

## 8. Enclosure and flight case

**Desk body.**
- **Front plates:** 2–3 mm aluminium (5052 or 5754), laser-cut and powder-coated or anodised,
  legends by UV print or laser engraving. Alternative for makers: 4–6 mm birch ply or black
  acrylic cut on a hobby laser.
- **Frame:** 15 mm birch ply sides and base plus **aluminium extrusion** (20×20 / 2020 profile,
  e.g. Misumi, Motedis, Dold Mechatronik) for the screen frame and stiffening. Extrusion makes a
  parametric design easy: change the width for 10 or 15 faders.
- **Ventilation and service:** removable bottom plate; filtered fan intake; all boards on
  standoffs; connectors (etherCON, XLR, powerCON, USB-B for an external surface) on a rear plate.

**Flight case.** Standard case: 9 mm phenolic birch ply, aluminium extrusions, ball corners,
butterfly latches, recessed handles, foam lining.
- **Penn Elcom** (EU depot in Emmerich am Rhein, near the Dutch border) has the full case
  hardware range and also laser cutting and custom panels; Adam Hall is an alternative, both
  sold through Bax-shop and other NL/EU retailers.
- A case maker can build from published drawings (outer dimensions, foam plan). Budget for a
  made-to-measure case: roughly €300–600; DIY with hardware kit: €150–250.

**Services.**
- **PCBs and assembly:** JLCPCB and PCBWay (cheap, fast, assembly for SMD parts incl. the
  RP2350); Aisler (Germany/NL, EU-made, more expensive) for small EU runs.
- **Laser / CNC sheet metal:** online quote services in NL/EU such as **247TailorSteel** (NL,
  laser cutting and bending, online quote), **Laserboost** (NL), **Protolabs** (EU) and **Xometry
  Europe**; Penn Elcom for 19"-style panels.
- **Front-panel engraving:** Schaeffer AG (Germany, "Front Panel Designer") for small runs of
  engraved, anodised aluminium panels.

**Recommendation.** Ply + 2020 extrusion body with laser-cut aluminium front plates from an
online NL service; a parametric case drawing using standard Penn Elcom/Adam Hall hardware.
Keep the design in an open CAD tool (FreeCAD for the body, KiCad for PCBs) so makers can edit it.

## 9. Hardware licence

The software licence is a separate choice (IDEA-21). For plans (PCB, CAD, BOM, build guide):

| Licence | Reciprocity | Fits | Watch out |
|---|---|---|---|
| **CERN-OHL-S v2** | Strong: changed designs and designs that include it must be published under the same licence | Keeps every derived desk open; closest to "GPL for hardware" | Hard to combine with closed component libraries or closed sub-modules; may scare off small makers who want to sell a variant with their own closed parts |
| **CERN-OHL-W v2** | Weak: changes to *our* design must be open, but it may be used inside a larger closed product | Good compromise: improvements to the desk come back, a company may still build a bigger product around a CMDMX surface | Larger product around it can stay closed |
| **CERN-OHL-P v2** | None (permissive, keep notices) | Maximum adoption | Anyone can close it |
| **CC BY-SA 4.0** | Share-alike for the *documents* (drawings, photos, build guide) | Good for the build guide and photos | Written for creative works; no patent clause, unclear for the physical object. Not recommended for PCB/CAD |

All three CERN-OHL v2 variants are OSI-approved. A common pattern is CERN-OHL for the design
files plus CC BY-SA for the build guide and photos.

**Recommendation.** **CERN-OHL-W v2** for PCB and CAD if the owner wants kits/PCB sales by third
parties to stay possible without forcing them to open everything around it; **CERN-OHL-S v2** if
every derived desk must stay open. Build guide and photos under **CC BY-SA 4.0**. Firmware is
software and follows the IDEA-21 decision.

---

## Sample bills of materials

Indicative, 10 playback faders + grand master, two 15.6" touch screens, ~30 keys, 4 wheels,
2 DMX ports. Ranges cover price swings and sourcing choices.

### Build A — budget Linux desk (Pi 5 or N100)

| Item | Choice | Price range |
|---|---|---|
| Compute | Raspberry Pi 5 8 GB + NVMe HAT + 256 GB NVMe + active cooler, **or** N100/N150 mini-PC 16 GB | €200–300 |
| Screens | 2× Waveshare-class 15.6" PCAP bare panel | €340–480 |
| Faders | 10 plain 100 mm + 1 GM (motorised upgrade: +€200) | €60–130 |
| Keys | 30 MX RGB switches, SK6812 Mini-E, relegendable caps | €60–100 |
| Encoders | 4 AS5600 wheels + 4 PEC11R | €20–40 |
| Surface electronics | 5 fader-strip PCBs + main controller PCB (RP2350, JLCPCB assembled; see §5a) | €60–120 |
| DMX | 2 ports: ADM2587E, NC5FD, second MCU | €30–50 |
| Power | powerCON TRUE1, Mean Well PSU, DC-DC, supercap UPS | €80–130 |
| Connectors & cables | etherCON, USB, internal cables, strain reliefs | €40–70 |
| Enclosure | ply + 2020 extrusion + laser-cut aluminium front plates | €150–300 |
| Flight case | DIY with Penn Elcom/Adam Hall kit | €150–250 |
| **Total** | | **€1,190–1,970** |

### Build B — Mac mini desk

| Item | Choice | Price range |
|---|---|---|
| Compute | Mac mini (current base model, 16 GB / 256 GB) | €970–1,100 |
| Touch driver | UPDD licence for two touch screens on macOS | €90–150 |
| Screens | 2× Waveshare-class 15.6" PCAP bare panel (or 2× ViewSonic TD1655) | €340–600 |
| Faders | 10 motorised Bourns PSM + 1 plain GM, 5–6 TB6612FNG, MPR121 | €250–320 |
| Keys | 30 MX RGB switches, SK6812 Mini-E, relegendable caps | €60–100 |
| Encoders | 4 optical wheels + 4 PEC11R | €130–220 |
| Surface electronics | 5 fader-strip PCBs + main controller (Teensy 4.1 or RP2350; see §5a) | €60–130 |
| DMX | 2 isolated ports with ENTTEC-Pro-compatible MCU | €30–50 |
| Power | powerCON TRUE1, Mean Well PSU for surface, external USB UPS | €120–190 |
| Connectors & cables | etherCON, USB, internal cables | €40–70 |
| Enclosure | ply + 2020 extrusion + laser-cut aluminium front plates | €200–350 |
| Flight case | made by a case maker | €300–600 |
| **Total** | | **€2,590–3,880** |

Build A lands inside the IDEA-23 target (€1,000–2,500). Build B overshoots €2,500 with a
made-to-measure case and optical wheels; with a DIY case, PEC11R wheels and plain faders it comes
down to roughly €2,000–2,400. Both are a fraction of a commercial two-screen desk.

---

## Open questions for the owner

1. **Step 3 at all:** is the Linux build (and so a cross-platform UI) the reference design, or
   is the Mac mini desk the only official build? This decides which BOM gets the build guide.
2. **Mac touch:** is a paid touch driver (UPDD) acceptable for the Mac desk, or should the Mac
   desk use one touch screen plus one plain screen?
3. **Layout:** 10 or 15 playback faders, two or three keys per fader, how many wheels, a keypad?
   Closer to Titan Mobile + wing or to Tiger Touch?
4. **Motorised or plain faders** in the reference design (motorised adds ~€200 and a 10 V rail)?
5. **Built-in DMX ports** (isolated, via an ENTTEC-Pro-compatible MCU) or Ethernet only?
6. **Smooth wheels:** optical encoders (~€40 each) or the cheaper AS5600 magnetic design?
7. **Kits/PCBs:** plans only, or sell PCBs/kits later? This affects the licence choice
   (CERN-OHL-S vs -W).
8. **Licence:** CERN-OHL-S or CERN-OHL-W for PCB/CAD, CC BY-SA for the build guide?
9. **Outdoor use:** is sunlight readability a requirement (1,000-nit panels, +€800)?
10. **Fader strips (§5a):** fader pitch, one or two faders per strip, smart (own RP2350) or
    dumb strips, RS485 or CAN between boards?
11. **Prototype order:** start with a USB-MIDI fader/key box (step 1) on the BCF2000-style
    profile, before any screens or enclosure?

## Sources

- Raspberry Pi price rises 2025–2026: [BGR](https://www.bgr.com/2150106/raspberry-pi-cost-ram-crisis/),
  [Notebookcheck](https://www.notebookcheck.net/Raspberry-Pi-5-now-costs-up-to-205-due-to-RAM-crisis.1218213.0.html),
  [OMG! Ubuntu](https://www.omgubuntu.co.uk/2026/02/raspberry-pi-annouce-more-price-rises)
- Mac mini NL prices: [iCulture](https://www.iculture.nl/nieuws/mac-mini-2026-prijzen/),
  [The Mac Index](https://themacindex.com/nl/products/mac-mini/m4-10-core-16gb-256gb)
- RK3588 mainline status: [Collabora](https://collabora.com/news-and-blog/news-and-events/rockchip-rk3588-upstream-support-progress-future-plans.html),
  [CNX Software](https://cnx-software.com/2024/12/21/rockchip-rk3588-mainline-linux-support-current-status-and-future-work-for-2025)
- N150 mini-PC prices: [PC-WELT](https://www.pcwelt.de/article/2770279/nipogi-e1-mini-pc-intel-bestseller-angebot-deal-amazon.html)
- Touch screens: [ViewSonic TD1655 (CDW)](https://www.cdwg.com/product/viewsonic-td1655-1080p-portable-monitor-with-touchscreen-60w-usb-c-buil/6191272),
  [Waveshare 15.6" (RobotShop EU)](https://eu.robotshop.com/products/waveshare-156-capacitive-touch-screen-lcd-19201080-hdmi-ips-eu),
  [Faytech 15.6" open frame](https://faytech.us/?p=14706), [2U CVM15WG (IPC2U)](https://ipc2u.com/catalog/cvm15wg_hdmi/)
- macOS touch driver: [Touch-Base UPDD](https://touch-base.com/drivers.html)
- Faders: [Bourns PSM at RS](https://uk.rs-online.com/web/p/potentiometers/1675184),
  [Conrad ALPS RSA0N11M9](https://www.conrad.com/en/p/alps-rsaon11m9-motorised-fader-10-k-0-5-w-linear-1-pc-s-442081.html),
  [Control-Surface-Motor-Fader](https://github.com/tttapa/Control-Surface-Motor-Fader/blob/master/README.md)
- UPS HATs: [SupTronics X1208 (The Pi Hut)](https://thepihut.com/products/suptronics-x1208-ups-m-2-2280-nvme-ssd-shield),
  [Geekworm UPS_Scap](https://geekworm.com/products/rpi5-ups-scap-pd5v5a)
- Isolated RS485: [Analog Devices ADM2587E](https://www.analog.com/en/products/adm2587e.html)
- Case hardware: [Penn Elcom flight case hardware](https://penn-elcom.com/flight-case-hardware-range)
- Licences: [CERN-OHL v2](https://cern-ohl.web.cern.ch/), [Weinberg, "Which open hardware license"](https://michaelweinberg.org/blog/2025/08/08/which-open-hardare-license/)
- Console prices: [Thomann MQ250M](https://www.thomann.de/be/chamsys_magicq_mq250m.htm),
  [Audiofanzine Titan Mobile](https://en.audiofanzine.com/avolites/titan-mobile/),
  [10kused Tiger Touch](https://www.10kused.com/used/avolites-tiger-touch/)
