# Top plate checklist: the 1.5 mm test strip on its printed backing (HWB-11, HW-50)

Gate 3: only the owner ticks these. Copy to `tests/hardware/runs/<date>-HWB-11.md` for a run.
CAD, parts, print settings and the numbers behind the limits:
[`subprojects/hardware/cad/acryl-proef/onderframe/README.md`](../../subprojects/hardware/cad/acryl-proef/onderframe/README.md).

The question is whether a 1.5 mm engraving plate on a printed frame is stiff enough to be the desk's top, and on
which of the two backings: *ribben* (open lattice) or *dicht* (closed skin). Run every step on both. The strip moves
from one backing to the other, and its switches stay clipped in. The material is chosen (DECISIONS 2026-11-23); this
run settles the backing. The first strip came with the acrylic's key windows: the steps without keys can run on it,
the key steps need Laserlokaal's recut with the 13.8 / 13.9 / 14.0 holes.

**20 N** is 2.0 kg on a kitchen scale. Put the strip, with its supports, on the scale, tare it, and push with one
fingertip until the scale reads 2.0 kg. **0.10 mm** is a feeler gauge: stand a steel ruler on its edge across the
plate, resting on the long edges next to the screws. Push 5 mm beside the ruler. If a 0.10 mm blade slides in under
the ruler where it did not before, the plate gave more than the limit.

## Parts and print
- [ ] Both STLs printed in PETG, plate face down, as the README says; no layer shift, and no rib came off the bed
- [ ] Four inserts per backing, flush or up to 0.1 mm under the face: a steel ruler laid across does not rock
- [ ] `python3 subprojects/hardware/cad/acryl-proef/onderframe/build/calc.py` and `build/layout.py` both print `ok`
      (HW-53)

## Keys, flat on the table (no faders yet)
- [ ] Three MX switches clip into the strip's 13.8 / 13.9 / 14.0 holes. Note which hole holds its switch firmly
      without forcing, and which lets it wobble
- [ ] The plate lies on the backing without rocking before the screws go in, and the switch bodies drop into their
      pockets without touching
- [ ] Each key, pressed hard to 2 kg: the plate round it does not visibly move, nothing creaks
- [ ] Rattle: flick each keycap sideways and tap it lightly. No buzz or tick from the switch in the plate. Then turn
      the strip over and shake it: nothing rattles

## The plate between ribs, flat on the table
- [ ] 2 kg in the middle of the largest open fields: the cell beside the 60 mm fader (x 75, y 64), the cell right
      of it (x 110, y 128) and the legend area (x 90, y 110). Under 0.10 mm (feeler gauge), and it does not feel like
      it gives. On *ribben*, note where it does
- [ ] 2 kg on the plate right next to each fader slot, halfway along: the slot edge does not dip visibly
- [ ] Tap the plate with a fingernail across the strip. Note where it sounds hollow (a drum) and where it sounds dead

## Faders, on two supports
- [ ] With the plate off, both faders screwed to the backing, then the plate back on. Each screw turned in by
      hand stops on its head
- [ ] Knobs on: across the whole travel each knob clears the plate (a sheet of paper slides under it), and the lever
      does not rub the slot's edge. The PSM leaves 4.0 mm of lever above the plate; note whether the knob seats
- [ ] The strip on two blocks at least 35 mm high under its short ends (y 0–15 and 185–200), so the PSM's motor hangs
      free; the blocks stand on the scale
- [ ] 2 kg straight down on each knob, at both ends and the middle of its travel: the fader does not move in its
      channel, and the plate beside the slot does not lift or dip visibly. Note how far the strip sags in the middle
      (calc: ribben 0.28 mm, dicht 0.19 mm)
- [ ] 2 kg on the plate in the middle of the strip (x 90, y 95), with the strip on the blocks: note the sag, and
      whether the plate slides on the backing

## Verdict
- [ ] Which backing makes the 1.5 mm plate feel like a solid top: *ribben*, *dicht*, or neither. Tell Claude, so the
      choice goes into DECISIONS.md and HWB-07. If neither does, the next step is closer ribs or a screwed bottom skin
      that makes the lattice a torsion box, not another material
