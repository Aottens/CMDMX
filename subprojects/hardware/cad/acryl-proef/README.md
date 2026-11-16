# Acrylic test strip (Laserlokaal)

Two 120 x 200 mm strips with the same layout, to choose the material for the desk's top skin.

| File | Material at Laserlokaal |
|---|---|
| `cmdmx-acryl-proef-3mm.dxf` | Acrylaat mat zwart, 3 mm (cast) |
| `cmdmx-acryl-proef-1.5mm.dxf` | Graveerplaat zwart/wit, 1.5 mm |

DXF in millimetres, one colour per operation; assign them after uploading:

- **red** = cut (snijden)
- **blue** = area engraving (vlakgraveren); every legend is a closed outline

What the strip tests: legend sizes 2-5 mm and line widths 0.2-0.8 mm; a 100 mm (Bourns PSM, slot 3.4 mm)
and a 60 mm (Bourns PTA6043, slot 2.4 mm) fader slot with scales; encoder (7.2) and wheel (10.2) holes;
M3 fixing holes (3.2). On 3 mm: key-cap windows, because MX switches cannot clip into 3 mm. On 1.5 mm:
MX holes drawn 13.8 / 13.9 / 14.0 mm, because the 0.2 mm kerf decides which one clips.

`acryl_strip.py` regenerates both files (`python3 acryl_strip.py <template.dxf> <out_dir>`, fontTools, Arial).

`onderframe/` holds two printed backings for the 1.5 mm strip (HWB-11): an open rib lattice and a closed skin, with
heat-set inserts, fader rails and MX pockets. They test whether a 1.5 mm plate on a printed frame is stiff enough for
the desk's top (`tests/hardware/top-plate.md`).
