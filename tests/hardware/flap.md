# Screen flap checklist — printed flap with the real screen on the test base (HWB-10, HW-53, 54, 56, 57, 58)

Gate 3: only the owner ticks these. Copy to `tests/hardware/runs/<date>-HWB-10.md` for a run.
CAD, parts and print settings: [`subprojects/hardware/cad/klep/README.md`](../../subprojects/hardware/cad/klep/README.md).
Needs the Waveshare 15.6" HDMI LCD (H) (STATE.md, *Owner to decide*).

## Parts and print
- [ ] Waveshare 15.6" HDMI LCD (H) on the desk; it shows the Mac's screen over HDMI and touch works over USB
      before it goes into the flap
- [ ] Pinet piano hinge 25 x 1.0 (RS 328-9666) cut to 508 mm, and two igus GFM-0608-05 (RS 272-4546)
- [ ] Walnut frame (4 bars) glued and dowelled square: diagonals within 1 mm of each other
- [ ] Aluminium: clamp bar 15 x 5 x 396, two arms 15 x 5 x 110 (holes 95.0 apart), foot rod Ø 8 x 450
- [ ] Two 6 x 45 dowel pins, M3 inserts and screws, wood screws, 1 mm foam strip, slim HDMI, micro-USB and 12 V cables
- [ ] Every STL printed on the 256 x 256 bed with no part split by the slicer (HW-58): blocks L/R, covers A/B, cable
      lid, cable clamp, four clamp strips, two keepers; base 1–3, two splices, base cable clamp
- [ ] `cmdmx-klep.step`, `cmdmx-klep-strut.step` and `cmdmx-klep-testvoet.step` open in a CAD viewer and
      `python3 subprojects/hardware/cad/klep/build/calc.py` prints `ok` (HW-53)

## Build
- [ ] The base pieces are joined with the splices, and the deck leaf of the hinge is screwed in its mortise. The base
      is clamped to the table with two clamps on its front strip, because a 20 N push would tip a loose base
- [ ] The blocks are screwed into the walnut frame, the module drops in from behind, and the four strips, the top bar
      and its keepers go on. No glue anywhere near the glass
- [ ] The cables are plugged in, laid through the left block and clamped under the lid, then run along the hinge
      channel and clamped under the base. With the flap at 45°, each cable has a visible slack loop between the two
      clamps
- [ ] The covers and the cable lid go on, the flap leaf is screwed to the walnut bottom bar, and the strut is fitted

## Face (HW-54, HW-58)
- [ ] Straightedge and feeler gauge at eight points round the glass (two per edge): the glass is no more than 0.5 mm
      above or below the face, walnut and print alike. Worst value: ____ mm
- [ ] The glass does not move when pressed at its centre, and the screen shows no pressure marks near the strips
- [ ] From the front at arm's length and at 50 cm, no print join is visible: only walnut, the two side fields and
      the glass
- [ ] Flap thickness at the four corners ≤ 25 mm: ____ mm

## Stand: 20 N at the top, at every step (HW-56)
Put a ruler or a dial gauge square to the face at the top centre of the walnut top bar. Push at the top centre of
the screen, square to the glass, with 20 N: 2.0 kg on a luggage scale, or a kitchen scale held against the glass.
Hold the push for 5 s, then let go.
- [ ] 15°: the top edge moves ≤ 2 mm under the push (____ mm) and the rod stays in its groove
- [ ] 25°: ≤ 2 mm (____ mm), same groove
- [ ] 35°: ≤ 2 mm (____ mm), same groove
- [ ] 45°: ≤ 2 mm (____ mm), same groove
- [ ] At each step the edge comes back to where it was within 0.5 mm after the push
- [ ] Lifting the flap ratchets the rod forward into the next groove, and lifting the rod out of its groove lets the
      flap down a step. Folded, the flap lies flat on the base with the arms and rod in their pockets, and nothing
      rattles when the base is tapped

## Cables: 50 folds (HW-57)
Screen on and showing the Mac, touch tested once. Mark each cable with a strip of tape against both clamps.
- [ ] 50 times from folded to 45° and back (each one a full fold, with the rod in the 45° groove). Count: ____
- [ ] Afterwards the picture and touch still work at 15° and at 45°, with no flicker while the flap moves
- [ ] No tape mark has moved away from its clamp, and every plug still sits fully home in the driver board (no gap,
      no tilt)
- [ ] None of the three cables is taut at any angle: each one can be pushed 5 mm sideways between the clamps

## Swap the screen with a screwdriver (HW-54, HW-57)
- [ ] Unscrew cover A and look at the driver board: its plugs and buttons can be reached without removing the module
- [ ] Remove cover B and the cable lid, the ten strip screws, both keepers and the top bar, then the module. Only a
      T10 screwdriver is used. Time: ____ min
- [ ] Put the module back and reassemble: the glass is flush again (repeat two points of *Face*), and picture and
      touch work

Result: pass / fail. Open points go into the run file with `→` the backlog row they make.
