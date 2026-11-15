import adsk.core, adsk.fusion

# Camera for the HWB-10 renders, desk coordinates in mm (z up). ARGS = (name, hidden) where hidden is a tuple of
# body-name prefixes to hide (e.g. ('K cover', 'K cable lid') to look into the flap from behind).
CAMS = {
    'front': ((-250, -600, 650), (285, 440, 60), 70),
    'rear': ((900, 1050, 120), (285, 440, 50), 70),
    'under': ((700, 900, -350), (285, 440, 60), 70),
    'side': ((1300, 470, 80), (285, 470, 60), 40),
    'cable': ((-260, 250, 120), (60, 380, 20), 22),
}


def run(_context: str):
    name, hidden = ARGS
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    for o in des.rootComponent.occurrences:
        for b in o.component.bRepBodies:
            b.isLightBulbOn = not b.name.startswith(tuple(hidden))
    eye, tgt, ext = CAMS[name]
    v = app.activeViewport
    cam = v.camera
    cam.isSmoothTransition = False
    cam.cameraType = adsk.core.CameraTypes.PerspectiveCameraType
    cam.eye = adsk.core.Point3D.create(*[c / 10 for c in eye])
    cam.target = adsk.core.Point3D.create(*[c / 10 for c in tgt])
    cam.upVector = adsk.core.Vector3D.create(0, 0, 1)
    cam.isFitView = False
    cam.viewExtents = ext
    v.camera = cam
    v.refresh()
