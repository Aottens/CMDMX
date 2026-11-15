import adsk.core, adsk.fusion, math

# HWB-10 screen flap (HW-54, 56, 57, 58). Run through the Fusion MCP wrapper (cad/README.md) in a direct-modelling
# document that holds the Waveshare 15.6" HDMI LCD (H) STEP at the root, imported as is (occurrence
# 15_6INCH-HDMI-LCD_ASM:1, glass front at local z 5.82). ARGS = flap angle in degrees (0 = folded, 15/25/35/45).
# The first run copies the screen's bodies into a flat component 'M scherm' in the flap frame and removes the
# import: moving the imported assembly misplaces its nested driver-board occurrences (Fusion, 2026-11-15).
# Rebuilds three components and turns the screen with the flap:
#   K klep      flap parts in the flap frame: X across (0..570), Y from the hinge axis (0..250), Z from the
#               underside (0) to the face (24); the occurrence turns it about the hinge to ARGS degrees.
#   S strut     the two arms, foot rod and pivot pins, in desk coordinates for ARGS.
#   T testvoet  the printed test base: a slice of the deck with the hinge leaf and the sawtooth grooves.
# Desk coordinates: hinge axis along x at (y 345, z 13), deck face z 13. Then it checks every part against the
# screen and the strut against the base, and prints mass, centre of gravity and strut loads (calc.py).

P0 = adsk.core.Point3D.create
V = adsk.core.Vector3D.create
T = adsk.fusion.TemporaryBRepManager.get()
UNION = adsk.fusion.BooleanTypes.UnionBooleanType
DIFF = adsk.fusion.BooleanTypes.DifferenceBooleanType
INTER = adsk.fusion.BooleanTypes.IntersectionBooleanType

W, H, TH = 570.0, 250.0, 24.0          # flap between the walnut sides, length, thickness (HW-54: at most 25)
GL = (95.6, 474.4, 12.58, 237.42)      # cover glass outline in the flap frame (378.8 x 224.8, R 11.63)
GR = 11.63
G_FRONT, G_BACK = 23.62, 21.52         # glass 0.38 under the face: the 0.38 mm lip holds it (+-0.5, HW-54)
SCREEN_OFF = (285.0, 129.65, 17.80)    # screen STEP origin in the flap frame
BOT, TOP = 12.3, 237.7                 # walnut bottom and top bars end / start (0.3 mm from the glass)
CL = 78.0                              # cover A starts here; the left block's seat for it runs to the glass
TONGUE_Z = 20.8                        # clamp tongues: 1.0 mm silicone foam on them, squeezed to 0.72
PIN = (115.0, 9.0)                     # strut pivot in the flap frame (Y, Z)
ROD_FOLD = 210.0                       # folded, the foot rod lies at Y 210
ARM_X = ((66.0, 71.0), (499.0, 504.0))
INSERT_R, CLEAR_R = 2.0, 1.7           # M3 heat-set insert hole 4.0, clearance 3.4
HX0, HX1, LEAF = 72.0, 552.0, 0.7      # Duovorm black piano hinge 480 x 25 x 0.7, uncut


def p(x, y, z):
    return P0(x / 10, y / 10, z / 10)


