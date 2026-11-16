"""The backing's geometry as plain boxes and cylinders, shared by onderframe.py (Fusion) and the cell check below.

Strip coordinates of acryl_strip.py layout('1.5mm'): x 0..120, y 0..200 (y up = away from the operator, fader top),
z 0 = the plate's underside, the backing below it down to z -H. `python3 layout.py` rasterises the face the plate
lies on and asserts that no open cell lets the plate give more than calc.LIMIT under 20 N.

Primitives: ('box', x0, x1, y0, y1, z0, z1) and ('cyl', x, y, r, z0, z1), in mm.
"""
import os, sys

if 'PITCH' not in globals():
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from calc import *          # noqa: F401,F403  (PITCH, RIB, WALL, H, SKIN, RAIL, cell_ss, rigidity, ...)

W, L, R = 120.0, 200.0, 3.0
THRU = (-H - 1, 1)

# Faders, from acryl_strip.py (slot centred on y 95) and paneel-print/build/desk_build_60.py FADER (keep-out, screws),
# shifted to the strip: PSM +10 mm in y, PTA +33 mm. Datasheets: PSM M3 tapped 120 apart, frame 13 wide, lever 8.2
# above the frame; PTA6043 M2 tapped 71 apart, body 75 x 9, lever 10 or 15 above the body.
FADERS = {
    #       x    lever slot (half-width, y0, y1)  keep-out (half-width, y0, y1)  screws y        hole r  head r  head depth
    'psm': (25.0, (2.5, 39.5, 150.5),            (8.0, 13.0, 167.0),            (37.0, 157.0), 1.7,    3.2,    1.75),
    'pta': (60.0, (2.0, 61.7, 128.3),            (6.0, 56.0, 134.0),            (59.5, 130.5), 1.2,    2.4,    1.75),
}
# MX holes drawn 13.8 / 13.9 / 14.0 at x 20, 50, 80 from y 170: pocket 16 x 16 for the switch body and its clips
KEYS = [(20 + 13.8 / 2, 170 + 13.8 / 2), (50 + 13.9 / 2, 170 + 13.9 / 2), (80 + 14.0 / 2, 170 + 14.0 / 2)]
POCKET = 8.0
ENCODERS = [(88.0, 32.0, 7.2), (106.0, 32.0, 10.2)]
CORNERS = [(6.0, 6.0), (W - 6, 6.0), (6.0, L - 6), (W - 6, L - 6)]
BOSS_R, INSERT_R, INSERT_D = 4.0, 2.0, 8.0     # Ruthex M3 x 5.7: hole 4.0, at least 6.7 deep

# Lattice: every open cell a rectangle with no clear side over PITCH (calc.py; layout.py checks each cell). The ribs
# along x run through the ends of the fader channels, so no cell wraps round a channel; the ribs along y run between
# the channels, with two more beside the 60 mm fader where its channel is short.
X_RIBS = [(43.5, WALL, L - WALL), (84.27, WALL, L - WALL), (100.93, WALL, L - WALL), (60.0, WALL, L - WALL),
          (72.0, WALL, 55.2), (72.0, 134.8, L - WALL)]
Y_RIBS = [(y, WALL, W - WALL) for y in (12.2, 26.53, 40.86, 55.2, 71.12, 87.04, 102.96, 118.88, 134.8, 151.3, 167.8,
                                        177.0, 187.3)]


