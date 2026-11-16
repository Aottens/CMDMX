import adsk.core, adsk.fusion, math, os

# Printed backing for the 1.5 mm test strip (HW-50, HW-53). Run through the Fusion MCP wrapper (cad/README.md); it
# needs only CAD. It opens a new direct-modelling document (no other document is touched), builds both variants from
# layout.py's boxes and cylinders, checks them, and writes into acryl-proef/onderframe/:
#   cmdmx-onderframe-<variant>.step / .f3d   strip coordinates: z 0 = plate underside, backing down to z -13
#   cmdmx-onderframe-<variant>.stl           print orientation: turned over, plate face on the bed (y -> 200 - y)
#   onderkant.png                            both variants as they lie on the bed, ribs up
# Then it closes the document without saving.

P0 = adsk.core.Point3D.create
V = adsk.core.Vector3D.create
T = adsk.fusion.TemporaryBRepManager.get()
UNION = adsk.fusion.BooleanTypes.UnionBooleanType
DIFF = adsk.fusion.BooleanTypes.DifferenceBooleanType
INTER = adsk.fusion.BooleanTypes.IntersectionBooleanType
PETG = 1.27   # g/cm3


def p(x, y, z):
    return P0(x / 10, y / 10, z / 10)


def solid(prim):
    if prim[0] == 'box':
        _, x0, x1, y0, y1, z0, z1 = prim
        ob = adsk.core.OrientedBoundingBox3D.create(p((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2), V(1, 0, 0),
                                                    V(0, 1, 0), (x1 - x0) / 10, (y1 - y0) / 10, (z1 - z0) / 10)
        return T.createBox(ob)
    _, x, y, r, z0, z1 = prim
    return T.createCylinderOrCone(p(x, y, z0), r / 10, p(x, y, z1), r / 10)


def combine(a, prims, op):
    for prim in prims:
        assert T.booleanOperation(a, solid(prim), op), prim
    return a


def build(variant):
    adds, subs = parts(variant)
    body = combine(solid(adds[0]), adds[1:], UNION)
    outline = combine(solid(('box', R, W - R, 0, L, -H - 1, 1)), [('box', 0, W, R, L - R, -H - 1, 1)]
                      + [('cyl', x, y, R, -H - 1, 1) for x in (R, W - R) for y in (R, L - R)], UNION)
    assert T.booleanOperation(body, outline, INTER)
    return combine(body, subs, DIFF)


def run(_context: str):
    g = globals()
    here = CAD + '/acryl-proef/onderframe'
    for f in ('calc.py', 'layout.py'):
        exec(open(here + '/build/' + f).read(), g)
    app = adsk.core.Application.get()
    doc = app.documents.add(adsk.core.DocumentTypes.FusionDesignDocumentType)
    des = adsk.fusion.Design.cast(app.activeProduct)
    des.designType = adsk.fusion.DesignTypes.DirectDesignType
    root, em = des.rootComponent, des.exportManager
    flip = adsk.core.Matrix3D.create()          # turn over about the x axis: y -> L - y, z -> -z
    flip.setToRotation(math.pi, V(1, 0, 0), p(0, L / 2, 0))
    for k, variant in enumerate(('ribben', 'dicht')):
        body = build(variant)
        m = adsk.core.Matrix3D.create()
        m.translation = V(k * 15.0, 0, 0)
        comp = root.occurrences.addNewComponent(m).component
        comp.name = 'onderframe ' + variant
        b = comp.bRepBodies.add(body)
        b.name = variant
        bb = b.boundingBox
        got = [round(v * 10, 3) for v in (bb.minPoint.x, bb.minPoint.y, bb.minPoint.z,
                                          bb.maxPoint.x, bb.maxPoint.y, bb.maxPoint.z)]
        assert got == [0, 0, -H, W, L, 0], got                 # fits a 256 x 256 bed, plate face at z 0
        assert b.lumps.count == 1, (variant, b.lumps.count)   # one piece, no loose ribs
        vol = b.volume
        base = os.path.join(here, 'cmdmx-onderframe-' + variant)
        assert em.execute(em.createSTEPExportOptions(base + '.step', comp))
        assert em.execute(em.createFusionArchiveExportOptions(base + '.f3d', comp))
        pr = T.copy(body)
        assert T.transform(pr, flip)
        pb = comp.bRepBodies.add(pr)
        assert abs(pb.boundingBox.minPoint.z) < 1e-6 and abs(pb.boundingBox.maxPoint.y * 10 - L) < 1e-3
        opt = em.createSTLExportOptions(pb, base + '.stl')
        opt.meshRefinement = adsk.fusion.MeshRefinementSettings.MeshRefinementHigh
        assert em.execute(opt)
        b.deleteMe()                  # exported; the render shows the print copy only
        print('%-6s %.1f cm3, about %.0f g PETG' % (variant, vol, vol * PETG))
    v = app.activeViewport
    cam = v.camera
    cam.isSmoothTransition = False
    cam.cameraType = adsk.core.CameraTypes.PerspectiveCameraType
    cam.eye = p(75, -60, 560)
    cam.target = p(135, 100, 0)
    cam.upVector = V(0, 0, 1)
    v.camera = cam
    v.fit()
    assert v.saveAsImageFile(os.path.join(here, 'onderkant.png'), 1600, 1000)
    doc.close(False)
    print(sorted(f for f in os.listdir(here) if f != 'build'))
