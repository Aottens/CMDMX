"""Acrylic test strip for Laserlokaal: one layout, two materials.

red (ACI 1) = cut, blue (ACI 5) = engrave (closed shapes, even-odd fill).
Writes DXF (mm, Fusion-exported AC1014 file as template) and an SVG preview per material.
Usage: python3 acryl_strip.py <template.dxf> <out_dir>
"""
import math, sys, os
from fontTools.ttLib import TTFont
from fontTools.pens.basePen import BasePen

W, H = 120.0, 200.0
FONT = '/System/Library/Fonts/Supplemental/Arial.ttf'
BOLD = '/System/Library/Fonts/Supplemental/Arial Bold.ttf'


class FlatPen(BasePen):
    def __init__(self, gs):
        super().__init__(gs)
        self.contours, self.cur = [], []

    def _moveTo(self, p):
        self.cur = [p]

    def _lineTo(self, p):
        self.cur.append(p)

    def _curveToOne(self, p1, p2, p3):
        p0 = self.cur[-1]
        for i in range(1, 9):
            t = i / 8
            self.cur.append(tuple((1 - t) ** 3 * a + 3 * (1 - t) ** 2 * t * b + 3 * (1 - t) * t * t * c + t ** 3 * d
                                  for a, b, c, d in zip(p0, p1, p2, p3)))

    def _qCurveToOne(self, p1, p2):
        p0 = self.cur[-1]
        for i in range(1, 7):
            t = i / 6
            self.cur.append(tuple((1 - t) ** 2 * a + 2 * (1 - t) * t * b + t * t * c for a, b, c in zip(p0, p1, p2)))

    def _closePath(self):
        if len(self.cur) > 2:
            self.contours.append(self.cur)
        self.cur = []

    _endPath = _closePath


_fonts = {}


def text(s, x, y, cap, bold=False, anchor='l'):
    """Outline polygons of s, baseline at y, cap height cap mm; anchor l/c/r on x."""
    path = BOLD if bold else FONT
    f = _fonts.get(path) or _fonts.setdefault(path, TTFont(path))
    gs, cmap, hmtx = f.getGlyphSet(), f.getBestCmap(), f['hmtx']
    k = cap / f['OS/2'].sCapHeight
    width = sum(hmtx[cmap[ord(ch)]][0] for ch in s) * k
    x0 = x - {'l': 0, 'c': width / 2, 'r': width}[anchor]
    out, adv = [], 0.0
    for ch in s:
        g = cmap[ord(ch)]
        pen = FlatPen(gs)
        gs[g].draw(pen)
        out += [[(x0 + adv + px * k, y + py * k) for px, py in c] for c in pen.contours]
        adv += hmtx[g][0] * k
    return out


def rrect(x1, y1, x2, y2, r, n=6):
    pts = []
    for cx, cy, a0 in ((x2 - r, y1 + r, -90), (x2 - r, y2 - r, 0), (x1 + r, y2 - r, 90), (x1 + r, y1 + r, 180)):
        pts += [(cx + r * math.cos(math.radians(a0 + 90 * i / n)), cy + r * math.sin(math.radians(a0 + 90 * i / n)))
                for i in range(n + 1)]
    return pts


def rect(x1, y1, x2, y2):
    return [(x1, y1), (x2, y1), (x2, y2), (x1, y2)]


def slot(cx, y1, y2, w):
    return rrect(cx - w / 2, y1, cx + w / 2, y2, w / 2 - 0.001)


def circle(cx, cy, d, n=48):
    return [(cx + d / 2 * math.cos(2 * math.pi * i / n), cy + d / 2 * math.sin(2 * math.pi * i / n)) for i in range(n)]


