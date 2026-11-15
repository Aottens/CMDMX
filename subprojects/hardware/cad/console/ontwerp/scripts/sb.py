import adsk.core, adsk.fusion, math

# SB fold-out strut, second geometry: pivot 115 mm from the hinge under the flap, strut 85 mm,
# leaning back; feet at 15/25/35/45 degrees land 15+ mm apart on a sawtooth rack.
# ARGS = target flap angle in degrees. Rotates the 'B flap*' bodies there and redraws the strut.
VI = adsk.core.ValueInput.createByReal
P = adsk.core.Point3D.create
NEW = adsk.fusion.FeatureOperations.NewBodyFeatureOperation
HY, HZ, RACK = 345.0, 13.0, 19.0
D, L = 115.0, 85.0
STEPS = (15, 25, 35, 45)


def mm(v):
    return v / 10.0


def geom(a):
    py = HY + D * math.cos(math.radians(a))
    pz = HZ + D * math.sin(math.radians(a))
    dz = pz - RACK
    return py, pz, py + math.sqrt(L * L - dz * dz)


def run(_context: str):
    target = ARGS
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    root = des.rootComponent
    feats = root.features
    made = []

    # current flap angle from the top of the flap body: z = 13 + 250 sin a + 20 cos a
    top = root.bRepBodies.itemByName('B flap body (print)').boundingBox.maxPoint.z * 10
    cur = min(range(0, 900), key=lambda k: abs(HZ + 250 * math.sin(math.radians(k / 10)) + 20 * math.cos(math.radians(k / 10)) - top)) / 10
    if abs(cur - target) > 0.05:
        coll = adsk.core.ObjectCollection.create()
        for b in root.bRepBodies:
            if b.name.startswith('B flap'):
                coll.add(b)
        assert coll.count == 4, coll.count
        m = adsk.core.Matrix3D.create()
        m.setToRotation(math.radians(target - cur), adsk.core.Vector3D.create(1, 0, 0), P(0, mm(HY), mm(HZ)))
        mi = feats.moveFeatures.createInput2(coll)
        mi.defineAsFreeMove(m)
        feats.moveFeatures.add(mi)

    rebuild_rack = not any(b.name.startswith('SB rack') and 'v2' in b.name for b in root.bRepBodies)
    for b in [b for b in root.bRepBodies if b.name.startswith('SB ')]:
        if b.name.startswith(('SB strut',)) or (rebuild_rack and b.name.startswith('SB rack')):
            b.deleteMe()

    def offplane(base, d):
        pi = root.constructionPlanes.createInput()
        pi.setByOffset(base, VI(mm(d)))
        pl = root.constructionPlanes.add(pi)
        pl.isLightBulbOn = False
        return pl

    def prof(name, x0, thick, pts):
        s = root.sketches.add(offplane(root.yZConstructionPlane, x0)); s.isVisible = False
        q = [s.modelToSketchSpace(P(mm(x0), mm(y), mm(z))) for y, z in pts]
        Ls = s.sketchCurves.sketchLines
        first = prev = Ls.addByTwoPoints(P(q[0].x, q[0].y, 0), P(q[1].x, q[1].y, 0))
        for p in q[2:]:
            prev = Ls.addByTwoPoints(prev.endSketchPoint, P(p.x, p.y, 0))
        Ls.addByTwoPoints(prev.endSketchPoint, first.startSketchPoint)
        assert s.profiles.count == 1, name
        b = feats.extrudeFeatures.addSimple(s.profiles.item(0), VI(mm(thick)), NEW).bodies.item(0)
        b.name = name
        made.append(b)

    def cyl(name, y, z, r, x0, x1):
        s = root.sketches.add(offplane(root.yZConstructionPlane, x0)); s.isVisible = False
        c = s.modelToSketchSpace(P(mm(x0), mm(y), mm(z)))
        s.sketchCurves.sketchCircles.addByCenterRadius(P(c.x, c.y, 0), mm(r))
        b = feats.extrudeFeatures.addSimple(s.profiles.item(0), VI(mm(x1 - x0)), NEW).bodies.item(0)
        b.name = name
        made.append(b)

    py, pz, fy = geom(target)
    uy, uz = (fy - py) / L, (RACK - pz) / L
    ny, nz = -uz, uy
    t = 3.0
    prof('SB strut (alu)', 85, 400, [(py + ny * t, pz + nz * t), (fy + ny * t, RACK + nz * t),
                                     (fy - ny * t, RACK - nz * t), (py - ny * t, pz - nz * t)])
    cyl('SB strut hinge (black)', py, pz, 4, 80, 490)

    if rebuild_rack:
        feet = sorted(geom(a)[2] for a in STEPS)
        y0, y1 = feet[0] - 15, feet[-1] + 8
        # sawtooth: each tooth rises toward the back, its steep face stops the foot sliding back
        pts = [(y0, 13.0), (y1, 13.0), (y1, RACK)]
        for f in reversed(feet):
            pts += [(f + 0.01, RACK), (f, RACK - 4), (f - 12, RACK)]
        pts.append((y0, RACK))
        prof('SB rack v2 (black)', 85, 400, pts)

    alu = des.appearances.itemByName('Aluminum - Satin')
    prt = root.bRepBodies.itemByName('PB1 panel (black)').appearance
    for b in made:
        b.appearance = alu if '(alu)' in b.name else prt
    print('angle', target, 'from', cur, 'pivot %.1f/%.1f foot %.1f' % (py, pz, fy),
          'feet', [round(geom(a)[2], 1) for a in STEPS])
