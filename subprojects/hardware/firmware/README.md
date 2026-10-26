# Desk firmware (HW-70)

Rust firmware for the CMDMX Desk, one cargo workspace separate from the engine.

- `logic/` — `cmdmx-desk-logic`: pure `no_std` logic (deadband, boot rule, encoder, keys, wire map,
  palette, host link), unit-tested on the host. Tests carry the criterion id (`logic/tests/hw.rs`).
- `box/` — `cmdmx-box`: the USB-MIDI box proof of concept (HWB-01) on a Raspberry Pi Pico 2,
  embassy on the RP2350. Pins and tunables are at the top of `box/src/main.rs`.
- [`wire-map.md`](wire-map.md) — which MIDI message each control sends and receives, and the LED
  colour palette (HW-20).

```bash
cargo test
```

```bash
cargo box
```

```bash
cargo flash
```

`cargo test` runs the host tests (the logic crate is the default member). `cargo box` builds the
firmware for `thumbv8m.main-none-eabihf`; `cargo flash` loads it with `picotool`
(`brew install picotool`) onto a Pico 2 plugged in with BOOTSEL held. Owner checklist:
[`tests/hardware/hw-box.md`](../../../tests/hardware/hw-box.md).