def layout(material):
    cut, eng = [], []
    cut.append(rrect(0, 0, W, H, 3))
    for x, y in ((6, 6), (W - 6, 6), (6, H - 6), (W - 6, H - 6)):
        cut.append(circle(x, y, 3.2))

    # fader 1: Bourns PSM 100 mm (slot 3.4 x 109), fader 2: Bourns PTA6043 60 mm (slot 2.4 x 64.6)
    for fx, sw, sl, travel, num in ((25, 3.4, 109.0, 100.0, '1'), (60, 2.4, 64.6, 60.0, '2')):
        yc = 95.0
        cut.append(slot(fx, yc - sl / 2, yc + sl / 2, sw))
        for i in range(11):
            ty = yc - travel / 2 + travel * i / 10
            major = i % 5 == 0
            eng.append(rect(fx - 12 if major else fx - 9, ty - 0.25, fx - 5, ty + 0.25))
            if major:
                eng += text(str(i), fx + 6, ty - 1.5, 3.0)
        eng += text(num, fx, 20, 8.0, bold=True, anchor='c')

    # separator line under the key area, as on the printed strip
    eng.append(rect(8, 154.75, W - 8, 155.25))

    if material == '3mm':
        # MX keys cannot clip into 3 mm: the cap needs a window. Two-key window (19.05 pitch) and one single.
        cut.append(rrect(14, 167, 14 + 19.05 + 20.2, 187.2, 1.0))
        cut.append(rrect(72, 167, 92.2, 187.2, 1.0))
        eng += text('2 toetsen', 14, 160, 2.5)
        eng += text('1 toets', 72, 160, 2.5)
        label = 'mat zwart acryl 3 mm'
    else:
        # MX clips into 1.5 mm; kerf ~0.2 mm, so try three drawn sizes
        for x, s in ((20, 13.8), (50, 13.9), (80, 14.0)):
            cut.append(rect(x, 170, x + s, 170 + s))
            eng += text('%.1f' % s, x + s / 2, 162, 2.5, anchor='c')
        label = 'graveerplaat zwart/wit 1,5 mm'

    # legend size test, right column
    y = 146.0
    for cap in (2.0, 2.5, 3.0, 4.0, 5.0):
        eng += text('Go %.1f' % cap, 82, y, cap)
        y -= cap + 5.5
    eng += text('PAGE', 82, y, 3.0, bold=True)
    y -= 9
    # line-width test
    for lw in (0.2, 0.3, 0.5, 0.8):
        eng.append(rect(82, y - lw / 2, 100, y + lw / 2))
        eng += text('%.1f' % lw, 103, y - 1, 2.0)
        y -= 5
    eng.append(rect(82, y - 6, 88, y))       # flat area
    eng += text('vlak', 91, y - 4.5, 2.0)
    # encoder (7.2) and wheel (10.2) holes
    cut.append(circle(88, 32, 7.2))
    cut.append(circle(106, 32, 10.2))
    eng += text('7.2', 88, 24, 2.0, anchor='c')
    eng += text('10.2', 106, 24, 2.0, anchor='c')
    eng += text(label, 96, 10, 2.0, anchor='c')
    eng += text('CMDMX', 96, 14, 2.5, bold=True, anchor='c')
    return cut, eng


def write_dxf(template, path, cut, eng):
    lines = [l.rstrip('\r\n') for l in open(template)]
    pairs = list(zip(lines[0::2], lines[1::2]))
    i = next(k for k, (c, v) in enumerate(pairs) if c.strip() == '2' and v.strip() == 'ENTITIES')
    j = next(k for k in range(i, len(pairs)) if pairs[k][0].strip() == '0' and pairs[k][1].strip() == 'ENDSEC')
    out = pairs[:i + 1]
    h = 0x3000
    for colour, polys in ((1, cut), (5, eng)):
        for poly in polys:
            out += [('0', 'LWPOLYLINE'), ('5', '%X' % h), ('100', 'AcDbEntity'), ('8', '0'), ('62', str(colour)),
                    ('100', 'AcDbPolyline'), ('90', str(len(poly))), ('70', '1'), ('43', '0.0')]
            for x, y in poly:
                out += [('10', '%.4f' % x), ('20', '%.4f' % y)]
            h += 1
    out += pairs[j:]
    with open(path, 'w') as f:
        for c, v in out:
            f.write('%s\n%s\n' % (c, v))
    return h - 0x3000


def write_svg(path, cut, eng, title):
    def d(polys):
        return ' '.join('M' + ' L'.join('%.3f,%.3f' % (x, H - y) for x, y in p) + ' Z' for p in polys)
    with open(path, 'w') as f:
        f.write('<svg xmlns="http://www.w3.org/2000/svg" width="%gmm" height="%gmm" viewBox="0 0 %g %g">\n'
                '<title>%s</title>\n' % (W, H, W, H, title))
        f.write('<path d="%s" fill="#1a1a1a" fill-rule="evenodd" stroke="#e00" stroke-width="0.2"/>\n' % d(cut))
        f.write('<path d="%s" fill="#eee" fill-rule="evenodd"/>\n</svg>\n' % d(eng))


def check(cut, eng):
    """Smallest self-check: everything inside the strip, every polygon closed-able (>=3 points)."""
    for p in cut + eng:
        assert len(p) >= 3
        for x, y in p:
            assert -0.01 <= x <= W + 0.01 and -0.01 <= y <= H + 0.01, (x, y)


if __name__ == '__main__':
    template, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    for mat in ('3mm', '1.5mm'):
        cut, eng = layout(mat)
        check(cut, eng)
        n = write_dxf(template, os.path.join(out, 'cmdmx-acryl-proef-%s.dxf' % mat), cut, eng)
        write_svg(os.path.join(out, 'cmdmx-acryl-proef-%s.svg' % mat), cut, eng, 'CMDMX acryl-proef %s' % mat)
        print(mat, 'cut', len(cut), 'engrave', len(eng), 'entities', n)