def parts(variant):
    """(adds, subs) for variant 'ribben' (open lattice) or 'dicht' (closed skin under the whole plate)."""
    h = RIB / 2
    adds = [('box', 0, W, 0, WALL, -H, 0), ('box', 0, W, L - WALL, L, -H, 0),
            ('box', 0, WALL, 0, L, -H, 0), ('box', W - WALL, W, 0, L, -H, 0)]
    adds += [('box', x - h, x + h, y0, y1, -H, 0) for x, y0, y1 in X_RIBS]
    adds += [('box', x0, x1, y - h, y + h, -H, 0) for y, x0, x1 in Y_RIBS]
    adds += [('cyl', x, y, BOSS_R, -H, 0) for x, y in CORNERS]
    subs = []
    for x, (sw, s0, s1), (kw, k0, k1), screws, hr, cr, cd in FADERS.values():
        adds.append(('box', x - kw - RIB, x + kw + RIB, k0 - RIB, k1 + RIB, -H, 0))   # channel walls and rail
        subs.append(('box', x - kw, x + kw, k0, k1, -H - 1, -RAIL))                  # room for the fader body
        subs.append(('box', x - sw, x + sw, s0, s1, *THRU))                          # lever
        for y in screws:
            subs += [('cyl', x, y, hr, *THRU), ('cyl', x, y, cr, -cd, 1)]
    for x, y in KEYS:
        f = POCKET + RIB
        adds.append(('box', x - f, x + f, y - f, y + f, -H, 0))
        subs.append(('box', x - POCKET, x + POCKET, y - POCKET, y + POCKET, *THRU))
    for x, y, d in ENCODERS:
        adds.append(('cyl', x, y, d / 2 + 4, -RAIL, 0))
        subs.append(('cyl', x, y, d / 2 + 0.2, *THRU))
    subs += [('cyl', x, y, INSERT_R, -INSERT_D, 1) for x, y in CORNERS]
    if variant == 'dicht':
        adds.append(('box', 0, W, 0, L, -SKIN, 0))
    return adds, subs


def inside(p, x, y):
    if p[0] == 'box':
        return p[1] <= x <= p[2] and p[3] <= y <= p[4]
    return (x - p[1]) ** 2 + (y - p[2]) ** 2 <= p[3] ** 2


def in_outline(x, y):
    cx, cy = min(max(x, R), W - R), min(max(y, R), L - R)
    return (x - cx) ** 2 + (y - cy) ** 2 <= R * R


def open_cells(variant, step=0.5):
    """Bounding boxes (w, h) of the regions of the top face where the plate has nothing under it."""
    adds, subs = parts(variant)
    top_subs = [s for s in subs if s[-1] > 0]
    nx, ny = int(W / step), int(L / step)
    free = set()
    for i in range(nx):
        for j in range(ny):
            x, y = (i + 0.5) * step, (j + 0.5) * step
            if in_outline(x, y) and (not any(inside(a, x, y) for a in adds) or any(inside(s, x, y) for s in top_subs)):
                free.add((i, j))
    cells = []
    while free:
        stack = [free.pop()]
        i0 = i1 = stack[0][0]
        j0 = j1 = stack[0][1]
        while stack:
            i, j = stack.pop()
            i0, i1, j0, j1 = min(i0, i), max(i1, i), min(j0, j), max(j1, j)
            for n in ((i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)):
                if n in free:
                    free.remove(n)
                    stack.append(n)
        cells.append(((i1 - i0 + 1) * step, (j1 - j0 + 1) * step, (i0 * step, j0 * step)))
    return cells


if __name__ == '__main__':
    d = rigidity(*PLATES[DESIGN])
    for variant in ('ribben', 'dicht'):
        cells = open_cells(variant)
        worst = max(cells, key=lambda c: cell_ss(c[0], c[1], d, 61))
        w = cell_ss(worst[0], worst[1], d)
        print('%-7s %3d open regions, worst %.1f x %.1f at (%.1f, %.1f): %.3f mm under 20 N'
              % (variant, len(cells), worst[0], worst[1], *worst[2], w))
        assert w <= LIMIT, (variant, worst, w)
    # the faders' screws sit inside their rails, the switches and bosses inside the outline
    for x, _, (kw, k0, k1), screws, _, cr, _ in FADERS.values():
        assert all(k0 + cr < y < k1 - cr for y in screws)
    assert all(R + POCKET + RIB < x < W - POCKET - RIB and y + POCKET + RIB < L - WALL for x, y in KEYS)
    print('ok')