def box(x0, x1, y0, y1, z0, z1):
    ob = adsk.core.OrientedBoundingBox3D.create(p((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2), V(1, 0, 0), V(0, 1, 0),
                                                (x1 - x0) / 10, (y1 - y0) / 10, (z1 - z0) / 10)
    return T.createBox(ob)


def cylx(x0, x1, y, z, r):
    return T.createCylinderOrCone(p(x0, y, z), r / 10, p(x1, y, z), r / 10)


def cylz(x, y, z0, z1, r):
    return T.createCylinderOrCone(p(x, y, z0), r / 10, p(x, y, z1), r / 10)


def U(a, *bs):
    for b in bs:
        assert T.booleanOperation(a, b, UNION)
    return a


def S(a, *bs):
    for b in bs:
        assert T.booleanOperation(a, b, DIFF)
    return a


def rrect(x0, x1, y0, y1, z0, z1, r):
    b = U(box(x0 + r, x1 - r, y0, y1, z0, z1), box(x0, x1, y0 + r, y1 - r, z0, z1))
    return U(b, *[cylz(x, y, z0, z1, r) for x in (x0 + r, x1 - r) for y in (y0 + r, y1 - r)])


def below_line(x0, x1, a, b, depth=40.0):
    """Prism under the line from a=(y,z) to b=(y,z), on its right side going a->b, for cutting ramps."""
    dy, dz = b[0] - a[0], b[1] - a[1]
    n = math.hypot(dy, dz)
    uy, uz = dy / n, dz / n
    ny, nz = uz, -uy                       # right-hand normal
    c = ((a[0] + b[0]) / 2 + ny * depth / 2, (a[1] + b[1]) / 2 + nz * depth / 2)
    ob = adsk.core.OrientedBoundingBox3D.create(p((x0 + x1) / 2, c[0], c[1]), V(0, uy, uz), V(1, 0, 0),
                                                (n + 2 * depth) / 10, (x1 - x0) / 10, depth / 10)
    return T.createBox(ob)


def run(_context: str):
    angle = float(ARGS)
    calc = {'__name__': 'calc'}
    exec(open(CAD + '/klep/build/calc.py').read(), calc)
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    assert des.designType == adsk.fusion.DesignTypes.DirectDesignType, 'run in a direct-modelling document'
    root = des.rootComponent
    if not any(o.component.name == 'M scherm' for o in root.occurrences):
        imp = next(o for o in root.occurrences if o.name.startswith('15_6INCH-HDMI-LCD_ASM'))
        assert imp.transform2.isEqualTo(adsk.core.Matrix3D.create()), 'import the screen STEP afresh'
        m_occ = root.occurrences.addNewComponent(adsk.core.Matrix3D.create())
        m_occ.component.name = 'M scherm'
        off = adsk.core.Matrix3D.create()
        off.translation = V(SCREEN_OFF[0] / 10, SCREEN_OFF[1] / 10, SCREEN_OFF[2] / 10)
        def grab(o):
            for b in o.bRepBodies:
                t = T.copy(b)
                assert T.transform(t, off)
                m_occ.component.bRepBodies.add(t).name = o.name.split('+')[-1].split(':')[0]
            for c in o.childOccurrences:
                grab(c)
        grab(imp)
        imp.deleteMe()
    screen = next(o for o in root.occurrences if o.component.name == 'M scherm')
    bb = screen.component.boundingBox
    assert abs(bb.maxPoint.z * 10 - G_FRONT) < 0.05 and bb.minPoint.z * 10 > 3, 'screen body out of place'
    for o in [o for o in root.occurrences if o.component.name in ('K klep', 'S strut', 'T testvoet')]:
        o.deleteMe()

    lib = app.materialLibraries.itemByName('Fusion Appearance Library')
    def look(name):
        a = des.appearances.itemByName(name)
        if a is None and lib is not None and lib.appearances.itemByName(name) is not None:
            a = des.appearances.addByCopy(lib.appearances.itemByName(name), name)
        return a
    looks = {'print': look('Plastic - Matte (Black)'), 'walnut': look('Walnut'),
             'alu': look('Aluminum - Satin'), 'steel': look('Steel - Satin')}

    def comp(name):
        occ = root.occurrences.addNewComponent(adsk.core.Matrix3D.create())
        occ.component.name = name
        return occ

    def add(c, name, body):
        b = c.bRepBodies.add(body)
        b.name = name
        kind = name[name.rfind('(') + 1:-1]
        if looks.get(kind) is not None:
            b.appearance = looks[kind]
        return b

    # ------------------------------------------------------------------ K klep (flap frame)
    k_occ = comp('K klep')
    K = k_occ.component

    # walnut frame, glued and dowelled at the corners; 5 x 3 rebates take the printed parts' flanges from behind
    add(K, 'K walnut side L (walnut)', S(box(-12, 0, 0, H, 0, TH), box(-5, 0, BOT, TOP, 0, 3)))
    add(K, 'K walnut side R (walnut)', S(box(W, W + 12, 0, H, 0, TH), box(W, W + 5, BOT, TOP, 0, 3)))
    add(K, 'K walnut top (walnut)', S(box(0, W, TOP, H, 0, TH), box(0, W, TOP, TOP + 5, 0, 3)))
    add(K, 'K walnut bottom (walnut)', S(box(0, W, 0, BOT, 0, TH),
                                         box(95.3, 474.7, BOT - 3, BOT, 3.3, 5.7),   # groove for the covers' tongue
                                         box(HX0, HX1, 0, BOT, 0, LEAF),             # hinge leaf mortise
                                         cylx(HX0, HX1, 0, 0, 2.4),                  # knuckle cove
                                         cylx(14, 54, 0, 0, 6.5),                    # cables run along the axis
                                         box(40, 54, 0, BOT, 0, 9)))                 # and drop in from the block

    def side_block(right):
        m = (lambda x: W - x) if right else (lambda x: x)
        def bx(x0, x1, *r):
            a, b = sorted((m(x0), m(x1)))
            return box(a, b, *r)
        def cx(x0, x1, *r):
            a, b = sorted((m(x0), m(x1)))
            return cylx(a, b, *r)
        def cz(x, *r):
            return cylz(m(x), *r)
        b = U(bx(0, 95.3, BOT, TOP, 0, TH),
              bx(95.3, 107.2, BOT, TOP, G_BACK, TH),                 # face over the glass corners and the lip
              bx(-5, 0, BOT, TOP, 0, 3),                             # flange in the side rebate
              bx(0, CL, TOP, TOP + 5, 0, 3))                         # flange in the top rebate
        S(b, rrect(GL[0] - 0.3, GL[1] + 0.3, GL[2] - 0.3, GL[3] + 0.3, G_BACK - 0.1, G_FRONT, GR + 0.3),  # glass pocket
          rrect(GL[0] + 1.5, GL[1] - 1.5, GL[2] + 1.5, GL[3] - 1.5, G_FRONT - 0.1, TH + 0.1, GR - 1.5),   # window
          bx(CL, 95.3, BOT, TOP, 0, 3),                              # seat for the back cover's edge
          bx(60, 77, 106, 222, 0, 17.5), bx(60, 77, 98, 106, 0, 8),  # strut pocket, deeper where the arm leaves at 45
          cx(50.3, 95.3, PIN[0], PIN[1], 3.0),                       # 6 mm pivot pin
          cx(78, 84, 123, 6, INSERT_R),                              # pin retainer screw
          bx(84, 95.3, 13, 52, 0, 14), bx(84, 95.3, 110, 231.2, 0, 14),   # clamp strip channels
          bx(77, 95.3, ROD_FOLD - 6, ROD_FOLD + 6, 0, 14),           # folded foot rod
          bx(87, 95.3, 231.7, 237.4, 0, TONGUE_Z),                   # top clamp bar end
          bx(78, 95.3, 231.2, 237.4, 3, 5.8),                        # its keeper
          cz(83.5, 234.3, 5.8, 10.8, INSERT_R),
          *[cz(89.5, y, 14, 19, INSERT_R) for y in (20, 44, 130, 175, 224)],   # strip screws
          *[cz(81, y, 3, 9, INSERT_R) for y in (30, 160)],           # cover screws
          *[cz(-2.5, y, -1, 4, CLEAR_R) for y in (40, 125, 210)],    # wood screws into the walnut
          cz(40, TOP + 2.5, -1, 4, CLEAR_R))
        if not right:   # cables: plug channel from the driver board, cavity, clamp, route to the hinge notch
            S(b, box(12, 58, 34, 102, 2.5, 18), box(58, 76, 34, 95, 2.5, 18), box(12, 76, BOT, 34, 2.5, 10),
              box(76, 95.3, 50, 106, 2, 18),
              box(40, 54, BOT, 14.5, 0, 9),
              box(4, 58, 14, 109, 0, 2.5), box(58, 78, 14, 96, 0, 2.5),        # lid seat
              *[cylz(x, y, 2.5, 8, INSERT_R) for x, y in ((8, 25), (8, 95), (30, 106), (55, 106))],
              *[cylz(x, 23, 10, 15, INSERT_R) for x in (19, 55)])
        return b

    add(K, 'K block L (print)', side_block(False))
    add(K, 'K block R (print)', side_block(True))

    def cover(x0, x1, screws_x, flange_screws):
        inner0, inner1 = max(x0, 95.3), min(x1, 474.7)
        b = U(box(x0, x1, BOT, TOP + 5, 0, 3),
              box(inner0, inner1, BOT, BOT + 2.7, 3, 6), box(inner0, inner1, BOT - 2.8, BOT, 3.5, 5.5),  # tongue
              box(inner0, inner1, ROD_FOLD - 7.5, ROD_FOLD + 7.5, 0, 16),                                # rod channel
              *[box(inner0, inner1, y, y + 2, 3, 6) for y in (120, 160)])                               # ribs
        S(b, box(x0 - 1, x1 + 1, ROD_FOLD - 6, ROD_FOLD + 6, -1, 14.5),
          *[cylz(x, y, -1, 7, CLEAR_R) for x in screws_x for y in (30, 160)],
          *[cylz(x, TOP + 2.5, -1, 4, CLEAR_R) for x in flange_screws])
        return b

    cov_a = S(cover(CL, 330, (81,), (120, 205, 290)),
              *[box(x, x + 4, 62, 102, -1, 4) for x in range(130, 303, 12)],         # vents under the driver board
              box(CL - 1, 124, 50, 106, 2, 4))                                         # 2 mm under the plugs
    add(K, 'K cover A (print)', cov_a)
    add(K, 'K cover B (print)', cover(330, W - CL, (W - 81,), (370, 450)))
    add(K, 'K cable lid (print)', S(U(box(4, 58, 14, 109, 0, 2.5), box(58, 78, 14, 96, 0, 2.5)),
                                    *[cylz(x, y, -1, 4, CLEAR_R) for x, y in ((8, 25), (8, 95), (30, 106), (55, 106))]))
    add(K, 'K cable clamp (print)', S(box(14, 60, 18, 28, 2.6, 5.0), *[cylz(x, 23, 2, 6, CLEAR_R) for x in (19, 55)]))

    def strip(x_plate, x_tongue, y0, y1, holes_x, holes_y, arm=None, rod_gap=False):
        b = U(box(x_plate[0], x_plate[1], y0, y1, 11, 14), box(x_tongue[0], x_tongue[1], y0, y1, 14, TONGUE_Z))
        if arm:
            U(b, box(arm[0], arm[1], 14, 23.5, 11, TONGUE_Z))
        if rod_gap:
            S(b, box(x_plate[0] - 1, x_plate[1] + 1, ROD_FOLD - 9, ROD_FOLD + 9, 10, 16.5))
        return S(b, *[cylz(holes_x, y, 10, 15, CLEAR_R) for y in holes_y])

    add(K, 'K clamp L low (print)', strip((84.5, 106.4), (96.6, 106.4), 14, 50, 89.5, (20, 44), arm=(96.6, 143)))
    add(K, 'K clamp L high (print)', strip((84.5, 106.4), (96.6, 106.4), 120, 230, 89.5, (130, 175, 224), rod_gap=True))
    add(K, 'K clamp R low (print)', strip((468.0, 485.5), (468.0, 473.4), 14, 50, W - 89.5, (20, 44), arm=(427, 473.4)))
    add(K, 'K clamp R high (print)', strip((468.0, 485.5), (468.0, 473.4), 120, 230, W - 89.5, (130, 175, 224), rod_gap=True))
    add(K, 'K clamp top 15x5 (alu)', box(87, W - 87, 232, 237, 5.8, TONGUE_Z))
    add(K, 'K keeper L (print)', S(box(78, 95.3, 231.2, 237.4, 3, 5.8), cylz(83.5, 234.3, 2, 7, CLEAR_R)))
    add(K, 'K keeper R (print)', S(box(W - 95.3, W - 78, 231.2, 237.4, 3, 5.8), cylz(W - 83.5, 234.3, 2, 7, CLEAR_R)))
    add(K, 'K pivot pin L (steel)', cylx(50.3, 95.3, PIN[0], PIN[1], 3.0))
    add(K, 'K pivot pin R (steel)', cylx(W - 95.3, W - 50.3, PIN[0], PIN[1], 3.0))
    add(K, 'K hinge leaf + knuckle (steel)', U(box(HX0, HX1, 1.6, BOT, 0, LEAF), cylx(HX0, HX1, 0, 0, 1.6)))

    r = math.radians(angle)
    flap = adsk.core.Matrix3D.create()
    flap.translation = V(0, 34.5, 1.3)
    rot = adsk.core.Matrix3D.create()
    rot.setToRotation(r, V(1, 0, 0), p(0, 345, 13))
    flap.transformBy(rot)
    k_occ.transform2 = flap
    screen.transform2 = flap

    def world(y, z):   # flap frame (Y, Z) -> desk (y, z)
        return 345 + y * math.cos(r) - z * math.sin(r), 13 + y * math.sin(r) + z * math.cos(r)

    # ------------------------------------------------------------------ S strut (desk coordinates)
    s_occ = comp('S strut')
    SC = s_occ.component
    if angle == 0:
        pv, ft = world(*PIN), world(ROD_FOLD, PIN[1])
    else:
        pv, ft = calc['pivot'](angle), calc['foot'](angle)
    L = math.hypot(ft[0] - pv[0], ft[1] - pv[1])
    assert abs(L - calc['L']) < 0.01, L
    for i, (x0, x1) in enumerate(ARM_X):
        uy, uz = (ft[0] - pv[0]) / L, (ft[1] - pv[1]) / L
        ob = adsk.core.OrientedBoundingBox3D.create(p((x0 + x1) / 2, (pv[0] + ft[0]) / 2, (pv[1] + ft[1]) / 2),
                                                    V(0, uy, uz), V(1, 0, 0), L / 10, (x1 - x0) / 10, 1.5)
        arm = U(T.createBox(ob), cylx(x0, x1, pv[0], pv[1], 7.5), cylx(x0, x1, ft[0], ft[1], 7.5))
        S(arm, cylx(x0 - 1, x1 + 1, pv[0], pv[1], 4.0), cylx(x0 - 1, x1 + 1, ft[0], ft[1], 4.0))   # 8 mm bushing bores
        add(SC, 'S arm %s 15x5 (alu)' % 'LR'[i], arm)
    add(SC, 'S foot rod 8 (steel)', cylx(60, 510, ft[0], ft[1], 4.0))

    # ------------------------------------------------------------------ T testvoet (desk coordinates)
    t_occ = comp('T testvoet')
    TC = t_occ.component
    base = box(0, W, 320, 551, 0, 13)
    S(base, box(HX0, HX1, 332.7, 345, 13 - LEAF, 13.1), cylx(HX0, HX1, 345, 13, 2.4),    # hinge leaf, knuckle
      cylx(14, 54, 345, 13, 6.5), box(14, 26, 339, 351, -1, 14),                         # cable run on the axis, slot down
      box(58, 77, 440, 552, 3, 14), box(W - 77, W - 58, 440, 552, 3, 14))                # arm channels
    for a in calc['STEPS']:
        f = calc['foot'](a)[0]
        tooth = U(box(56, 514, f - 4, f + 4.3, 8, 14), box(56, 514, f - 14, f - 4, 8, 14))
        S(tooth, below_line(55, 515, (f - 14, 13), (f - 4, 8)))                       # 10 mm lead-in ramp
        S(base, tooth)                                    # steep face behind the rod: a push cannot move it back
    S(base, *[cylz(x, y, -1, 6, INSERT_R) for x in (175, 205, 365, 395) for y in (360, 520)],   # splice bars
      *[cylz(x, 362, -1, 6, INSERT_R) for x in (10, 32)])                                     # cable clamp
    for i, (x0, x1) in enumerate(((0, 190), (190, 380), (380, W))):
        piece = T.copy(base)
        assert T.booleanOperation(piece, box(x0, x1, 300, 560, -1, 20), INTER)
        add(TC, 'T base %d (print)' % (i + 1), piece)
    for x in (190, 380):
        add(TC, 'T splice %d (print)' % x, S(box(x - 25, x + 25, 340, 540, -6, 0),
                                             *[cylz(x + dx, y, -7, 1, CLEAR_R) for dx in (-15, 15) for y in (360, 520)]))
    add(TC, 'T cable clamp (print)', S(box(6, 36, 356, 368, -5, 0), *[cylz(x, 362, -6, 1, CLEAR_R) for x in (10, 32)]))
    add(TC, 'T hinge leaf (steel)', box(HX0, HX1, 332.7, 343.4, 13 - LEAF, 13))

    # ------------------------------------------------------------------ checks
    def solids(occ):
        return [(b.name, T.copy(b.createForAssemblyContext(occ))) for b in occ.component.bRepBodies]
    scr = [(b.name, T.copy(b.createForAssemblyContext(screen))) for b in screen.component.bRepBodies]
    def clash(xs, ys, tol=0.02):   # cm3; 0.02 cm3 = 20 mm3 allows touching faces
        out = []
        for na, a in xs:
            for nb, b in ys:
                ba, bb = a.boundingBox, b.boundingBox
                if not ba.intersects(bb):
                    continue
                c = T.copy(a)
                if T.booleanOperation(c, b, INTER) and c.volume > tol:
                    out.append('%s x %s %.0f mm3' % (na, nb, c.volume * 1000))
        return out
    K_s, S_s, T_s = solids(k_occ), solids(s_occ), solids(t_occ)
    plugs = [('plug envelope', box(78, 118, 72, 103, 2.5, 14))]   # HDMI overmould 21 x 11, DC and micro-USB beside it
    found = clash(plugs, [(b.name, T.copy(b)) for b in K.bRepBodies]) + clash(scr, K_s + S_s) + clash(S_s, T_s) + clash(K_s, T_s) + clash(S_s, [x for x in K_s if '(print)' in x[0] or 'walnut' in x[0]])
    for i, (na, a) in enumerate(K_s):
        for nb, b in K_s[i + 1:]:
            found += clash([(na, a)], [(nb, b)])
    for f in found:
        print('CLASH', f)
    assert TH <= 25 and abs(TH - G_FRONT) <= 0.5                       # HW-54: 25 mm shell, glass flush +-0.5
    for c in (K, TC):                                                  # HW-58: every printed part fits 256^3
        for b in c.bRepBodies:
            bb = b.boundingBox
            dims = sorted((bb.maxPoint.x - bb.minPoint.x, bb.maxPoint.y - bb.minPoint.y, bb.maxPoint.z - bb.minPoint.z))
            if '(print)' in b.name:
                assert dims[2] * 10 <= 256, (b.name, dims)
            if '(walnut)' in b.name or '(alu)' in b.name:
                print('  cut %-28s %s mm' % (b.name, ' x '.join('%.1f' % (d * 10) for d in reversed(dims))))

    dens = {'print': 0.6, 'walnut': 0.65, 'alu': 2.7, 'steel': 7.9}   # print: PETG at about 45 % fill
    mass, my, mz = 1.0, 1.0 * 125.0, 1.0 * 12.0                         # module 1.0 kg, centred in the glass
    for b in K.bRepBodies:
        kind = b.name[b.name.rfind('(') + 1:-1]
        m = b.volume * dens[kind] / 1000
        c = b.physicalProperties.centerOfMass
        mass, my, mz = mass + m, my + m * c.y * 10, mz + m * c.z * 10
    cgy, cgz = my / mass, mz / mass
    print('angle %g  flap %.2f kg  cg Y %.0f Z %.0f' % (angle, mass, cgy, cgz))
    for a in calc['STEPS']:
        f = calc['strut_force'](a, mass, cgy, cgz)
        print('  %2d deg foot y %.1f strut %.0f N rack %.0f N back' % (a, calc['foot'](a)[0], f, calc['rod_push'](a, f)[0]))
    print('clashes', len(found))
