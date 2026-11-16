import adsk.core, adsk.fusion, math, os

# Writes the HWB-10 files into cad/klep/ after klep.py ran (at 25 degrees for the strut): per component a native
# Fusion archive (.f3d) and a STEP (the flap in its own frame, strut and test base in desk coordinates), and one STL
# per printed body.
# The screen's STEP is Waveshare's and stays out of the repository.


def run(_context: str):
    out = CAD + '/klep'
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    em = des.exportManager
    names = {'K klep': 'cmdmx-klep', 'S strut': 'cmdmx-klep-strut', 'T testvoet': 'cmdmx-klep-testvoet'}
    for o in des.rootComponent.occurrences:
        if o.component.name not in names:
            continue
        base = os.path.join(out, names[o.component.name])
        assert em.execute(em.createSTEPExportOptions(base + '.step', o.component))
        assert em.execute(em.createFusionArchiveExportOptions(base + '.f3d', o.component))
        for b in o.component.bRepBodies:
            if '(print)' not in b.name:
                continue
            slug = b.name[2:b.name.rfind('(')].strip().lower().replace(' ', '-')
            pre = 'cmdmx-klep-voet-' if o.component.name == 'T testvoet' else 'cmdmx-klep-'
            opt = em.createSTLExportOptions(b, os.path.join(out, pre + slug + '.stl'))
            opt.meshRefinement = adsk.fusion.MeshRefinementSettings.MeshRefinementHigh
            assert em.execute(opt)
    arm_step(out)
    print(sorted(os.listdir(out)))


def arm_step(out):
    # The strut arm flat, for a machine shop (flat.py writes its DXF): built in a scratch document that is closed
    # without saving, so the flap document stays as it was.
    app = adsk.core.Application.get()
    doc = app.documents.add(adsk.core.DocumentTypes.FusionDesignDocumentType)
    des = adsk.fusion.Design.cast(app.activeProduct)
    des.designType = adsk.fusion.DesignTypes.DirectDesignType
    T = adsk.fusion.TemporaryBRepManager.get()
    P, V = adsk.core.Point3D.create, adsk.core.Vector3D.create
    cyl = lambda x, r: T.createCylinderOrCone(P(x, 0, 0), r, P(x, 0, 0.5), r)   # cm, 5 mm thick
    arm = T.createBox(adsk.core.OrientedBoundingBox3D.create(P(4.75, 0, 0.25), V(1, 0, 0), V(0, 1, 0), 9.5, 1.5, 0.5))
    for c in (cyl(0, 0.75), cyl(9.5, 0.75)):
        assert T.booleanOperation(arm, c, adsk.fusion.BooleanTypes.UnionBooleanType)
    for c in (cyl(0, 0.4), cyl(9.5, 0.4)):
        assert T.booleanOperation(arm, c, adsk.fusion.BooleanTypes.DifferenceBooleanType)
    des.rootComponent.bRepBodies.add(arm).name = 'arm 15x5 (alu)'
    em = des.exportManager
    assert em.execute(em.createSTEPExportOptions(os.path.join(out, 'cmdmx-klep-arm.step'), des.rootComponent))
    vol = des.rootComponent.bRepBodies.item(0).volume * 1000
    doc.close(False)
    expect = 95 * 15 * 5 + math.pi * 7.5 ** 2 * 5 - 2 * math.pi * 4 ** 2 * 5
    assert abs(vol - expect) < 1, (vol, expect)
    print('arm %.0f mm3' % vol)
