"""Printed backing for the 1.5 mm test strip (HW-50): how far the top plate and the backing give under a 20 N push.

Plain Python 3, no Fusion. `python3 calc.py` prints the table and asserts the numbers the build relies on.
onderframe.py execs this file for PITCH, RIB, H, SKIN and RAIL, so the rib spacing is chosen here, on these numbers.

1. The plate between ribs: a rectangular cell, simply supported on its ribs, 20 N on a point in the middle (Navier
   series). The plate runs on over the ribs into the next cell, which holds its edges partly, so the real value lies
   between this and the clamped one, about half (Timoshenko: 0.0116 against 0.0056 P a^2/D for a square).
2. The backing as a beam: the strip on two supports 170 mm apart (its short ends on two blocks, as in the test), 20 N
   in the middle. The plate is screwed only at its corners and slides on the ribs, so it adds nothing here.
"""
import math

P = 20.0                 # N: a hard press with one finger (about 2 kg on a kitchen scale)
NU = 0.35
PLATES = {               # name: (thickness mm, E MPa)
    '1.5 graveerplaat, ABS-kern (laag)': (1.5, 2000.0),
    '1.5 graveerplaat, acryl-kern': (1.5, 3000.0),
    '2 mm gegoten acryl': (2.0, 3200.0),
    '1.5 aluminium (gewone lichttafel)': (1.5, 70000.0),
}
DESIGN = '1.5 graveerplaat, ABS-kern (laag)'   # the rib spacing is chosen for the weakest reading of the plate
LIMIT = 0.10             # mm under 20 N, simply supported bound: a push you cannot feel give

# backing (onderframe.py builds with these)
PITCH = 16.5             # largest clear side of an open cell, mm; half the desk's 33 mm fader pitch
RIB = 1.6                # rib width: four 0.4 mm lines
WALL = 2.4               # outer wall
H = 13.0                 # depth of ribs and walls, as the printed panel (3 mm skin + 10 mm ribs)
SKIN = 2.0               # closed variant: skin under the whole plate
RAIL = 2.7               # fader seat: ISO 7380 head 1.65 in a 1.75 counterbore, 0.95 mm under it
E_PRINT = 1800.0         # PETG as printed, along the lines, MPa
SPAN = 170.0             # test supports under the short ends


def rigidity(t, e):
    return e * t ** 3 / (12 * (1 - NU * NU))


def cell_ss(a, b, d, terms=401):
    """Centre deflection of a simply supported a x b plate under P at its centre (Navier double series)."""
    s = 0.0
    for m in range(1, terms + 1, 2):
        for n in range(1, terms + 1, 2):
            s += 1.0 / ((m / a) ** 2 + (n / b) ** 2) ** 2
    return 4 * P * s / (math.pi ** 4 * a * b * d)


def section_i(rects):
    """Second moment of area about the neutral axis; rects are (width, z0, z1), z down from the plate face."""
    a = sum(w * (z1 - z0) for w, z0, z1 in rects)
    zc = sum(w * (z1 - z0) * (z0 + z1) / 2 for w, z0, z1 in rects) / a
    return sum(w * (z1 - z0) ** 3 / 12 + w * (z1 - z0) * ((z0 + z1) / 2 - zc) ** 2 for w, z0, z1 in rects)


def beam(rects):
    return P * SPAN ** 3 / (48 * E_PRINT * section_i(rects))


# cross-section at y 95 (mid-span), from onderframe.py: two outer walls, three lattice ribs along y, two walls per
# fader channel, and the rails on top of the channels (16 and 12 wide, minus the 5.0 and 4.0 lever slots).
FULL = [(2 * WALL, 0, H), (3 * RIB, 0, H), (4 * RIB, 0, H), (16.0 - 5.0, 0, RAIL), (12.0 - 4.0, 0, RAIL)]
OPEN_WIDTH = 120.0 - 2 * WALL - 7 * RIB - 16.0 - 12.0


if __name__ == '__main__':
    k_sq = cell_ss(1.0, 1.0, 1.0) / P
    assert abs(k_sq - 0.0116) < 0.0002, k_sq          # Timoshenko's square plate, point load: 0.0116 P a^2 / D
    print('plate in a square cell of side s, 20 N in the middle (simply supported; clamped is about half):')
    print('%-36s %7s %7s %7s %7s' % ('', 's=12', 's=16.5', 's=25', 's=33'))
    for name, (t, e) in PLATES.items():
        d = rigidity(t, e)
        print('%-36s' % name + ''.join(' %6.3f ' % cell_ss(s, s, d) for s in (12, 16.5, 25, 33)) + ' mm')
    d = rigidity(*PLATES[DESIGN])
    w = cell_ss(PITCH, PITCH, d)
    s_max = PITCH * math.sqrt(LIMIT / w)
    print('\nchosen cell %.1f mm: %.3f mm (limit %.2f; the limit allows %.1f mm)' % (PITCH, w, LIMIT, s_max))
    assert w <= LIMIT, w
    long_cell = cell_ss(PITCH, 3 * PITCH, d)
    print('long cell %.1f x %.1f: %.3f mm, so the ribs run both ways' % (PITCH, 3 * PITCH, long_cell))
    assert long_cell > LIMIT
    print('old printed panel (3 mm PETG skin, 33 mm lattice): %.3f mm' % cell_ss(33, 33, rigidity(3.0, E_PRINT)))

    skin = FULL + [(OPEN_WIDTH, 0, SKIN)]
    wa, wb = beam(FULL), beam(skin)
    print('\nbacking as a beam, %d mm span, 20 N in the middle: ribs only %.2f mm, with %.0f mm skin %.2f mm'
          % (SPAN, wa, SKIN, wb))
    assert wb < wa
    # stress in the plate at a fingertip (r0 = 4 mm) in the chosen cell: well under ABS's 40 MPa
    t = PLATES[DESIGN][0]
    sigma = 3 * P * (1 + NU) / (2 * math.pi * t * t) * math.log(PITCH / 2 / 4.0) + 3 * P / (2 * math.pi * t * t)
    print('plate stress under the finger about %.0f MPa' % sigma)
    assert sigma < 20
    print('ok')
