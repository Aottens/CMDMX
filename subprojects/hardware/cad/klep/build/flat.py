"""Flat parts of the HWB-10 flap as DXF (R12, mm), plain Python 3: python3 build/flat.py

cmdmx-klep-arm.dxf    strut arm, 15 x 5 aluminium: R 7.5 ends, two 8.0 holes 95.0 apart (klep.py: ARM_X, calc.L)
cmdmx-klep-dummy.dxf  stand-in for the screen's cover glass, 2 mm black acrylic: the glass outline
                      378.8 x 224.8, R 11.63 (klep.py: GL, GR), so the flap can be built and its strut tested
                      before the screen is bought
"""
import math
import os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
ARM_L, ARM_R, HOLE_R = 95.0, 7.5, 4.0
GLASS_W, GLASS_H, GLASS_R = 378.8, 224.8, 11.63


def dxf(ents):
    head = ['0', 'SECTION', '2', 'HEADER', '9', '$ACADVER', '1', 'AC1009', '9', '$INSUNITS', '70', '4',
            '0', 'ENDSEC', '0', 'SECTION', '2', 'ENTITIES']
    return '\n'.join(head + [str(v) for e in ents for v in e] + ['0', 'ENDSEC', '0', 'EOF', ''])


def line(x0, y0, x1, y1):
    return ('0', 'LINE', '8', '0', '10', x0, '20', y0, '11', x1, '21', y1)


def arc(x, y, r, a0, a1):      # counter-clockwise from a0 to a1, degrees
    return ('0', 'ARC', '8', '0', '10', x, '20', y, '40', r, '50', a0, '51', a1)


def circle(x, y, r):
    return ('0', 'CIRCLE', '8', '0', '10', x, '20', y, '40', r)


def arm():
    return [line(0, -ARM_R, ARM_L, -ARM_R), arc(ARM_L, 0, ARM_R, 270, 90), line(ARM_L, ARM_R, 0, ARM_R),
            arc(0, 0, ARM_R, 90, 270), circle(0, 0, HOLE_R), circle(ARM_L, 0, HOLE_R)]


def rrect(w, h, r):
    x, y = w / 2 - r, h / 2 - r
    return [line(-x, -h / 2, x, -h / 2), arc(x, -y, r, 270, 360), line(w / 2, -y, w / 2, y), arc(x, y, r, 0, 90),
            line(x, h / 2, -x, h / 2), arc(-x, y, r, 90, 180), line(-w / 2, y, -w / 2, -y), arc(-x, -y, r, 180, 270)]


def ends(e):                   # start and end point of a LINE or ARC
    if e[1] == 'LINE':
        return (e[5], e[7]), (e[9], e[11])
    x, y, r, a0, a1 = e[5], e[7], e[9], math.radians(e[11]), math.radians(e[13])
    return (x + r * math.cos(a0), y + r * math.sin(a0)), (x + r * math.cos(a1), y + r * math.sin(a1))


def closed(ents):              # each piece starts where the one before it ends
    loop = [ends(e) for e in ents if e[1] != 'CIRCLE']
    return all(math.dist(loop[i - 1][1], loop[i][0]) < 1e-6 for i in range(len(loop)))


if __name__ == '__main__':
    for name, ents in (('arm', arm()), ('dummy', rrect(GLASS_W, GLASS_H, GLASS_R))):
        assert closed(ents), name
        with open(os.path.join(OUT, 'cmdmx-klep-%s.dxf' % name), 'w') as f:
            f.write(dxf(ents))
    print('ok')
