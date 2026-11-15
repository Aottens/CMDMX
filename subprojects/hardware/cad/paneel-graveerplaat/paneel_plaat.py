"""Full 570 x 325 top plate in 1.5 mm black/white engraving plate (Laserlokaal "graveerplaat zwart/wit").

Same layout as the printed 60 mm panel (paneel-print/), as one plate:
- red = cut: outline, fader slots, MX key holes, wheel and knob holes, M3 fixing holes;
- blue = area engraving: fader scales and numbers, group letters, separator lines.
Faders mount to the printed underframe, not to the plate, so the plate has no fader screw holes.
M3 fixing holes sit where the underframe has room for a boss with a heat-set insert.
Usage: python3 paneel_plaat.py <template.dxf> <out_dir>
"""
import math, os, sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'acryl-proef'))
import acryl_strip as A  # noqa: E402

W, H = 570.0, 325.0
KP = 19.05
KEY = 14.0          # MX hole as drawn; set to the size that clips on the test strip
WHEEL, KNOB, M3 = 10.2, 7.2, 3.2
BOSS = 4.0          # radius of the underframe boss round each M3 insert
SLOT = {'psm': (3.4, 30.5, 139.5, 35, 10), 'pta': (2.4, 29.7, 94.3, 32, 6)}   # width, y1, y2, tick0, tick
BODY = {'psm': (8, 3, 157), 'pta': (6, 23, 101)}                             # fader body half-width, y1, y2


def row(x0, y, n):
    return [(x0 + KP * i, y) for i in range(n)]


def layout():
    """World coordinates (x right, y back from the front edge), as in desk_build_60 pieces()."""
    faders, keys, holes, texts = [], [], [], []
    for first, nstr, preset, ox, oy in ((1, 3, False, 0, 0), (7, 2, False, 198, 0),
                                        (11, 3, True, 0, 200), (17, 2, True, 198, 200)):
        for s in range(nstr):
            for j in (0, 1):
                fx = 66 * s + 16.5 + 33 * j
                faders.append((ox + fx, oy, str(first + 2 * s + j), 'pta' if preset else 'psm'))
                for ky in ((109.5,) if preset else (166.0, 185.05)):
                    keys.append((ox + fx, oy + ky))
    faders.append((330 + 16.5, 0, 'GM', 'psm'))
    keys.append((330 + 16.5, 166.0))
    keys += [(330 + x, y) for x, y in row(50, 166.0, 10) + row(50, 185.05, 10)]
    keys += [(330 + 156 + KP * c, 46.4 + KP * r) for r in range(4) for c in range(4)]
    holes += [(330 + x, y, WHEEL) for x, y in ((70, 45), (120, 45), (70, 105), (120, 105))]
    grp = row(50, 22.0, 8)
    keys += [(330 + x, 200 + y) for x, y in grp + row(50, 52.0, 10) + row(50, 71.05, 10) + row(50, 109.5, 10)]
    holes.append((330 + 16.5, 200 + 45, KNOB))
    texts += [(l, 330 + x, 200 + 32.5, 3.0) for l, (x, _) in zip('I P C G B E S FX'.split(), grp)]
    texts.append(('LED', 330 + 16.5, 200 + 54.5, 3.0))
    return faders, keys, holes, texts


def keepouts(faders, keys, holes):
    """Rectangles the underframe needs free (switch bodies, fader bodies, wheels)."""
    k = [(x - 8, y - 8, x + 8, y + 8) for x, y in keys]                      # MX body + clips below the plate
    for fx, oy, _, kind in faders:
        hw, y1, y2 = BODY[kind]
        k.append((fx - hw, oy + y1, fx + hw, oy + y2))
    k += [(x - d / 2 - 6, y - d / 2 - 6, x + d / 2 + 6, y + d / 2 + 6) for x, y, d in holes]
    return k


def screw_points(ko):
    """Four rows (front edge, mid faders, row seam, back edge) on five columns between fader pairs and
    programmer blocks; a point is kept where its boss clears every keep-out."""
    pts = [(6, 206)]
    for x in (66, 198, 330, 471, 564):
        for y in (6, 100, 206, 319):
            pts.append((x, y))
    return [(x, y) for x, y in pts
            if all(x + BOSS <= a or x - BOSS >= c or y + BOSS <= b or y - BOSS >= d for a, b, c, d in ko)]


def build():
    faders, keys, holes, texts = layout()
    cut, eng = [A.rrect(0, 0, W, H, 2)], []
    for fx, oy, lab, kind in faders:
        w, y1, y2, t0, t = SLOT[kind]
        cut.append(A.slot(fx, oy + y1, oy + y2, w))
        for i in range(11):
            ty = oy + t0 + t * i
            eng.append(A.rect(fx - 13 if i % 5 == 0 else fx - 11, ty - 0.3, fx - 8, ty + 0.3))
            if i % 5 == 0:
                eng += A.text(str(i), fx + 7.5, ty - 1.25, 2.5)
        eng += A.text(lab, fx, oy + 11.5, 5.0, bold=True, anchor='c')
    for x, y in keys:
        cut.append(A.rect(x - KEY / 2, y - KEY / 2, x + KEY / 2, y + KEY / 2))
    for x, y, d in holes:
        cut.append(A.circle(x, y, d))
    for s, x, y, cap in texts:
        eng += A.text(s, x, y, cap, bold=True, anchor='c')
    eng.append(A.rect(12, 152.6, W - 12, 153.4))
    eng.append(A.rect(12, 299.2, W - 12, 300.0))
    screws = screw_points(keepouts(faders, keys, holes))
    cut += [A.circle(x, y, M3) for x, y in screws]
    return cut, eng, faders, keys, holes, screws


def check(cut, eng, screws):
    A.W, A.H = W, H
    A.check(cut, eng)
    # every pair of cut shapes keeps a 2 mm web (bounding boxes), so the thin plate does not break
    boxes = [(min(x for x, _ in p), min(y for _, y in p), max(x for x, _ in p), max(y for _, y in p)) for p in cut[1:]]
    for i, a in enumerate(boxes):
        for b in boxes[i + 1:]:
            gap = max(b[0] - a[2], a[0] - b[2], b[1] - a[3], a[1] - b[3])
            assert gap >= 2.0, ('web < 2 mm', a, b, gap)
    assert len(screws) >= 16, len(screws)


if __name__ == '__main__':
    template, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    cut, eng, faders, keys, holes, screws = build()
    check(cut, eng, screws)
    A.W, A.H = W, H
    n = A.write_dxf(template, os.path.join(out, 'cmdmx-paneel-graveerplaat.dxf'), cut, eng)
    A.write_svg(os.path.join(out, 'cmdmx-paneel-graveerplaat.svg'), cut, eng, 'CMDMX paneel graveerplaat')
    cutlen = sum(math.dist(p[i], p[(i + 1) % len(p)]) for p in cut for i in range(len(p)))
    print('faders', len(faders), 'keys', len(keys), 'holes', len(holes), 'screws', len(screws))
    print('cut shapes', len(cut), 'engrave shapes', len(eng), 'entities', n, 'cut length %.0f mm' % cutlen)
    print('screws', screws)
