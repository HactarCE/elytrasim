"""Load the one-tick energy field's grid and curves from `src/bin/field.rs`.

    python3 tools/field_data.py --check      # Rust against field_geometry.py, on a small grid

The Rust binary does the heavy part (branches, exact derivatives, curve location); this runs it
into runs/field/<arguments>/ and loads the .npy files it writes, so a second plot of the same
window reads the cache. Delete the directory to force a recompute, which is needed after any
change to field.rs: the cache is keyed on the arguments, not on the code.
"""

import hashlib
import os
import subprocess
import sys

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TPS = 20
JUMP = 3.0                 # field.rs JUMP: degrees between neighbors that count as a jump
KINDS = {0: "tie", 1: "conversion", 2: "vz=0", 3: "smooth"}
# elytra-vario's chart, blocks/second: vz_lo, vz_hi, vy_lo, vy_hi.
CHART_BPS = (-10.0, 60.0, -30.0, 40.0)


class FieldData:
    """The grid (indexed [vy][vz], velocities in blocks/tick) and the curves on it."""

    def __init__(self, d):
        load = lambda name: np.load(os.path.join(d, f"{name}.npy"))
        self.vz, self.vy = load("vz"), load("vy")
        self.hz, self.hy = self.vz[1] - self.vz[0], self.vy[1] - self.vy[0]
        self.h = max(self.hz, self.hy)
        self.Z, self.Y = np.meshgrid(self.vz, self.vy)
        self.p1, self.G, self.p2, self.g2 = load("p1"), load("G"), load("p2"), load("g2")
        self.stuck = load("stuck").astype(bool)
        self.gy, self.gz = load("gy"), load("gz")
        self.hyy, self.hyz, self.hzz = load("hyy"), load("hyz"), load("hzz")
        jump = np.zeros(self.p1.shape, bool)
        dz = np.abs(np.diff(self.p1, axis=1)) > JUMP
        dy = np.abs(np.diff(self.p1, axis=0)) > JUMP
        jump[:, 1:] |= dz; jump[:, :-1] |= dz; jump[1:] |= dy; jump[:-1] |= dy
        self.jump = jump
        # [(points (N, 2) as (vz, vy), class, kind)]: class +1 trough, -1 ridge, 0 kink only.
        self.curves = []
        path = os.path.join(d, "curves.npy")
        if os.path.exists(path):
            rows = np.load(path)
            for cid in np.unique(rows[:, 0]):
                r = rows[rows[:, 0] == cid]
                self.curves.append((r[:, 3:5], int(r[0, 2]), KINDS[int(r[0, 1])]))

    def ext(self):
        return (self.vz[0] * TPS, self.vz[-1] * TPS, self.vy[0] * TPS, self.vy[-1] * TPS)

    def nearest(self, pts):
        c = np.clip(np.round((pts[:, 0] - self.vz[0]) / self.hz).astype(int), 0, len(self.vz) - 1)
        r = np.clip(np.round((pts[:, 1] - self.vy[0]) / self.hy).astype(int), 0, len(self.vy) - 1)
        return r, c


def binary():
    subprocess.run(["cargo", "build", "--release", "--quiet", "--bin", "field"], cwd=ROOT,
                   check=True)
    return os.path.join(ROOT, "target", "release", "field")


def load(window_bps=CHART_BPS, samples=(1050,), pad=0.05, curves=True):
    """Run field.rs for this window (blocks/second) unless cached, and load it."""
    args = ["--window", ",".join(f"{x:g}" for x in window_bps),
            "--samples", ",".join(f"{x:g}" for x in samples), "--pad", f"{pad:g}"]
    if not curves:
        args.append("--no-curves")
    key = hashlib.sha1(" ".join(args).encode()).hexdigest()[:12]
    d = os.path.join(ROOT, "runs", "field", key)
    if not os.path.exists(os.path.join(d, "spec.txt")):
        subprocess.run([binary(), *args, "--out", d], check=True)
    return FieldData(d)


def check():
    """Rust's grid against field_geometry.py's, sample for sample, on a small chart grid."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import field_geometry as fg
    F = load(samples=(140,), curves=False)
    p1, G, p2, g2 = fg.branches(F.Y, F.Z)
    st = fg.stuck(F.Y, F.Z, p1)
    d = fg.derivatives(F.Y, F.Z, p1, st)
    print(f"{F.p1.size} samples")
    print(f"  max |G diff|          {np.abs(G - F.G).max():.2e}")
    print(f"  max |p1 diff| (deg)   {np.abs(p1 - F.p1).max():.2e}")
    print(f"  stuck disagreements   {(st != F.stuck).sum()}")
    for name, a, b in zip(("gy", "gz", "hyy", "hyz", "hzz"), d,
                          (F.gy, F.gz, F.hyy, F.hyz, F.hzz)):
        scale = np.abs(b).max()
        print(f"  max |{name} diff| / max|{name}|  {np.abs(a - b).max() / scale:.2e}")


if __name__ == "__main__":
    if "--check" in sys.argv:
        check()
