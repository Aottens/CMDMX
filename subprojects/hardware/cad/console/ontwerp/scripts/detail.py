import adsk.core, adsk.fusion

# Detail variants of the M1 body in K1 colours: walnut band height h (top stays at z 16) and
# front recede r (how far the foot of the front and side walls sits back from the frame).
VI = adsk.core.ValueInput.createByReal
P = adsk.core.Point3D.create
NEW = adsk.fusion.FeatureOperations.NewBodyFeatureOperation
W = 570.0
VARIANTS = (('D1', 8, 8), ('D2', 12, 16), ('D3', 20, 26))


def mm(v):
    return v / 10.0


def run(_context: str):
    des = adsk.fusion.Design.cast(adsk.core.Application.get().activeProduct)
    root = des.rootComponent
    assert not [b for b in root.bRepBodies if b.name.startswith(('D1 ', 'D2 ', 'D3 '))]
    feats = root.features
    wood = des.appearances.itemByName('Walnut')
    body_app = des.appearances.itemByName('CMF 52 53 55')
    foot_app = root.bRepBodies.itemByName('PB1 panel (black)').appearance
    assert wood and body_app

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

    for tag, h, r in VARIANTS:
        zt = 16 - h
        made = []
        for x0 in (-12, W):
            made.append(prof('%s side (print)' % tag, x0, 12, [(-10 + r, -68), (-10, zt), (563, zt), (563, -68)]))
            made.append(prof('%s frame side (walnut)' % tag, x0, 12, [(-10, zt), (563, zt), (563, 16), (-10, 16)]))
        fr = box('%s frame front (walnut)' % tag, -12, -10, zt, W + 12, 0, 16)
        fillet_x(fr, -10, 16, min(6, h - 2))
        made.append(fr)
        made.append(box('%s frame back (walnut)' % tag, 0, 551, zt, W, 563, 16))
        made.append(box('%s tray (print)' % tag, 0, 325, 0, W, 551, 13))
        made.append(prof('%s front wall (print)' % tag, 0, W, [(-10, zt), (-2, zt), (-2 + r, -62), (-10 + r, -62)]))
        made.append(box('%s bottom (print)' % tag, 0, -10 + r, -68, W, 563, -62))
        made.append(box('%s rear wall (print)' % tag, 0, 551, -62, W, 557, min(zt, 0)))
        for x, y in ((30, 20), (W - 60, 20), (30, 520), (W - 60, 520)):
            made.append(box('%s foot' % tag, x, y, -76, x + 30, y + 30, -68))
        for b in made:
            b.appearance = wood if 'walnut' in b.name else foot_app if 'foot' in b.name else body_app
        print(tag, 'band', h, 'recede', r, 'bodies', len(made))
