import adsk.core, adsk.fusion, math

VI = adsk.core.ValueInput.createByReal
P = adsk.core.Point3D.create
V3 = adsk.core.Vector3D.create
NEW = adsk.fusion.FeatureOperations.NewBodyFeatureOperation
W = 570.0
T20 = math.tan(math.radians(20))


def mm(v):
    return v / 10.0


def run(_context: str):
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    root = des.rootComponent
    assert root.bRepBodies.count == 441, root.bRepBodies.count
    assert not [b for b in root.bRepBodies if b.name.startswith(('B ', 'G '))]
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

    def side_profile(name, x0, thick, pts):
        """Extrude a (y, z) polygon along +x from x0."""
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
        bb = b.boundingBox
        assert abs(bb.minPoint.x * 10 - x0) < 0.01, (name, bb.minPoint.x * 10, x0)
        made.append(b)
        return b

    def fillet_x(body, y, z, r):
        """Round the edge that runs along x at (y, z)."""
        for e in body.edges:
            a, b = e.startVertex.geometry, e.endVertex.geometry
            if all(abs(p.y - mm(y)) < 1e-3 and abs(p.z - mm(z)) < 1e-3 for p in (a, b)) and abs(a.x - b.x) > 1e-3:
                coll = adsk.core.ObjectCollection.create(); coll.add(e)
                fi = feats.filletFeatures.createInput()
                fi.edgeSetInputs.addConstantRadiusEdgeSet(coll, VI(mm(r)), False)
                feats.filletFeatures.add(fi)
                return
        raise RuntimeError('no x-edge %s at %s %s' % (body.name, y, z))

    def fillet_y(body, x, z, r):
        """Round the edge that runs along y at (x, z)."""
        for e in body.edges:
            a, b = e.startVertex.geometry, e.endVertex.geometry
            if all(abs(p.x - mm(x)) < 1e-3 and abs(p.z - mm(z)) < 1e-3 for p in (a, b)) and abs(a.y - b.y) > 1e-3:
                coll = adsk.core.ObjectCollection.create(); coll.add(e)
                fi = feats.filletFeatures.createInput()
                fi.edgeSetInputs.addConstantRadiusEdgeSet(coll, VI(mm(r)), False)
                feats.filletFeatures.add(fi)
                return
        raise RuntimeError('no y-edge %s at %s %s' % (body.name, x, z))

    # ===== B: floating slab, walnut rim, recessed base, hinged screen =====================
    # deck face z 13; slab from z -5 to 14 (rim 1 mm proud); base set back; table at z -85
    lip = box('B front lip (walnut)', -12, -45, -5, W + 12, 0, 14)
    fillet_x(lip, -45, 14, 8)
    fillet_x(lip, -45, -5, 3)
    for nm, x1, x2, xo in (('B rim left (walnut)', -12, 0, -12), ('B rim right (walnut)', W, W + 12, W + 12)):
        r = box(nm, x1, 0, -5, x2, 528, 14)
        fillet_y(r, xo, 14, 3)
    back = box('B back rim (walnut)', -12, 528, -5, W + 12, 540, 14)
    fillet_x(back, 540, 14, 3)
    box('B rear tray (print)', 0, 325, -5, W, 528, 13)
    box('B under deck (print)', 0, 0, -5, W, 325, 0)
    box('B base (anthracite)', 10, 40, -77, W - 10, 515, -5)
    for x, y in ((40, 70), (W - 70, 70), (40, 455), (W - 70, 455)):
        box('B foot', x, y, -85, x + 30, y + 30, -77)
    # screen flap: flat on the tray, front edge at the hinge (y 345, z 13), then rotated
    flap = [box('B flap body (print)', 0, 345, 13, W, 595, 33),
            box('B flap side left (walnut)', -12, 345, 13, 0, 595, 33),
            box('B flap side right (walnut)', W, 345, 13, W + 12, 595, 33),
            box('B flap glass', 112, 345 + 25, 33, 458, 345 + 221, 33.6)]
    hinge = root.sketches.add(root.yZConstructionPlane); hinge.isVisible = False
    c = hinge.modelToSketchSpace(P(0, mm(345), mm(13)))
    hinge.sketchCurves.sketchCircles.addByCenterRadius(P(c.x, c.y, 0), mm(6))
    hb = feats.extrudeFeatures.addSimple(hinge.profiles.item(0), VI(mm(W)), NEW).bodies.item(0)
    hb.name = 'B hinge rod (alu)'
    made.append(hb)
    m = adsk.core.Matrix3D.create()
    m.setToRotation(math.radians(25), V3(1, 0, 0), P(0, mm(345), mm(13)))
    coll = adsk.core.ObjectCollection.create()
    for b in flap:
        coll.add(b)
    mi = feats.moveFeatures.createInput2(coll)
    mi.defineAsFreeMove(m)
    feats.moveFeatures.add(mi)

    # ===== G: black body, walnut frame along the top edge, front leans back at the foot ====
    # reuses the deck, the printed bezel at 20 degrees and the rear plate of the console model
    ztop = 4 + 238 * T20
    for nm, x0 in (('G side left (print)', -12), ('G side right (print)', W)):
        side_profile(nm, x0, 12, [(-2, -68), (-10, 4), (325, 4), (563, ztop), (563, -68)])
    for nm, x0 in (('G frame left (walnut)', -12), ('G frame right (walnut)', W)):
        side_profile(nm, x0, 12, [(-10, 4), (325, 4), (563, ztop), (563, ztop + 12), (325, 16), (-10, 16)])
    fr = box('G frame front (walnut)', -12, -10, 4, W + 12, 0, 16)
    fillet_x(fr, -10, 16, 6)
    side_profile('G front wall (print)', 0, W, [(-10, 4), (0, 4), (6, -62), (-2, -62)])
    box('G bottom plate (print)', 0, -2, -68, W, 563, -62)
    for x, y in ((30, 20), (W - 60, 20), (30, 520), (W - 60, 520)):
        box('G foot', x, y, -76, x + 30, y + 30, -68)

    # appearances
    allapp = [lib.appearances.item(i) for lib in app.materialLibraries for i in range(lib.appearances.count)]

    def find(*words):
        a = next((a for a in allapp if all(w in a.name.lower() for w in words)), None)
        return a and (des.appearances.itemByName(a.name) or des.appearances.addByCopy(a, a.name))
    wood = des.appearances.itemByName('Walnut')
    alu = des.appearances.itemByName('Aluminum - Satin')
    prt = root.bRepBodies.itemByName('PB1 panel (black)').appearance
    dark = find('plastic', 'matte', 'dark') or find('paint', 'dark', 'gray') or prt
    glass = find('glass', 'smoke') or find('glass', 'tinted') or find('enamel', 'glossy', 'black') or prt
    print('appearances', wood and wood.name, alu and alu.name, prt.name, dark.name, glass.name)
    for b in made:
        n = b.name
        b.appearance = (wood if 'walnut' in n else alu if 'alu' in n else dark if 'anthracite' in n or 'foot' in n
                        else glass if 'glass' in n else prt)
    print('made', len(made), 'bodies', root.bRepBodies.count)
