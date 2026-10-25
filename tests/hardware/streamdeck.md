# Stream Deck checklist — native USB HID deck (VS7-07, CTL-44..58, ADR-009)

Gate 3: only the owner ticks these. Copy to `tests/hardware/runs/<date>-VS7-07.md` for a run.
Draft written with VS7-05 (the model is known); VS7-06 and VS7-07 complete the lines.

## The owner's deck (read from USB on 2026-10-25, VS7-05)
- Model: **Stream Deck Original** (first generation), USB vendor `0x0fd9` (Elgato Systems),
  product `0x0060`, bcdDevice `0x0100`, serial ending `8715` → listed as
  `Stream Deck Original (8715)` (CTL-44).
- Table row (`subprojects/engine/profiles/streamdeck-models.json`): 5 × 3 keys, 72 px BMP,
  flipped both ways, report family `v1` (8191-byte image reports, keys mirrored per row).
  `verified: false` until this run passes; then set it to `true` in the same PR (CTL-45).
- Default mapping with `faders_per_page` 10: keys 1–10 `upper 1–10`, 11–12 unbound,
  13 `page prev`, 14 `page next`, 15 `go` (CTL-49).

Setup: the deck on USB to the Mac (directly or through a hub), the Elgato Stream Deck app
**quit** unless a line says otherwise. Console running with a show in `--show-dir /tmp/…`
(`faders_per_page` 10, a few playbacks with legends and halo colours on page 1).

## Attach (CTL-44, 46)
- [ ] The Control screen lists `Stream Deck Original (8715)`, kind Stream Deck, `Disabled`; the
      keys keep what they showed (the deck is not opened)
- [ ] Enable it: within 2 s the keys show the default mapping's images
- [ ] macOS asked for **Input Monitoring** when the deck was opened: yes / no (ADR-009)
- [ ] With the Elgato app running, enabling shows `Busy` with the "quit the Elgato Stream Deck
      app" warning; quitting the Elgato app makes it `Ok` within ~2 s
- [ ] Disable: the keys go black at once and the Elgato app can take the deck again

## Keys and images (CTL-47, 50, 51)
- [ ] Key 1 (top left) is `upper 1`: it does what playback 1's button does (Flash) and the key
      shows the legend, `Flash`, the halo bar and the level bar
- [ ] Images are upright and in the right key (not mirrored, not rotated)
- [ ] An inactive playback is dimmed, an active one bright, a held Flash inverted
- [ ] An empty slot and an unbound key (11, 12) are black
- [ ] A long legend wraps to at most 3 lines and stays readable at arm's length
- [ ] Keys 13–15: page prev, page next, go; a console page change redraws keys 1–10

## Deck pages, brightness, show-proof (CTL-48, 53, 54)
- [ ] A key bound to `deck page 2` switches the deck to page 2; a key there goes back
- [ ] Brightness 0, 70, 100 from the Control screen change the deck's backlight
- [ ] Unplug the deck: it goes `Missing`, output keeps running; plug it back: `Ok` with its
      images within 2 s
- [ ] Quit the daemon: the keys go black

## Open points
- none
