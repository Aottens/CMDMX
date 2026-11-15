import adsk.core, adsk.fusion, os

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
    print(sorted(os.listdir(out)))
