import adsk.core, adsk.fusion, json

# Massing caps on the deck (deck face z 13): 105 keycaps 18 x 18 x 6.5 (z 18.5..25) and
# 21 fader knobs 12 x 18 x 12 (z 20..32), knobs at varied levels. Positions from paneel_plaat.layout() via layout.json.
LAYOUT = globals().get('CAD', '') + '/console/ontwerp/scripts/layout.json'  # runner sets CAD

VI = adsk.core.ValueInput.createByReal
P = adsk.core.Point3D.create
NEW = adsk.fusion.FeatureOperations.NewBodyFeatureOperation
LEVELS = (0.8, 0.65, 0.9, 0.3, 0.0, 0.0, 0.55, 1.0, 0.4, 0.7, 0.85)
TRAVEL = {'psm': (35, 100), 'pta': (32, 60)}
GROUP_Y = 222.0   # world y of the I P C G B E S FX row (accent keys)


def mm(v):
    return v / 10.0


def run(_context: str):
    des = adsk.fusion.Design.cast(adsk.core.Application.get().activeProduct)
    root = des.rootComponent
    assert not [b for b in root.bRepBodies if b.name.startswith('CAP ')]
    feats = root.features
    lay = json.load(open(LAYOUT))
    faders, keys = lay['faders'], lay['keys']

    def plane(z):
        pi = root.constructionPlanes.createInput()
        pi.setByOffset(root.xYConstructionPlane, VI(mm(z)))
        pl = root.constructionPlanes.add(pi)
        pl.isLightBulbOn = False
        return pl

    def boxes(rects, z1, z2):
        s = root.sketches.add(plane(z1)); s.isVisible = False
        for x1, y1, x2, y2 in rects:
            s.sketchCurves.sketchLines.addTwoPointRectangle(P(mm(x1), mm(y1), 0), P(mm(x2), mm(y2), 0))
        coll = adsk.core.ObjectCollection.create()
        for pr in s.profiles:
            coll.add(pr)
        assert coll.count == len(rects), (coll.count, len(rects))
        f = feats.extrudeFeatures.addSimple(coll, VI(mm(z2 - z1)), NEW)
        return [f.bodies.item(i) for i in range(f.bodies.count)]

    kb = boxes([(x - 9, y - 9, x + 9, y + 9) for x, y in keys], 18.5, 25.0)
    for b in kb:
        c = b.boundingBox
        cx, cy = (c.minPoint.x + c.maxPoint.x) * 5, (c.minPoint.y + c.maxPoint.y) * 5
        kind = 'accent' if abs(cy - GROUP_Y) < 1 else 'keypad' if cx > 470 and cy < 120 else 'prog' if cx > 360 else 'pb'
        b.name = 'CAP key %s' % kind
    rects = []
    for i, (fx, oy, lab, kind) in enumerate(faders):
        t0, tr = TRAVEL[kind]
        y = oy + t0 + tr * (1.0 if lab == 'GM' else LEVELS[i % len(LEVELS)])
        rects.append((fx - 6, y - 9, fx + 6, y + 9))
    fb = boxes(rects, 20.0, 32.0)
    for b in fb:
        c = b.boundingBox
        cx = (c.minPoint.x + c.maxPoint.x) * 5
        b.name = 'CAP fader %s' % ('gm' if abs(cx - 346.5) < 1 else 'pb')
    print('keys', len(kb), 'faders', len(fb), {n: sum(1 for b in kb + fb if b.name == n) for n in set(b.name for b in kb + fb)})
