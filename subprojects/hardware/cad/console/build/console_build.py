import adsk.core, adsk.fusion, math, os, shutil

VI = adsk.core.ValueInput.createByReal
P = adsk.core.Point3D.create
V = adsk.core.Vector3D.create
NEW = adsk.fusion.FeatureOperations.NewBodyFeatureOperation
JOIN = adsk.fusion.FeatureOperations.JoinFeatureOperation
CUT = adsk.fusion.FeatureOperations.CutFeatureOperation
T, RH = 3.0, 10.0
OUT = globals().get('OUT', '/tmp/cmdmx-console-bodies')  # runner may set OUT
TILT = math.radians(20)
DECK_D, BEZ_D, REAR_D = 325.0, 250.0, 142.0
W = 570.0


def mm(v):
    return v / 10.0


# Console frame (world, mm): x 0..570 between the cheeks, y 0 = front edge of the deck, z 13 = deck face.
# Screen: assumed 15.6" module 360 x 225 x 15 behind a 346 x 196 opening (to check against the chosen panel).
SCR = (105.0, 10.5, 465.0, 235.5)          # module footprint in the bezel frame (x, v)
OPEN = (112.0, 25.0, 458.0, 221.0)          # visible opening


def bezel_pieces(bx, by):
    out = []
    for nm, x0, v0, w, d, xs, jys, texts in (
            ('SL', 0, 0, 190, BEZ_D, [56.0], (25, 100, 225), [('CMDMX', 7, 10, 6, 100, 20, 'C')]),
            ('SMB', 190, 0, 190, OPEN[1], [95.0], (5,), []),
            ('SMT', 190, OPEN[3], 190, BEZ_D - OPEN[3], [95.0], (22,), []),
            ('SR', 380, 0, 190, BEZ_D, [134.0], (25, 100, 225), [])):
        out.append(dict(name=nm, ox=bx + x0, oy=by + v0, W=w, D=d, xs=xs, ys=None, jys=jys, texts=texts, holes=[],
                        cuts=[] if nm in ('SMB', 'SMT') else [(OPEN[0] - x0, OPEN[1] - v0, OPEN[2] - x0, OPEN[3] - v0)],
                        free_kos=[(SCR[0] - x0, SCR[1] - v0, SCR[2] - x0, SCR[3] - v0)]))
    return out


def dsize(x, y):
    # Neutrik D-size: 24 mm hole, two M3 holes on the 19 x 24 diagonal
    return [(x, y, 12.0, 16), (x + 9.5, y + 12, 1.6, 0), (x - 9.5, y - 12, 1.6, 0)]


def rear_pieces(bx, by):
    # local x runs left to right as seen from behind the desk, local y is height above the bottom
    yc = 75.0
    conns = {'RL': [(40, 'POWER')], 'RM': [(95, 'NETWORK')],
             'RR': [(20, 'A'), (60, 'B'), (100, 'C'), (140, 'D')]}
    out = []
    for i, nm in enumerate(('RL', 'RM', 'RR')):
        holes, texts = [], []
        for x, lab in conns[nm]:
            holes += dsize(x, yc)
            texts.append((lab, 4, x - 18, yc - 26, x + 18, yc - 20, 'C'))
        if nm == 'RR':
            texts.append(('DMX OUT', 4, 20, yc + 22, 140, yc + 28, 'C'))
        out.append(dict(name=nm, ox=bx + 190 * i, oy=by, W=190, D=REAR_D, xs=[95.0], ys=[0.0, 71.0, REAR_D],
                        jys=(25, 105, 130), texts=texts, holes=holes, cuts=[], free_kos=[]))
    return out


