"""Screen flap (HWB-10): strut geometry and the load it carries. Plain Python 3, no Fusion.

World frame of the desk: y toward the back, z up; the hinge axis runs along x at (y 345, z 13), the deck face is
z 13. The flap frame (used by klep.py) has X across, Y from the hinge along the flap, Z out of the flap's
underside toward its face. `python3 calc.py` prints the table and asserts the criteria the build relies on.
"""
import math

HY, HZ = 345.0, 13.0          # hinge axis
D, E = 115.0, 9.0             # strut pivot: 115 mm along the flap from the hinge, 9 mm inside the flap
L = 95.0                      # strut, pivot centre to foot-rod centre
ROD_R = 4.0                   # foot rod, 8 mm stainless
GROOVE = 5.0                  # sawtooth grooves sunk into the deck: floor at z 8, rod centre at z 12
FOOT_Z = HZ - GROOVE + ROD_R
STEPS = (15, 25, 35, 45)
PUSH_N, PUSH_Y = 20.0, 250.0  # HW-56: 20 N at the top centre of the screen, square to its face


def pivot(a):
    r = math.radians(a)
    return HY + D * math.cos(r) - E * math.sin(r), HZ + D * math.sin(r) + E * math.cos(r)


def foot(a):
    """Foot-rod centre (y, z) when the flap stands at a degrees."""
    py, pz = pivot(a)
    dz = pz - FOOT_Z
    return py + math.sqrt(L * L - dz * dz), FOOT_Z


def strut_force(a, mass_kg, cg_y, cg_z=12.0):
    """Compression in the strut (both arms together), N: flap weight plus the HW-56 push, moments about the hinge."""
    r = math.radians(a)
    # centre of gravity in the flap frame (cg_y along, cg_z up from the underside) -> horizontal lever about the hinge
    lever_w = (cg_y * math.cos(r) - cg_z * math.sin(r)) / 1000
    m = mass_kg * 9.81 * lever_w + PUSH_N * PUSH_Y / 1000
    (py, pz), (fy, fz) = pivot(a), foot(a)
    uy, uz = (fy - py) / L, (fz - pz) / L
    arm = abs((py - HY) * uz - (pz - HZ) * uy) / 1000   # perpendicular distance hinge -> strut line
    return m / arm


def rod_push(a, f):
    """Horizontal (toward the back) and downward parts of the strut force on the rack."""
    (py, pz), (fy, fz) = pivot(a), foot(a)
    return f * (fy - py) / L, f * (pz - fz) / L


if __name__ == '__main__':
    feet = [foot(a)[0] for a in STEPS]
    for a, fy in zip(STEPS, feet):
        f = strut_force(a, 2.6, 125.0)
        h, v = rod_push(a, f)
        print('%2d deg  pivot y %.1f z %.1f  foot y %.1f  strut %.0f N  rack %.0f N back, %.0f N down'
              % (a, *pivot(a), fy, f, h, v))
    # the foot lands inside the deck behind the hinge and in front of the rear wall (y 551)
    assert 440 < min(feet) and max(feet) + ROD_R < 551 - 5, feet
    # grooves 18.3 mm long (rod 8 + 0.3 play + 10 mm ramp) must not run into each other
    assert all(feet[i] - feet[i + 1] > 18.3 for i in range(3)), feet
    # folded, the rod lies under the flap in front of the top clamp bar (Y 232)
    assert D + L + ROD_R < 222
    # the worst case is 15 degrees and stays near the 120 N the brief sized for
    worst = max(strut_force(a, 2.6, 125.0) for a in STEPS)
    assert worst == strut_force(15, 2.6, 125.0) and worst < 150, worst
    print('ok')
