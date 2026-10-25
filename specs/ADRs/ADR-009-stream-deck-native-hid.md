# ADR-009: Stream Decks over native USB HID from the daemon

**Status:** accepted (owner, 2026-10-25)
**Date:** 2026-10-25
**Origin:** IDEA-22, BACKLOG P1-19; criteria CTL-44..58 in `specs/modules/control-surfaces.md`

## Context
The owner wants Elgato Stream Decks to work natively: link playbacks to keys from the software,
keys showing legend, colour and state. A Stream Deck is not MIDI. It is a USB HID device (vendor
`0x0fd9`) with an LCD per key; the host sends each key a JPEG or BMP image in 1024-byte output
reports and reads key states (and on the Stream Deck + dial and touch reports) from input reports.
Models differ in key count, image size, format, rotation and extras.

Two routes:
1. **Native HID in the daemon.** CMDMX opens the deck, reads keys and draws every image itself.
2. **An Elgato Stream Deck plugin** (`@elgato/streamdeck`, Node.js 24, Stream Deck app 7.1+) that
   talks to the daemon over OSC or IPC; the Elgato app owns the device and draws the keys.

## Decision
1. **Route 1.** The daemon drives decks over USB HID through the `hidapi` crate, on its own `deck`
   thread with panics caught, like the MIDI thread. A deck is a device on the general control
   layer (DECISIONS 2026-10-23): CTL-05 actions, Learn, mappings per Mac, feedback.
2. **The deck is opened only while enabled, and then exclusively** (hidapi's macOS default). While
   another program (the Elgato app) holds it, the deck is `Busy` and retried every 2 s.
3. **Models are a table** (`profiles/streamdeck-models.json`), not code.
4. **The daemon renders the key images.** `cmdmx-control` holds a pure renderer (key look + model
   → image bytes, deterministic, golden-tested); the deck thread renders and writes, so the tick
   never waits for USB and the deck works with the app closed. New dependencies: `hidapi` in the
   daemon; a font rasterizer (candidate `fontdue`) and a JPEG encoder (candidate
   `jpeg-encoder`) in `cmdmx-control`, both pure Rust; one built-in font under the SIL Open Font
   Licence. VS7-05 confirms the crates and licences before adding them.

## Alternatives considered
- **Elgato plugin (route 2).** Lives beside other Stream Deck actions, but the show then depends on
  a second program (the Elgato app and a Node runtime) running and staying healthy, a plugin
  install per Mac, and Elgato's SDK versions. A show-proof console cannot need that. Anyone who
  wants it can already use the Elgato app's or Bitfocus Companion's OSC actions on `/cmdmx`
  (CTL-36) without CMDMX code.
- **Both routes.** Twice the work and two behaviours for one device.
- **Shared (non-exclusive) open.** Both programs would write images to the same keys and both
  would act on every press.
- **The app renders images (CoreGraphics) and sends them to the daemon.** Decks would go dark when
  the app is closed or restarted, and image traffic would cross the IPC.

## Consequences
- The daemon gets a third device thread (after MIDI and serial) and a model table to maintain;
  new models are a table row plus, rarely, a report family.
- Only the owner's model is verified on hardware; the rest ship as `untested`.
- Unknown: whether macOS asks for Input Monitoring when the daemon opens a deck.
  `tests/hardware/streamdeck.md` records it; if it does, the daemon names the permission in the
  `Busy`/error warning.

## Reversal trigger
Elgato changes the firmware protocol in a way that cannot be followed from the open references,
or decks prove unreliable over raw HID on macOS in the gate-3 run.
