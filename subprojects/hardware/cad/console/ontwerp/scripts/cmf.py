import adsk.core, adsk.fusion

# Colour directions on M1 + SB with caps. ARGS = 'K1' | 'K2' | 'K3'.
# A value is an (r, g, b) tuple (a matte plastic copy in that colour) or the name of a design appearance.
VARIANTS = {
    'K1': dict(body=(52, 53, 55), frame='Walnut', pb=(70, 70, 72), prog=(70, 70, 72), keypad=(70, 70, 72),
               accent=(225, 222, 214), fader=(225, 222, 214), gm=(200, 40, 35)),
    'K2': dict(body=(214, 211, 203), frame='Walnut', pb=(238, 236, 230), prog=(238, 236, 230),
               keypad=(238, 236, 230), accent=(60, 130, 85), fader=(238, 236, 230), gm=(60, 130, 85)),
    'K3': dict(body='Aluminum - Satin', frame=(30, 30, 30), pb=(190, 190, 188), prog=(190, 190, 188),
               keypad=(120, 120, 120), accent=(240, 100, 30), fader=(30, 30, 30), gm=(240, 100, 30)),
}
BODY = ('M1 side', 'M1 tray', 'M1 front wall', 'M1 bottom', 'M1 rear wall', 'B flap body')
FRAME = ('M1 frame', 'B flap side')


def run(_context: str):
    v = VARIANTS[ARGS]
    des = adsk.fusion.Design.cast(adsk.core.Application.get().activeProduct)
    root = des.rootComponent
    base = des.appearances.itemByName('Plastic - Matte (Gray)')

    def app(val):
        if isinstance(val, str):
            a = des.appearances.itemByName(val)
            assert a, val
            return a
        name = 'CMF %d %d %d' % val
        a = des.appearances.itemByName(name)
        if not a:
            a = des.appearances.addByCopy(base, name)
            col = next(p for p in a.appearanceProperties if p.name == 'Color')
            adsk.core.ColorProperty.cast(col).value = adsk.core.Color.create(val[0], val[1], val[2], 255)
        return a

    def role(n):
        if n.startswith(BODY):
            return 'body'
        if n.startswith(FRAME):
            return 'frame'
        if n.startswith('CAP key '):
            return n.split(' ')[2]
        if n.startswith('CAP fader '):
            return 'gm' if n.split(' ')[2] == 'gm' else 'fader'
        return None

    count = {}
    for b in root.bRepBodies:
        r = role(b.name)
        if r:
            b.appearance = app(v[r])
            count[r] = count.get(r, 0) + 1
    print(ARGS, count)
