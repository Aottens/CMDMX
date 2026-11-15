import adsk.core, adsk.fusion, math

# Two screen-stand studies on M1, flap at 25 degrees, hinge at (y 345, z 13).
# SA: brushed quadrant on each flap side, slotted on an arc round the hinge, knob in the body side.
# SB: fold-out strut under the flap, its foot in a notched rack on the tray.
VI = adsk.core.ValueInput.createByReal
P = adsk.core.Point3D.create
NEW = adsk.fusion.FeatureOperations.NewBodyFeatureOperation
CUT = adsk.fusion.FeatureOperations.CutFeatureOperation
W = 570.0
HY, HZ = 345.0, 13.0
A = 25.0


def mm(v):
    return v / 10.0


def rot(y, z, deg):
    """Rotate a (y, z) point about the hinge by deg."""
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    dy, dz = y - HY, z - HZ
    return HY + dy * c - dz * s, HZ + dy * s + dz * c


def polar(r, deg):
    return HY + r * math.cos(math.radians(deg)), HZ + r * math.sin(math.radians(deg))


def run(_context: str):
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    root = des.rootComponent
    assert not [b for b in root.bRepBodies if b.name.startswith(('SA ', 'SB '))]
    fb = root.bRepBodies.itemByName('B flap body (print)').boundingBox
    top = HZ + 250 * math.sin(math.radians(A)) + 20 * math.cos(math.radians(A))
    assert abs(fb.maxPoint.z * 10 - top) < 0.5, ('flap not at 25 deg', fb.maxPoint.z * 10, top)
    feats = root.features
    made = []

    def offplane(base, d):
        pi = root.constructionPlanes.createInput()
        pi.setByOffset(base, VI(mm(d)))
        pl = root.constructionPlanes.add(pi)
        pl.isLightBulbOn = False
        return pl

    def prof(name, x0, thick, pts, op=NEW, target=None):
        s = root.sketches.add(offplane(root.yZConstructionPlane, x0)); s.isVisible = False
        q = [s.modelToSketchSpace(P(mm(x0), mm(y), mm(z))) for y, z in pts]
        L = s.sketchCurves.sketchLines
        first = prev = L.addByTwoPoints(P(q[0].x, q[0].y, 0), P(q[1].x, q[1].y, 0))
        for p in q[2:]:
            prev = L.addByTwoPoints(prev.endSketchPoint, P(p.x, p.y, 0))
        L.addByTwoPoints(prev.endSketchPoint, first.startSketchPoint)
        assert s.profiles.count == 1, name
        ei = feats.extrudeFeatures.createInput(s.profiles.item(0), op)
        ei.setDistanceExtent(False, VI(mm(thick)))
        if target:
            ei.participantBodies = [target]
        f = feats.extrudeFeatures.add(ei)
        if op == NEW:
            b = f.bodies.item(0)
            b.name = name
            made.append(b)
            return b

    def cyl(name, y, z, r, x0, x1):
        s = root.sketches.add(offplane(root.yZConstructionPlane, x0)); s.isVisible = False
        c = s.modelToSketchSpace(P(mm(x0), mm(y), mm(z)))
        s.sketchCurves.sketchCircles.addByCenterRadius(P(c.x, c.y, 0), mm(r))
        b = feats.extrudeFeatures.addSimple(s.profiles.item(0), VI(mm(x1 - x0)), NEW).bodies.item(0)
        b.name = name
        made.append(b)
        return b

    def box(name, x1, y1, z1, x2, y2, z2):
        s = root.sketches.add(offplane(root.xYConstructionPlane, z1)); s.isVisible = False
        s.sketchCurves.sketchLines.addTwoPointRectangle(P(mm(x1), mm(y1), 0), P(mm(x2), mm(y2), 0))
        b = feats.extrudeFeatures.addSimple(s.profiles.item(0), VI(mm(z2 - z1)), NEW).bodies.item(0)
        b.name = name
        made.append(b)
        return b

    # ===== SA: quadrant plates (flap-local angles 0..-60, r 105), slot r 80..90 at local -20..-50 ====
    R, R1, R2 = 105.0, 80.0, 90.0
    plate = [polar(0, 0)] + [polar(R, A - k * 5) for k in range(13)]
    slot = ([polar(R1, A - 20 - k * 2.5) for k in range(13)]
            + [polar(R2, A - 50 + k * 2.5) for k in range(13)])
    knob_y, knob_z = polar(85, -10)          # fixed in the body side; local angle at 25 deg is -35
    for side, x0 in (('left', -16), ('right', W + 12)):
        pl = prof('SA quadrant %s (alu)' % side, x0, 4, plate)
        prof('slot', x0, 4, slot, CUT, pl)
        cyl('SA knob %s (black)' % side, knob_y, knob_z, 5, x0, x0 + 4)
        kx = x0 - 14 if side == 'left' else x0 + 4
        cyl('SA knob cap %s (black)' % side, knob_y, knob_z, 12, kx, kx + 14)

    # ===== SB: strut hinged under the flap at d 200, 150 long, foot leaning toward the hinge ======
    D, L, RACK = 200.0, 150.0, 19.0
    py, pz = rot(HY + D, HZ, A)
    dz = pz - RACK
    fy = py - math.sqrt(L * L - dz * dz)
    uy, uz = (fy - py) / L, (RACK - pz) / L
    ny, nz = -uz, uy
    t = 3.0
    strut = [(py + ny * t, pz + nz * t), (fy + ny * t, RACK + nz * t),
             (fy - ny * t, RACK - nz * t), (py - ny * t, pz - nz * t)]
    prof('SB strut (alu)', 85, 400, strut)
    cyl('SB strut hinge (black)', py, pz, 5, 80, 490)
    box('SB rack (black)', 85, fy - 40, 13, 485, fy + 25, RACK)
    for k in range(-3, 3):
        box('SB rack notch (dark)', 85, fy + k * 10 - 1.5, RACK, 485, fy + k * 10 + 1.5, RACK + 0.4)
    print('SB pivot y %.1f z %.1f foot y %.1f' % (py, pz, fy))
    for deg in (10, 15, 20, 30, 35, 40, 45):
        qy, qz = rot(HY + D, HZ, deg)
        dd = qz - RACK
        print('  %d deg: foot y %s' % (deg, '%.1f' % (qy - math.sqrt(L * L - dd * dd)) if dd < L else 'n/a'))

    alu = des.appearances.itemByName('Aluminum - Satin')
    prt = root.bRepBodies.itemByName('PB1 panel (black)').appearance
    for b in made:
        b.appearance = alu if '(alu)' in b.name else prt
    print('made', len(made), 'bodies', root.bRepBodies.count)
