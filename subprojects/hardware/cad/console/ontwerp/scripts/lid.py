import adsk.core, adsk.fusion, math

LID = ('SL', 'SMB', 'SMT', 'SR', 'Screen')


def run(_context: str):
    deg = ARGS
    root = adsk.fusion.Design.cast(adsk.core.Application.get().activeProduct).rootComponent
    coll = adsk.core.ObjectCollection.create()
    for b in root.bRepBodies:
        if b.name.split(' ')[0] in LID or b.name.startswith(('M2 lid',)):
            coll.add(b)
    m = adsk.core.Matrix3D.create()
    m.setToRotation(math.radians(deg), adsk.core.Vector3D.create(1, 0, 0), adsk.core.Point3D.create(0, 32.5, 1.3))
    mi = root.features.moveFeatures.createInput2(coll)
    mi.defineAsFreeMove(m)
    root.features.moveFeatures.add(mi)
    print('moved', coll.count)
