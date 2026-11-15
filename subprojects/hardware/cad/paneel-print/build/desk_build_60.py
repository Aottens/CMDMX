import adsk.core, adsk.fusion, math, os

VI = adsk.core.ValueInput.createByReal
P = adsk.core.Point3D.create
NEW = adsk.fusion.FeatureOperations.NewBodyFeatureOperation
JOIN = adsk.fusion.FeatureOperations.JoinFeatureOperation
CUT = adsk.fusion.FeatureOperations.CutFeatureOperation
T, RH, KP = 3.0, 10.0, 19.05
DB = 125.0  # depth of the back row (60 mm preset faders)

# Fader geometry, piece-local y. psm: Bourns PSM 100 mm motor fader (HWB-01 testbox, proven).
# pta: Bourns PTA6043 60 mm, body 75 x 9, M2 holes 71 apart (datasheet), centre y 62.
FADER = {
    'psm': dict(recess=(6.8, 3.7, 156.3), slot=(1.7, 30.5, 139.5), screws=(27.0, 147.0), sr=1.7,
                keepout=(8, 3, 157), tick0=35, tick=10),
    'pta': dict(recess=(4.8, 24.2, 99.8), slot=(1.2, 29.7, 94.3), screws=(26.5, 97.5), sr=1.1,
                keepout=(6, 23, 101), tick0=32, tick=6),
}


def mm(v):
    return v / 10.0


def fader_strips(first, nstrips, preset):
    faders, groups = [], []
    for s in range(nstrips):
        g = []
        for j in (0, 1):
            fx = 66 * s + 16.5 + 33 * j
            faders.append((fx, str(first + 2 * s + j), 'pta' if preset else 'psm'))
            for ky in ((109.5,) if preset else (166.0, 185.05)):
                g.append((fx, ky))
        groups.append(g)
    return dict(faders=faders, groups=groups, holes=[], texts=[],
                xs=[33.0 * k for k in range(1, nstrips * 2)])


def row(x0, y, n):
    return [(x0 + KP * i, y) for i in range(n)]


def pieces():
    out = []
    p = fader_strips(1, 3, False); p.update(name='PB1', ox=0, oy=0, W=198, D=200); out.append(p)
    p = fader_strips(7, 2, False); p.update(name='PB2', ox=198, oy=0, W=132, D=200); out.append(p)
    p = fader_strips(11, 3, True); p.update(name='PR1', ox=0, oy=200, W=198, D=DB); out.append(p)
    p = fader_strips(17, 2, True); p.update(name='PR2', ox=198, oy=200, W=132, D=DB); out.append(p)
    keypad = [(156 + KP * c, 46.4 + KP * r) for r in range(4) for c in range(4)]
    out.append(dict(name='PG1', ox=330, oy=0, W=240, D=200,
                    faders=[(16.5, 'GM', 'psm')],
                    groups=[[(16.5, 166.0)], row(50, 166.0, 10) + row(50, 185.05, 10), keypad],
                    holes=[(70, 45, 5.0, 16), (120, 45, 5.0, 16), (70, 105, 5.0, 16), (120, 105, 5.0, 16)],
                    texts=[], xs=[33.0, 95.0, 141.0]))
    grp = row(50, 22.0, 8)
    letters = [(l, 3, x - 6, 31.5, x + 6, 36.5, 'C') for l, (x, _) in zip('I P C G B E S FX'.split(), grp)]
    out.append(dict(name='PG2', ox=330, oy=200, W=240, D=DB, faders=[],
                    groups=[grp, row(50, 52.0, 10) + row(50, 71.05, 10), row(50, 109.5, 10)],
                    holes=[(16.5, 45, 3.5, 10)],
                    texts=letters + [('LED', 3, 6.5, 53, 26.5, 58, 'C')], xs=[33.0, 95.0, 141.0]))
    return out


