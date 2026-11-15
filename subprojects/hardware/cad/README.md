# Desk CAD (phase 10, HWB-07 groundwork)

Design-phase CAD for the open-hardware desk. Nothing here is final engineering yet: the form is chosen
(`DECISIONS.md`, 2026-11-15), the screen flap is engineered in `klep/`, and the inner frame and fasteners come next.

| Folder | What it is |
|---|---|
| `console/ontwerp/` | The design process: `ontwerpblad.html` (brief, silhouettes, massing, mixes, screen stand, colour, details, the chosen form) and its renders. `scripts/` rebuilds the massing models in Fusion. |
| `console/` | The first full console (walnut cheeks, printed bezel and rear plate, 2020 frame), whose form was replaced by the design process; printable bezel and rear-plate STLs. `build/` rebuilds it. |
| `klep/` | The screen flap (HWB-10): walnut frame, printed side blocks, glass clamps, covers, strut, hinge and a printed test base; STEP, STL, native `.f3d` and `build/` (Fusion scripts, `calc.py`). |
| `paneel-print/` | The 570 x 325 top panel as six printed pieces (3 mm skin, 10 mm ribs, AMS white inlay legends), 100 mm playback and 60 mm preset faders. `build/` rebuilds it. |
| `paneel-graveerplaat/` | The same panel as one 1.5 mm black/white engraving plate for Laserlokaal: red = cut, blue = area engraving; 20 M3 holes for heat-set inserts in a printed under-panel. `paneel_plaat.py` regenerates it. |
| `acryl-proef/` | Two 120 x 200 mm test strips (3 mm or 2 mm black acrylic, 1.5 mm engraving plate) to choose the top skin: legend sizes, line widths, fader slots, MX holes 13.8/13.9/14.0. |
| `proefstrip-print/` | The printed test strip (two faders, four keys) with inlay legends. |
| `proefkap/`, `proefkap-flatpack/` | Early sheet-metal test hood and its flat-pack plates (DXF, STEP, JLC order zip). |

## Running the Fusion scripts

The models they build are saved in the owner's Fusion hub as *CMDMX desk vorm (M1 SB K1 D2)* (Default Project).

The scripts are Fusion API scripts with a `run(context)` entry. They are run through the Fusion MCP
server by a two-line wrapper that execs the file with its globals set:

```python
def run(_context: str):
    g = {'__name__': 'x', 'CAD': '<repo>/subprojects/hardware/cad', 'ARGS': ...}
    exec(open(g['CAD'] + '/console/ontwerp/scripts/view.py').read(), g)
    g['run'](_context)
```

Order for the design models: `paneel-print/build/desk_build_60.py` (deck), `console/build/console_build.py` and
`console_frame.py` (console), then `console/ontwerp/scripts/massing.py` (B, G), `mix.py` (M1, M2), `stand.py`
(SA, SB), `sb.py` (`ARGS` = flap angle), `caps.py`, `cmf.py` (`ARGS` = K1/K2/K3), `detail.py` (D1–D3);
`view.py` (`ARGS` = (variant, camera)) shows one variant. The flap (`klep/build/klep.py`) builds in its own direct-modelling
document with the screen's STEP; `klep/README.md` says how. Each script asserts the body count it expects, so
they only run in that order on a fresh document. `layout.json` is written from `paneel_plaat.layout()`.

The DXF writers (`acryl-proef/acryl_strip.py`, `paneel-graveerplaat/paneel_plaat.py`) are plain Python 3
with fontTools and use a Fusion-exported DXF (`proefkap-flatpack/cmdmx-dekplaat.dxf`) as the file template.
