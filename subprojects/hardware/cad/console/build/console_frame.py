import adsk.core, adsk.fusion, math

VI = adsk.core.ValueInput.createByReal
P = adsk.core.Point3D.create
NEW = adsk.fusion.FeatureOperations.NewBodyFeatureOperation
TAN = math.tan(math.radians(20))
W = 570.0
CHEEK = 25.0
ZB = -62.0          # top of the bottom plate (inside floor)


def mm(v):
    return v / 10.0


def run(_context: str):
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    root = des.rootComponent
    assert root.bRepBodies.count == 407, root.bRepBodies.count
    feats = root.features
    made = {}

    def box(name, x1, y1, z1, x2, y2, z2, kind):
        pi = root.constructionPlanes.createInput()
        pi.setByOffset(root.xYConstructionPlane, VI(mm(z1)))
        pl = root.constructionPlanes.add(pi)
        pl.isLightBulbOn = False
        s = root.sketches.add(pl); s.isVisible = False
        s.sketchCurves.sketchLines.addTwoPointRectangle(P(mm(x1), mm(y1), 0), P(mm(x2), mm(y2), 0))
        b = feats.extrudeFeatures.addSimple(s.profiles.item(0), VI(mm(z2 - z1)), NEW).bodies.item(0)
        b.name = name
        made[name] = (b, kind)
        return b

    def edge_along_x(body, y, z):
        for e in body.edges:
            a, b = e.startVertex.geometry, e.endVertex.geometry
            if all(abs(p.y - mm(y)) < 1e-4 and abs(p.z - mm(z)) < 1e-4 for p in (a, b)) and abs(a.x - b.x) > 1e-3:
                return e
        raise RuntimeError('edge not found %s %s %s' % (body.name, y, z))

    def fillet(body, edges_r):
        for y, z, r in edges_r:
            coll = adsk.core.ObjectCollection.create()
            coll.add(edge_along_x(body, y, z))
            fi = feats.filletFeatures.createInput()
            fi.edgeSetInputs.addConstantRadiusEdgeSet(coll, VI(mm(r)), False)
            feats.filletFeatures.add(fi)

    # --- solid wood: cheeks, armrest, front board -------------------------------------------
    prof = [(-45, ZB - 6), (-45, 25), (0, 25), (0, 16), (325, 16), (563, 16 + 238 * TAN), (563, ZB - 6)]
    for name, x0 in (('Cheek left (walnut)', -CHEEK), ('Cheek right (walnut)', W)):
        pi = root.constructionPlanes.createInput()
        pi.setByOffset(root.yZConstructionPlane, VI(mm(x0)))
        pl = root.constructionPlanes.add(pi)
        pl.isLightBulbOn = False
        s = root.sketches.add(pl); s.isVisible = False
        q = [s.modelToSketchSpace(P(mm(x0), mm(y), mm(z))) for y, z in prof]
        lines = s.sketchCurves.sketchLines
        first = prev = lines.addByTwoPoints(P(q[0].x, q[0].y, 0), P(q[1].x, q[1].y, 0))
        for i in range(2, len(q)):
            prev = lines.addByTwoPoints(prev.endSketchPoint, P(q[i].x, q[i].y, 0))
        lines.addByTwoPoints(prev.endSketchPoint, first.startSketchPoint)
        assert s.profiles.count == 1
        b = feats.extrudeFeatures.addSimple(s.profiles.item(0), VI(mm(CHEEK)), NEW).bodies.item(0)
        b.name = name
        bb = b.boundingBox
        assert abs(bb.minPoint.x * 10 - x0) < 0.01 and abs(bb.maxPoint.x * 10 - (x0 + CHEEK)) < 0.01, (name, bb.minPoint.x, bb.maxPoint.x)
        fillet(b, [(-45, 25, 14), (-45, ZB - 6, 4), (563, 16 + 238 * TAN, 6)])
        made[name] = (b, 'wood')

    arm = box('Armrest (walnut)', 0, -45, -16, W, 0, 25, 'wood')
    fillet(arm, [(-45, 25, 14), (0, 25, 4), (-45, -16, 6)])
    box('Front board (walnut)', 0, -32, ZB - 6, W, -20, -16, 'wood')
    box('Bottom plate (6 mm ply, removable)', 0, -20, ZB - 6, W, 563, ZB, 'ply')

    # --- 2020 aluminium ladder frame, every beam 570 long, end-tapped M5 into the cheeks -------
    beams = [
        ('2020 under the deck seam', 190, -20, 210, 0),
        ('2020 under deck-to-screen joint', 315, -20, 335, 0),
        ('2020 rear tie under the screen', 530, 40, 550, 60),
        ('2020 floor front', -20, ZB, 0, ZB + 20),
        ('2020 floor middle', 315, ZB, 335, ZB + 20),
        ('2020 floor rear', 530, ZB, 550, ZB + 20),
    ]
    for name, y1, z1, y2, z2 in beams:
        box(name, 0, y1, z1, W, y2, z2, 'alu')

    # --- placeholders for what hangs inside (sizes approximate) ------------------------------
    for k in range(10):
        fx = 16.5 + 33 * k
        box('Fader PSM %d (approx)' % (k + 1), fx - 6.5, 4, -15, fx + 6.5, 156, 10.5, 'part')
        box('Fader PTA %d (approx)' % (k + 11), fx - 4.5, 224.5, 1.5, fx + 4.5, 299.5, 10.5, 'part')
    box('Fader PSM GM (approx)', 346.5 - 6.5, 4, -15, 346.5 + 6.5, 156, 10.5, 'part')
    box('Compute, Mac mini size 127x127x50', 420, 390, ZB, 547, 517, ZB + 50, 'part')
    box('PSU 12 V, LRS-50 size', 300, 420, ZB, 399, 502, ZB + 30, 'part')

    # --- clash check: nothing new may overlap anything else ------------------------------------
    names = list(made)
    others = [b for b in root.bRepBodies if b.name not in made and 'legend' not in b.name]
    clashes = []
    for i, n in enumerate(names):
        b = made[n][0]
        for o in [made[m][0] for m in names[i + 1:]] + others:
            if b.boundingBox.intersects(o.boundingBox):
                tb = adsk.fusion.TemporaryBRepManager.get()
                t1, t2 = tb.copy(b), tb.copy(o)
                tb.booleanOperation(t1, t2, adsk.fusion.BooleanTypes.IntersectionBooleanType)
                if t1.volume > 1e-4:
                    clashes.append((n, o.name, round(t1.volume * 1000, 1)))
    print('clashes', clashes)

    prefs = {'wood': [('walnut',)], 'alu': [('aluminum', 'satin'), ('aluminum', 'brushed'), ('aluminum',)],
             'ply': [('birch',), ('plywood',), ('oak',)],
             'part': [('plastic', 'matte', 'gray'), ('plastic', 'matte', 'grey'), ('plastic', 'matte')]}
    allapp = [lib.appearances.item(i) for lib in app.materialLibraries for i in range(lib.appearances.count)]
    pick = {}
    for key, opts in prefs.items():
        for words in opts:
            hit = next((a for a in allapp if all(w in a.name.lower() for w in words)), None)
            if hit:
                pick[key] = des.appearances.itemByName(hit.name) or des.appearances.addByCopy(hit, hit.name)
                break
    print({k: v.name for k, v in pick.items()})
    for n, (b, kind) in made.items():
        if kind in pick:
            b.appearance = pick[kind]
    blk = [b for b in root.bRepBodies if 'PB1 panel' in b.name][0].appearance
    white = [b for b in root.bRepBodies if b.name.startswith('PB1 legend')][0].appearance
    for b in root.bRepBodies:
        if 'panel (black)' in b.name or b.name.startswith('Screen module'):
            b.appearance = blk
        elif 'legend' in b.name:
            b.appearance = white
    print('bodies', root.bRepBodies.count)
