# Merge each piece's STL bodies into one panel + one legend file, flipped face-down at the origin.
import struct, sys, glob, os
src, dst = sys.argv[1], sys.argv[2]
PIECES = {'SL': (1000, 0, 250), 'SMB': (1190, 0, 25), 'SMT': (1190, 221, 29), 'SR': (1380, 0, 250),
          'RL': (1000, 400, 142), 'RM': (1190, 400, 142), 'RR': (1380, 400, 142)}
TOP = 13.0

def load(p):
    d = open(p, 'rb').read()
    n = struct.unpack('<I', d[80:84])[0]
    assert len(d) == 84 + 50 * n, p
    return [struct.unpack('<12fH', d[84 + 50 * i:134 + 50 * i]) for i in range(n)]

def write(out, tris, ox, oy, D):
    pts = []
    with open(out, 'wb') as f:
        f.write(b'CMDMX panel'.ljust(80, b' ') + struct.pack('<I', len(tris)))
        for t in tris:
            # rotate pi about X (y -> -y, z -> -z), then shift to the origin
            v = [t[0], -t[1], -t[2]]
            for k in (3, 6, 9):
                p = (t[k] - ox, oy + D - t[k + 1], TOP - t[k + 2])
                v += p; pts.append(p)
            f.write(struct.pack('<12fH', *v, 0))
    lo = [round(min(p[i] for p in pts), 2) for i in range(3)]
    hi = [round(max(p[i] for p in pts), 2) for i in range(3)]
    print(os.path.basename(out), len(tris), 'tris', lo, hi)
    return lo, hi

os.makedirs(dst, exist_ok=True)
for nm, (ox, oy, D) in PIECES.items():
    lo, hi = write(f'{dst}/cmdmx-{nm}-panel-black.stl', load(f'{src}/{nm}/panel.stl'), ox, oy, D)
    W = hi[0] - lo[0]
    assert lo == [0, 0, 0] and abs(hi[1] - D) < 0.01 and abs(hi[2] - TOP) < 0.01, (nm, lo, hi)
    leg = [t for f in sorted(glob.glob(f'{src}/{nm}/legend-*.stl')) for t in load(f)]
    if not leg:
        continue
    llo, lhi = write(f'{dst}/cmdmx-{nm}-legend-white.stl', leg, ox, oy, D)
    assert llo[2] >= -0.01 and lhi[2] <= 0.61 and lhi[0] <= W, (nm, llo, lhi)
