import adsk.core, adsk.fusion, math

VI = adsk.core.ValueInput.createByReal
P = adsk.core.Point3D.create
NEW = adsk.fusion.FeatureOperations.NewBodyFeatureOperation
W = 570.0
T20 = math.tan(math.radians(20))
ZT = 4 + 238 * T20          # top of G's frame line at the back (z of the slope at y 563)


def mm(v):
    return v / 10.0


def run(_context: str):
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    root = des.rootComponent
    assert root.bRepBodies.count == 468, root.bRepBodies.count
    feats = root.features
    made = []

    def offplane(base, d):
        pi = root.constructionPlanes.createInput()
        pi.setByOffset(base, VI(mm(d)))
        pl = root.constructionPlanes.add(pi)
        pl.isLightBulbOn = False
        return pl

    def box(name, x1, y1, z1, x2, y2, z2):
        s = root.sketches.add(offplane(root.xYConstructionPlane, z1)); s.isVisible = False
        s.sketchCurves.sketchLines.addTwoPointRectangle(P(mm(x1), mm(y1), 0), P(mm(x2), mm(y2), 0))
        b = feats.extrudeFeatures.addSimple(s.profiles.item(0), VI(mm(z2 - z1)), NEW).bodies.item(0)
        b.name = name
        made.append(b)
        return b

    def prof(name, x0, thick, pts):
        s = root.sketches.add(offplane(root.yZConstructionPlane, x0)); s.isVisible = False
        q = [s.modelToSketchSpace(P(mm(x0), mm(y), mm(z))) for y, z in pts]
        L = s.sketchCurves.sketchLines
        first = prev = L.addByTwoPoints(P(q[0].x, q[0].y, 0), P(q[1].x, q[1].y, 0))
        for p in q[2:]:
            prev = L.addByTwoPoints(prev.endSketchPoint, P(p.x, p.y, 0))
        L.addByTwoPoints(prev.endSketchPoint, first.startSketchPoint)
        assert s.profiles.count == 1, name
        b = feats.extrudeFeatures.addSimple(s.profiles.item(0), VI(mm(thick)), NEW).bodies.item(0)
        b.name = name
        assert abs(b.boundingBox.minPoint.x * 10 - x0) < 0.01, name
        made.append(b)
        return b

    def fillet_x(body, y, z, r):
        for e in body.edges:
            a, b = e.startVertex.geometry, e.endVertex.geometry
            if all(abs(p.y - mm(y)) < 1e-3 and abs(p.z - mm(z)) < 1e-3 for p in (a, b)) and abs(a.x - b.x) > 1e-3:
                coll = adsk.core.ObjectCollection.create(); coll.add(e)
                fi = feats.filletFeatures.createInput()
                fi.edgeSetInputs.addConstantRadiusEdgeSet(coll, VI(mm(r)), False)
                feats.filletFeatures.add(fi)
                return
        raise RuntimeError('no x-edge %s %s %s' % (body.name, y, z))

    sides = ((-12,), (W,))

    # ===== M1: G's body kept flat all the way back, B's tilting screen flap on top =========
    for (x0,) in sides:
        prof('M1 side (print)', x0, 12, [(-2, -68), (-10, 4), (563, 4), (563, -68)])
        prof('M1 frame side (walnut)', x0, 12, [(-10, 4), (563, 4), (563, 16), (-10, 16)])
    fr = box('M1 frame front (walnut)', -12, -10, 4, W + 12, 0, 16)
    fillet_x(fr, -10, 16, 6)
    box('M1 frame back (walnut)', 0, 551, 4, W, 563, 16)
    box('M1 tray (print)', 0, 325, 0, W, 551, 13)
    prof('M1 front wall (print)', 0, W, [(-10, 4), (0, 4), (6, -62), (-2, -62)])
    box('M1 bottom (print)', 0, -2, -68, W, 563, -62)
    box('M1 rear wall (print)', 0, 551, -62, W, 557, 0)
    for x, y in ((30, 20), (W - 60, 20), (30, 520), (W - 60, 520)):
        box('M1 foot', x, y, -76, x + 30, y + 30, -68)

    # ===== M2: G exactly, but the whole screen section is a lid that tilts with its frame ===
    for (x0,) in sides:
        prof('M2 side (print)', x0, 12, [(-2, -68), (-10, 4), (563, 4), (563, -68)])
        prof('M2 frame deck (walnut)', x0, 12, [(-10, 4), (325, 4), (325, 16), (-10, 16)])
        prof('M2 lid side (print)', x0, 12, [(325, 4), (563, 4), (563, ZT)])
        prof('M2 lid frame (walnut)', x0, 12, [(325, 4), (563, ZT), (563, ZT + 12), (325, 16)])
    prof('M2 lid back (print)', 0, W, [(557, 4), (563, 4), (563, ZT), (557, ZT - 6 * T20)])
    box('M2 rear wall (print)', 0, 557, -62, W, 563, 4)

    wood = des.appearances.itemByName('Walnut')
    prt = root.bRepBodies.itemByName('PB1 panel (black)').appearance
    for b in made:
        b.appearance = wood if 'walnut' in b.name else prt
    print('made', len(made), 'bodies', root.bRepBodies.count)
