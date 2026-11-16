# Screen flap (HWB-10: HW-53, 54, 56, 57, 58)

The desk's screen flap around the Waveshare 15.6" HDMI LCD (H), with a printed test base, so the flap can be built
and tested before the body exists. Gate 3: `tests/hardware/flap.md`.

![The flap at 25 degrees on the test base](klep-front.png)

| File | What it is |
|---|---|
| `cmdmx-klep.f3d`, `.step` | The flap in its own frame: X across (0..570), Y from the hinge axis (0..250), Z from the underside (0) to the face (24). |
| `cmdmx-klep-strut.f3d`, `.step` | Strut arms, foot rod (desk coordinates, flap at 25°). |
| `cmdmx-klep-testvoet.f3d`, `.step` | The test base: the deck from y 320 to 551 with the hinge and the sawtooth grooves (desk coordinates). |
| `cmdmx-klep-arm.step`, `.dxf` | One strut arm, flat, for a machine shop or a CNC: 15 x 5 aluminium, two 8.0 holes 95.0 apart. Two are needed. |
| `cmdmx-klep-dummy.dxf` | A stand-in for the screen's glass, to cut from 2 mm black acrylic (see *Prototype without the screen*). |
| `cmdmx-klep-*.stl`, `cmdmx-klep-voet-*.stl` | Every printed part, one file each; each fits a 256 x 256 x 256 mm bed (HW-58). |
| `build/klep.py` | Fusion script that builds all of it. It checks every part against the real screen, the strut against the base at 0, 15, 25, 35 and 45°, and the plug envelope, and asserts thickness, flushness and bed size. |
| `build/flat.py` | Writes the two DXFs, plain Python 3: `python3 build/flat.py` prints `ok`. |
| `build/calc.py` | Strut geometry and loads, plain Python 3: `python3 build/calc.py` prints the table and asserts it. |
| `build/export.py`, `build/view.py` | Writes the files above (the arm's STEP in a scratch document it closes unsaved), and sets the cameras for the renders. |

Desk coordinates: the hinge axis runs along x at (y 345, z 13), and the deck face is z 13 (as in `console/ontwerp/scripts/sb.py`).

## How it works

- **Face (HW-54, HW-58).** The flap has a walnut frame all round: sides 12 x 24 x 250, top and bottom bars
  570 x 12.3 x 24 (owner, 2026-11-15). Between them two printed side blocks carry the face beside the glass. The glass
  fills the middle, so the only print joins are hidden behind the glass and the walnut. The glass sits 0.38 mm under
  the face. On its left and right edges a 0.38 mm lip (the blocks' first two layers, printed face down) overlaps the
  black glass border by 1.5 mm. That lip is what holds the glass forward, since nothing else can do that without glue.
  Against the walnut, the top and bottom edges are flush with a 0.3 mm gap.
- **Glass clamp (HW-54).** From behind, four printed clamp strips press the glass border forward against the lip, and
  so does an aluminium 15 x 5 bar along the top edge. Each presses through 1 mm silicone foam, squeezed to 0.72 mm.
  The lower strips' arms reach the bottom corners. The middle of the bottom edge is left free, because the panel's
  flex cable runs there. Nothing is glued: take off the covers, undo the ten strip screws and the two keepers, and the module
  comes out the back.
- **Back cover (HW-57).** Cover A (78..330) lies over the driver board, with vents under it and 2 mm thin under the
  plugs. Cover B covers the rest. Each cover has a tongue in a groove of the walnut bottom bar, and is screwed at the
  top into the walnut and at the side into a block. The cable lid on the left block is separate.
- **Hinge.** A black piano hinge, 480 x 25 x 0.7 mm, used uncut (X 72..552). Its leaves are let 0.7 mm into the
  walnut bottom bar and into the base, and the knuckle sits in a cove on the axis.
- **Strut and rack (HW-56).** Each side block takes a 6 mm stainless pivot pin, 115 mm along the flap and 9 mm
  inside it. On the pin an aluminium arm of 15 x 5 swings, 95 mm between centres, on an iglidur sleeve. An 8 mm
  stainless foot rod joins the two arms. The rod drops into four sawtooth grooves sunk 5 mm into the deck, with the
  steep face behind it, so a push on the screen only presses it harder into its step. The grooves sit at y 454.0
  (45°), 493.2 (35°), 520.8 (25°) and 540.2 (15°). Folded, the arms lie in pockets in the blocks and the rod in a
  channel of the covers, and the flap lies flat on the deck. The pivot moved 9 mm into the flap and the grooves went
  below the deck face, so the strut is 95 mm, not the massing's 85.
- **Loads.** The flap weighs 2.2 kg (module 1.0 kg), centre of gravity 125 mm from the hinge. With the HW-56 push
  (20 N at the top centre, square to the face), the strut carries 94 N at 15°, which is the worst case, and the rack
  takes 85 N toward the back. At 25° it is 69 N, at 35° 61 N, at 45° 66 N. The parts are sized for the 120 N of
  the brief.
- **Cables (HW-57).** HDMI, micro-USB (touch) and 12 V (3.5 x 1.35 mm DC) leave the driver board's left end into
  the left block. A printed clamp grips them under the cable lid. They drop through a notch in the walnut into a
  6.5 mm channel that runs 40 mm along the hinge axis (X 14..54). Folding turns them by 45° along that length
  instead of bending them at the hinge. They leave the base through a slot on the axis, and a second clamp under
  the base holds them.

![Behind the covers: clamp strips, top bar, pivot pockets, driver board](klep-open.png)

## Parts to buy (pro grade; ship-from and EU route named)

| Part | Spec | Source |
|---|---|---|
| Piano hinge | Duovorm black zinc-plated steel, 480 x 25 x 0.7 mm (SKU DV13343), uncut; €2.32 incl. VAT (owner's pick, 2026-11-15: the Pinet stainless hinge from RS was too dear for this). The hinge carries at most about 100 N along its length. | meubelbeslagonline.nl, a Dutch shop: ships within NL, VAT included, no import; free delivery from €199, so add it to a larger order. Check the hole pitch on arrival. If it misses the walnut, drill 2.5 mm countersunk holes. |
| Walnut | American black walnut, planed: 2 x 250 x 24 x 12, 2 x 570 x 24 x 12.3. Rebates 5 x 3 on the inner back edge of the sides and the top bar. A 3 x 2.4 groove (Z 3.3..5.7) along the inner face of the bottom bar from X 95.3 to 474.7. The 1 mm hinge mortise, the R 2.4 knuckle cove, the R 6.5 cable channel and the notch at X 40..54, all as in the STEP. Corners: butt joints, two 6 x 30 beech dowels and PVA each (the frame is glued; the glass never is). | The same timber source as the desk's frame (HWB-07). |
| Clamp bar | Aluminium flat bar EN AW-6060, 15 x 5, 396 mm, no holes. | Any EU metal stockist that cuts to length (e.g. a German online cut-to-size shop: ships from Germany, delivered, VAT incl.). |
| Strut arms | 2 x aluminium 5 mm, 110 x 15 with R 7.5 ends, two holes 95.0 mm apart (`cmdmx-klep-arm.step`, `.dxf`). Ream both holes to 8.0 H7 after cutting: the pivot one takes the sleeve as a press fit, the foot one the rod. Drill the pair clamped together if you make them yourself, so the two are the same. No grub screw: 0.85 mm of wall either side of an M4 tap hole in 5 mm plate is too thin, so the rod is bonded in. | JLCCNC (JLCPCB's machining and sheet-metal service; ships from China, DDP: under €150 the VAT is charged at checkout under IOSS, check it there; worth it together with a PCB order), any EU laser or waterjet cutter from the DXF, or your own CNC. A plywood pair (birch, not solid wood, which splits beside the hole) does for a first try. |
| Pivot pins | 2 x dowel pin ISO 2338 6 m6 x 45, stainless A1; a printed hole is reamed to 6.0 and the pin is pressed in. Retained by M3 x 6 with a DIN 9021 washer. | Any EU fastener shop. |
| Arm sleeves | 2 x igus iglidur G flange bearing GFM-0608-05 (6 bore, 8 outer, 5 long: the arm's thickness), pressed into the arms' pivot holes. | igus GmbH (igus.eu, ships from Cologne, DE: delivered, VAT incl., no import), or RS Components Netherlands, RS 272-4546 (EU warehouse, DDP, Dutch VAT). |
| Foot rod | Stainless 1.4301 round bar Ø 8 h9, 450 mm, bonded into both arms with Loctite 648 (retaining compound): the strut becomes one rigid frame and the rod turns in its groove. | As the flat bar; Loctite from any hardware shop. |
| Inserts and screws | Heat-set inserts M3 x 4 (hole 4.0, 34 pcs); ISO 14583 A2 Torx pan head M3 x 8 (30) and M3 x 10 (12), so a T10 screwdriver does the whole swap; stainless wood screws 3.0 x 16 pan head (15) and 2.5 x 12 countersunk for the hinge. | Ruthex inserts (ships from Germany, VAT incl.); screws from any EU fastener shop. |
| Foam | Closed-cell silicone or EPDM sponge, 1.0 mm, 10 mm strip, adhesive on one side: it sticks to the clamp, never to the glass. About 1.2 m. | Any EU rubber supplier. |
| Cables | Slim HDMI (OD ≤ 4.5 mm, overmould ≤ 21 x 11 mm), micro-USB B to A, 12 V DC 3.5 x 1.35 mm, each about 0.6 m. | Buy with the screen. |

Print in anthracite PETG or ASA: 4 walls, 25 % gyroid, 0.2 mm first layer then 0.18 mm, so the lip is two layers.
Print the blocks face down on a textured plate. The covers, lid, strips and keepers lie flat. The base pieces print
top up.

## Prototype without the screen

Everything but the screen can be built and the strut tested now. Cut `cmdmx-klep-dummy.dxf` from 2 mm black cast
acrylic (the glass is 2.1 mm, so it sits 0.1 mm deeper). It goes where the glass goes, under the lip and the clamp
strips. Weight it to the module's 1.0 kg with about 0.8 kg of steel, taped to its back, centred, inside 359 x 217 mm
and no deeper than 11 mm. That is the panel's own footprint and depth, so the strips and covers still fit. Then
*Build* and *Stand* in `tests/hardware/flap.md` can run; the face, the 50 folds with real cables and the swap wait
for the screen.

## Rebuild

1. A new Fusion design in direct modelling (no history), with the Waveshare STEP imported at the root as is (the
   screen's file is Waveshare's and stays out of the repository).
2. `klep.py` with `ARGS` = the angle, through the wrapper in `../README.md`. Its first run copies the screen into a
   flat component `M scherm`: moving the imported assembly misplaces its nested driver board.
3. Run it at 25, then `export.py` and `python3 build/flat.py`. Renders: `view.py` with `ARGS = (camera, hidden prefixes)`.

Licence: CERN-OHL-S v2 (HW-55).
