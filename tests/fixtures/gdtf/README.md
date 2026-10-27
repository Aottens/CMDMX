# GDTF test files (VS2-41, `specs/modules/fixture-library.md` LIB-28..33)

Each folder holds a `description.xml` written for this project (no licence question) and, for
the importable ones, the profile the importer must produce (`expected.json`). The tests zip the
XML in memory into a `.gdtf` archive named `<folder>.gdtf`. Colour values are compared with a
small tolerance.

| Folder | What it covers |
|---|---|
| `rgbw-par` | 4ch mode: virtual Dimmer, RGB system, White emitter, emitter CIE (the Blue channel links no emitter). 6ch mode: Dimmer with Highlight, Strobe with three functions → slots. |
| `spot` | 16-bit Pan −270..270 / Tilt −135..135 with defaults `32768/2`, colour wheel with CIE slots, gobo wheel, Zoom 10–40°, Focus default `128/1`, an unknown attribute kept as a plain channel. |
| `pixel-bar` | Four geometry references with DMX breaks: master Dimmer on the body, four RGB cells. Empty `LongName` → model is `Name`. |
| `broken-xml`, `no-fixture-type`, `no-mode-left` | LIB-33 failures (the last: its only mode is on DMX break 2). |

`Generic@RGBW8@test.gdtf` is copied unchanged from the `gdtf` crate 0.3.0
(https://github.com/cpdt/gdtf-rs), under its MIT licence:

```
Copyright 2026 Tom Barham

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the “Software”), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```