def run(_context: str):
    app = adsk.core.Application.get()
    des = adsk.fusion.Design.cast(app.activeProduct)
    root = des.rootComponent
    assert root.bRepBodies.count == 372, root.bRepBodies.count
    feats = root.features
    planes = {}

    def plane(z):
        if z not in planes:
            pi = root.constructionPlanes.createInput()
            pi.setByOffset(root.xYConstructionPlane, VI(mm(z)))
            planes[z] = root.constructionPlanes.add(pi)
            planes[z].isLightBulbOn = False
        return planes[z]

    def extrude_profiles(sk, dist, op, n=None, body=None):
        coll = adsk.core.ObjectCollection.create()
        for pr in sk.profiles:
            coll.add(pr)
        if n is not None:
            assert coll.count == n, (sk.name, coll.count, n)
        inp = feats.extrudeFeatures.createInput(coll, op)
        inp.setDistanceExtent(False, VI(mm(dist)))
        if body is not None:
            inp.participantBodies = [body]
        return feats.extrudeFeatures.add(inp)

    C = adsk.core.HorizontalAlignments.CenterHorizontalAlignment
    L = adsk.core.HorizontalAlignments.LeftHorizontalAlignment

    def build(pc):
        """Flat printed piece, face up (face at z 13), same construction as the deck pieces."""
        nm, ox, oy, Wp, D = pc['name'], pc['ox'], pc['oy'], pc['W'], pc['D']
        assert Wp <= 250 and D <= 250, nm

        def sketch(z, label):
            s = root.sketches.add(plane(z) if z else root.xYConstructionPlane)
            s.name = nm + ' ' + label
            s.isVisible = False
            return s

        def rect(s, x1, y1, x2, y2):
            s.sketchCurves.sketchLines.addTwoPointRectangle(P(mm(ox + x1), mm(oy + y1), 0), P(mm(ox + x2), mm(oy + y2), 0))

        s = sketch(RH, 'skin'); rect(s, 0, 0, Wp, D)
        panel = extrude_profiles(s, T, NEW, 1).bodies.item(0)
        panel.name = nm + ' panel (black)'
        s = sketch(0, 'rib block'); rect(s, 0, 0, Wp, D)
        ribs = extrude_profiles(s, RH, NEW, 1).bodies.item(0)
        xs = [0.0] + pc['xs'] + [float(Wp)]
        ys = pc['ys'] or ([0.0, D / 4, D / 2, 3 * D / 4, D] if D > 100 else [0.0, D])
        cells = []
        for i in range(len(xs) - 1):
            for j in range(len(ys) - 1):
                x1 = xs[i] + (2 if i == 0 else 1); x2 = xs[i + 1] - (2 if i == len(xs) - 2 else 1)
                y1 = ys[j] + (2 if j == 0 else 1); y2 = ys[j + 1] - (2 if j == len(ys) - 2 else 1)
                cells.append((x1, y1, x2, y2))
        s = sketch(0, 'pockets')
        for c in cells:
            rect(s, *c)
        extrude_profiles(s, RH, CUT, len(cells), ribs)
        kos = [(x - k, y - k, x + k, y + k) for x, y, _, k in pc['holes'] if k]
        for k in kos:
            assert k[0] >= 2.5 and k[1] >= 2.5 and k[2] <= Wp - 2.5 and k[3] <= D - 2.5, (nm, k)
        kos += pc['free_kos']
        if kos:
            s = sketch(0, 'keep-outs')
            for k in kos:
                rect(s, *k)
            extrude_profiles(s, RH, CUT, None, ribs)
        rib_bodies = [b for b in root.bRepBodies if b.name.startswith('Body')]
        tools = adsk.core.ObjectCollection.create()
        for b in rib_bodies:
            tools.add(b)
        ci = feats.combineFeatures.createInput(panel, tools)
        ci.operation = JOIN
        feats.combineFeatures.add(ci)

        s = root.sketches.add(root.yZConstructionPlane); s.name = nm + ' join holes'; s.isVisible = False
        for y in pc['jys']:
            s.sketchCurves.sketchCircles.addByCenterRadius(s.modelToSketchSpace(P(0, mm(oy + y), mm(RH / 2))), mm(1.7))
        coll = adsk.core.ObjectCollection.create()
        for pr in s.profiles:
            coll.add(pr)
        assert coll.count == len(pc['jys'])
        inp = feats.extrudeFeatures.createInput(coll, CUT)
        inp.setTwoSidesDistanceExtent(VI(mm(3000)), VI(mm(3000)))
        inp.participantBodies = [panel]
        feats.extrudeFeatures.add(inp)

        if pc['holes'] or pc['cuts']:
            s = sketch(RH, 'through cuts')
            for x, y, r, _ in pc['holes']:
                s.sketchCurves.sketchCircles.addByCenterRadius(P(mm(ox + x), mm(oy + y), 0), mm(r))
            for c in pc['cuts']:
                rect(s, *c)
            extrude_profiles(s, T, CUT, len(pc['holes']) + len(pc['cuts']), panel)

        top = mm(RH + T)
        on = lambda v, t: abs(v - t) < 1e-4
        perim = adsk.core.ObjectCollection.create()
        bx = (mm(ox), mm(ox + Wp)); by = (mm(oy), mm(oy + D))
        for e in panel.edges:
            g = e.geometry
            if isinstance(g, adsk.core.Line3D):
                a, b = e.startVertex.geometry, e.endVertex.geometry
                if on(a.z, top) and on(b.z, top) and (any(on(a.x, v) and on(b.x, v) for v in bx) or any(on(a.y, v) and on(b.y, v) for v in by)):
                    perim.add(e)
        assert perim.count >= 3, (nm, perim.count)
        ci = feats.chamferFeatures.createInput2()
        ci.chamferEdgeSets.addEqualDistanceChamferEdgeSet(perim, VI(mm(0.4)), False)
        feats.chamferFeatures.add(ci)

        legend = []
        if pc['texts']:
            s = sketch(RH + T - 0.6, 'legend text')
            for txt, h, x1, y1, x2, y2, al in pc['texts']:
                ti = s.sketchTexts.createInput2(txt, mm(h))
                ti.setAsMultiLine(P(mm(ox + x1), mm(oy + y1), 0), P(mm(ox + x2), mm(oy + y2), 0), C if al == 'C' else L,
                                  adsk.core.VerticalAlignments.MiddleVerticalAlignment, 0)
                ti.fontName = 'Arial'
                ti.textStyle = adsk.fusion.TextStyles.TextStyleBold
                st = s.sketchTexts.add(ti)
                inp = feats.extrudeFeatures.createInput(st, NEW)
                inp.setDistanceExtent(False, VI(mm(0.6)))
                legend += [b for b in feats.extrudeFeatures.add(inp).bodies]
            for i, b in enumerate(legend):
                b.name = '%s legend (white) %03d' % (nm, i)
            tools = adsk.core.ObjectCollection.create()
            for b in legend:
                tools.add(b)
            ci = feats.combineFeatures.createInput(panel, tools)
            ci.operation = CUT
            ci.isKeepToolBodies = True
            feats.combineFeatures.add(ci)
        assert not [b for b in root.bRepBodies if b.name.startswith('Body')], nm
        return [panel] + legend

    # flat builds at an offset, exported, then moved into place
    BZ, RR_ = (1000.0, 0.0), (1000.0, 400.0)
    printed = {}
    for pc in bezel_pieces(*BZ) + rear_pieces(*RR_):
        printed[pc['name']] = build(pc)

    # screen module placeholder, in the bezel frame (under the skin)
    def flat_box(name, x1, y1, x2, y2, z1, z2):
        s = root.sketches.add(plane(z1) if z1 else root.xYConstructionPlane)
        s.name = name; s.isVisible = False
        s.sketchCurves.sketchLines.addTwoPointRectangle(P(mm(x1), mm(y1), 0), P(mm(x2), mm(y2), 0))
        b = extrude_profiles(s, z2 - z1, NEW, 1).bodies.item(0)
        b.name = name
        return b
    screen = flat_box('Screen module 15.6 (placeholder)', BZ[0] + SCR[0], BZ[1] + SCR[1], BZ[0] + SCR[2], BZ[1] + SCR[3], -5.0, 10.0)

    if os.path.isdir(OUT):
        shutil.rmtree(OUT)
    em = des.exportManager
    for nm, bodies in printed.items():
        os.makedirs(os.path.join(OUT, nm), exist_ok=True)
        for b in bodies:
            fn = 'panel.stl' if 'panel' in b.name else 'legend-%s.stl' % b.name.split(' ')[-1]
            o = em.createSTLExportOptions(b, os.path.join(OUT, nm, fn))
            o.meshRefinement = adsk.fusion.MeshRefinementSettings.MeshRefinementHigh
            em.execute(o)

    def move(bodies, origin, xa, ya, za, base):
        m = adsk.core.Matrix3D.create()
        o = P(origin[0] - (base[0] * xa[0] + base[1] * ya[0]), origin[1] - (base[0] * xa[1] + base[1] * ya[1]),
              origin[2] - (base[0] * xa[2] + base[1] * ya[2]))
        m.setWithCoordinateSystem(P(mm(o.x), mm(o.y), mm(o.z)), V(*xa), V(*ya), V(*za))
        coll = adsk.core.ObjectCollection.create()
        for b in bodies:
            coll.add(b)
        mi = feats.moveFeatures.createInput2(coll)
        mi.defineAsFreeMove(m)
        feats.moveFeatures.add(mi)

    c, s_ = math.cos(TILT), math.sin(TILT)
    bez = [b for n in ('SL', 'SMB', 'SMT', 'SR') for b in printed[n]] + [screen]
    # bezel: local (0, 0, 13) -> world (0, 325, 13), tilted 20 degrees back
    move(bez, (0.0, DECK_D + 13 * s_, 13 - 13 * c), (1, 0, 0), (0, c, s_), (0, -s_, c), BZ)
    rear = [b for n in ('RL', 'RM', 'RR') for b in printed[n]]
    # rear: face outward (+y), seen from behind local x runs towards -x
    move(rear, (W, 550.0, -62.0), (-1, 0, 0), (0, 0, 1), (0, 1, 0), RR_)

    bb = printed['SL'][0].boundingBox
    print('SL bbox', [round(v * 10, 1) for v in (bb.minPoint.x, bb.minPoint.y, bb.minPoint.z, bb.maxPoint.x, bb.maxPoint.y, bb.maxPoint.z)])
    bb = printed['RR'][0].boundingBox
    print('RR bbox', [round(v * 10, 1) for v in (bb.minPoint.x, bb.minPoint.y, bb.minPoint.z, bb.maxPoint.x, bb.maxPoint.y, bb.maxPoint.z)])
    print('bodies', root.bRepBodies.count)
