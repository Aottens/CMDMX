import adsk.core, adsk.fusion

DECK = ('PB1', 'PB2', 'PR1', 'PR2', 'PG1', 'PG2')
G_KEEP = DECK + ('SL', 'SMB', 'SMT', 'SR', 'RL', 'RM', 'RR', 'G', 'Screen')
CAMS = {
    'front': ((-35.0, -95.0, 70.0), (28.5, 26.0, 0.0)),
    'side': ((160.0, 26.0, 2.0), (28.5, 26.0, 2.0)),
    'rear': ((95.0, 150.0, 60.0), (28.5, 40.0, 2.0)),
    'low3q': ((125.0, 100.0, 14.0), (30.0, 38.0, 3.0)),
    'corner': ((-62.0, -80.0, 8.0), (22.0, 18.0, -1.0)),
    'rear3qL': ((-75.0, 115.0, 40.0), (28.5, 38.0, 3.0)),
}


def run(_context: str):
    variant, cam = ARGS
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    def shown(n):
        p = n.split(' ')[0]
        if variant == 'B':
            return p in DECK + ('B',)
        if variant == 'G':
            return p in G_KEEP
        if variant == 'M1':
            return p in DECK + ('M1',) or n.startswith(('B flap', 'B hinge'))
        if variant in ('D1', 'D2', 'D3'):
            return p in DECK + (variant, 'SB', 'CAP') or n.startswith(('B flap', 'B hinge'))
        if variant in ('SA', 'SB'):
            return p in DECK + ('M1', variant) or n.startswith(('B flap', 'B hinge'))
        if variant == 'M2':
            return (p in DECK + ('SL', 'SMB', 'SMT', 'SR', 'Screen', 'M2')
                    or n.startswith(('G front wall', 'G frame front', 'G bottom', 'G foot')))
        raise ValueError(variant)
    for b in des.rootComponent.bRepBodies:
        b.isLightBulbOn = shown(b.name)
    vp = app.activeViewport
    c = vp.camera
    c.cameraType = adsk.core.CameraTypes.PerspectiveCameraType
    c.isFitView = False
    eye, tgt = CAMS[cam]
    c.eye = adsk.core.Point3D.create(*eye)
    c.target = adsk.core.Point3D.create(*tgt)
    c.upVector = adsk.core.Vector3D.create(0, 0, 1)
    c.perspectiveAngle = 0.5
    vp.camera = c
    vp.refresh()
