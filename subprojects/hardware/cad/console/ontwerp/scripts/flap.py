import adsk.core, adsk.fusion, math


def run(_context: str):
    deg = ARGS
    des = adsk.fusion.Design.cast(adsk.core.Application.get().activeProduct)
    root = des.rootComponent
    coll = adsk.core.ObjectCollection.create()
    for b in root.bRepBodies:
        if b.name.startswith('B flap'):
            coll.add(b)
    assert coll.count == 4, coll.count
    m = adsk.core.Matrix3D.create()
    m.setToRotation(math.radians(deg), adsk.core.Vector3D.create(1, 0, 0), adsk.core.Point3D.create(0, 34.5, 1.3))
    mi = root.features.moveFeatures.createInput2(coll)
    mi.defineAsFreeMove(m)
    root.features.moveFeatures.add(mi)