def run(_context: str):
    app = adsk.core.Application.get()
    old = app.activeDocument
    od = adsk.fusion.Design.cast(old.products.itemByProductType('DesignProductType'))
    assert old.name.startswith('Untitled') and od.rootComponent.bRepBodies.count == 372, old.name
    old.close(False)
    app.documents.add(adsk.core.DocumentTypes.FusionDesignDocumentType)
    des = adsk.fusion.Design.cast(app.activeProduct)
    des.designType = adsk.fusion.DesignTypes.ParametricDesignType
    des.fusionUnitsManager.distanceDisplayUnits = adsk.fusion.DistanceUnits.MillimeterDistanceUnits
    root = des.rootComponent
    feats = root.features
    planes = {}

    def plane(z):
        if z not in planes:
            pi = root.constructionPlanes.createInput()
            pi.setByOffset(root.xYConstructionPlane, VI(mm(z)))
            planes[z] = root.constructionPlanes.add(pi)
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
    report = []

    for pc in pieces():
        nm, ox, oy, W, D = pc['name'], pc['ox'], pc['oy'], pc['W'], pc['D']
        assert W <= 250 and D <= 250, nm
        keys = [k for g in pc['groups'] for k in g]
        line_y = 153.0 if D == 200 else 99.6

        def sketch(z, label):
            s = root.sketches.add(plane(z) if z else root.xYConstructionPlane)
            s.name = nm + ' ' + label
            return s

        def rect(s, x1, y1, x2, y2):
            s.sketchCurves.sketchLines.addTwoPointRectangle(P(mm(ox + x1), mm(oy + y1), 0), P(mm(ox + x2), mm(oy + y2), 0))

        def circle(s, x, y, r):
            s.sketchCurves.sketchCircles.addByCenterRadius(P(mm(ox + x), mm(oy + y), 0), mm(r))

        s = sketch(RH, 'skin'); rect(s, 0, 0, W, D)
        panel = extrude_profiles(s, T, NEW, 1).bodies.item(0)
        panel.name = nm + ' panel (black)'

        s = sketch(0, 'rib block'); rect(s, 0, 0, W, D)
        ribs = extrude_profiles(s, RH, NEW, 1).bodies.item(0)
        xs = [0.0] + pc['xs'] + [float(W)]
        ys = [0.0, 45.0, 90.0, 135.0, float(D)] if D == 200 else [0.0, 45.0, 90.0, float(D)]
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
        want = (W * D - sum((c[2] - c[0]) * (c[3] - c[1]) for c in cells)) * RH
        got = ribs.volume * 1000
        assert abs(got - want) < 0.01 * want, (nm, got, want)

        kos = []
        for fx, _, kind in pc['faders']:
            hw, y1, y2 = FADER[kind]['keepout']
            kos.append((fx - hw, y1, fx + hw, y2))
        for g in pc['groups']:
            kos.append((min(x for x, _ in g) - 10, min(y for _, y in g) - 10, max(x for x, _ in g) + 10, max(y for _, y in g) + 10))
        for x, y, _, k in pc['holes']:
            kos.append((x - k, y - k, x + k, y + k))
        for k in kos:
            assert k[0] >= 2.5 and k[1] >= 2.5 and k[2] <= W - 2.5 and k[3] <= D - 2.5, (nm, k)
        s = sketch(0, 'keep-outs')
        for k in kos:
            rect(s, *k)
        extrude_profiles(s, RH, CUT, None, ribs)
        assert ribs.volume * 1000 < got, (nm, 'keep-outs cut nothing')

        tools = adsk.core.ObjectCollection.create(); tools.add(ribs)
        ci = feats.combineFeatures.createInput(panel, tools)
        ci.operation = JOIN
        feats.combineFeatures.add(ci)

        s = root.sketches.add(root.yZConstructionPlane); s.name = nm + ' join holes'
        jys = (25, 112, 175) if D == 200 else (25, 65, 110)
        for y in jys:
            s.sketchCurves.sketchCircles.addByCenterRadius(s.modelToSketchSpace(P(0, mm(oy + y), mm(RH / 2))), mm(1.7))
        coll = adsk.core.ObjectCollection.create()
        for pr in s.profiles:
            coll.add(pr)
        assert coll.count == 3
        inp = feats.extrudeFeatures.createInput(coll, CUT)
        inp.setTwoSidesDistanceExtent(VI(mm(600)), VI(mm(600)))
        inp.participantBodies = [panel]
        feats.extrudeFeatures.add(inp)

        if pc['faders']:
            s = sketch(RH, 'fader recesses')
            for fx, _, kind in pc['faders']:
                hw, y1, y2 = FADER[kind]['recess']
                rect(s, fx - hw, y1, fx + hw, y2)
            extrude_profiles(s, 0.5, CUT, len(pc['faders']), panel)
        s = sketch(RH, 'key recesses')
        for x, y in keys:
            rect(s, x - 8.5, y - 8.5, x + 8.5, y + 8.5)
        extrude_profiles(s, 1.5, CUT, len(keys), panel)

        s = sketch(RH, 'through cuts')
        for fx, _, kind in pc['faders']:
            f = FADER[kind]
            hw, y1, y2 = f['slot']
            rect(s, fx - hw, y1, fx + hw, y2)
            for sy in f['screws']:
                circle(s, fx, sy, f['sr'])
        for x, y in keys:
            rect(s, x - 7, y - 7, x + 7, y + 7)
        for x, y, r, _ in pc['holes']:
            circle(s, x, y, r)
        extrude_profiles(s, T, CUT, 3 * len(pc['faders']) + len(keys) + len(pc['holes']), panel)

        # countersinks only for the M3 flat heads of the PSM; the PTA takes M2 button heads
        top = mm(RH + T)
        on = lambda v, t: abs(v - t) < 1e-4
        csk = adsk.core.ObjectCollection.create()
        perim = adsk.core.ObjectCollection.create()
        bx = (mm(ox), mm(ox + W)); by = (mm(oy), mm(oy + D))
        for e in panel.edges:
            g = e.geometry
            if isinstance(g, adsk.core.Circle3D) and on(g.center.z, top) and on(g.radius, mm(1.7)):
                csk.add(e)
            if isinstance(g, adsk.core.Line3D):
                a, b = e.startVertex.geometry, e.endVertex.geometry
                if on(a.z, top) and on(b.z, top) and (any(on(a.x, v) and on(b.x, v) for v in bx) or any(on(a.y, v) and on(b.y, v) for v in by)):
                    perim.add(e)
        assert csk.count == 2 * sum(1 for f in pc['faders'] if f[2] == 'psm'), (nm, csk.count)
        assert perim.count == 4, (nm, perim.count)
        for coll, dist in ((csk, 1.2), (perim, 0.4)):
            if coll.count:
                ci = feats.chamferFeatures.createInput2()
                ci.chamferEdgeSets.addEqualDistanceChamferEdgeSet(coll, VI(mm(dist)), False)
                feats.chamferFeatures.add(ci)

        zl = RH + T - 0.6
        s = sketch(zl, 'legend marks')
        nmarks = 0
        for fx, _, kind in pc['faders']:
            f = FADER[kind]
            for i in range(11):
                y = f['tick0'] + f['tick'] * i
                rect(s, fx - 13 if i % 5 == 0 else fx - 11, y - 0.3, fx - 8, y + 0.3)
                nmarks += 1
        rect(s, 3, line_y, W - 3, line_y + 0.8)
        nmarks += 1
        legend = [b for b in extrude_profiles(s, 0.6, NEW, nmarks).bodies]

        s = sketch(zl, 'legend text')
        texts = list(pc['texts'])
        for fx, lab, kind in pc['faders']:
            f = FADER[kind]
            texts.append((lab, 6 if len(lab) > 1 else 7, fx - 8, 7, fx + 8, 21, 'C'))
            for t, i in (('0', 0), ('5', 5), ('10', 10)):
                y = f['tick0'] + f['tick'] * i
                texts.append((t, 3, fx + 7.5, y - 3, fx + 15, y + 3, 'L'))
        for txt, h, x1, y1, x2, y2, al in texts:
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
        report.append((nm, W, D, round(panel.volume * 1000), len(legend), len(keys), len(pc['faders'])))

    found = {}
    for lib in app.materialLibraries:
        for i in range(lib.appearances.count):
            a = lib.appearances.item(i)
            n = a.name.lower()
            if 'plastic' in n and 'matte' in n and 'black' in n and 'black' not in found:
                found['black'] = a
            if 'plastic' in n and 'white' in n and 'white' not in found:
                found['white'] = a
    for k, a in found.items():
        da = des.appearances.itemByName(a.name) or des.appearances.addByCopy(a, a.name)
        for b in root.bRepBodies:
            if (k == 'black') == ('panel' in b.name):
                b.appearance = da
    print('bodies', root.bRepBodies.count)
    for r in report:
        print(r)
