"""The dive's two pitch statistics: the ramp's average (~39) and where it ends (~46.5).

    python3 tools/dive_pitch.py runs/steady/nlamsweep runs/atlas/mapsweep ...

Each argument is a directory holding a `best.csv` and an `out/` tree; only rows with
`structure = cyclic` are used. Prints four tables per corpus:

  ramp        the dive is a monotone ramp, not a convergence -- its mean and its end are
              different numbers, and only the mean is flat in dive length
  switch      what is actually invariant at the dive -> snap switch, ranked by spread
  chase       how far the ramp gets toward the hold-gamma fixed point before it is cut off
  criteria    steady-glide critical points, for comparison against the two statistics

The steady glide is evaluated in closed form (see docs/elytra-tick-algebra.md); the
trajectory comes from `load.Profile.replay()`, so there is only one transcription of the
tick map in the tree. Standard library only.
"""

import csv
import math
import os
import statistics as stat
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import load

DRAG_Y, DRAG_Z = 0.98, 0.99          # the exact rationals; the sim's f32 drags differ in the 8th digit
DIVE_PITCH, FLICK_PITCH = 20.0, -30.0        # the thresholds opt.rs::Structure uses


# ---- the steady glide, in closed form

def eq_pitch(p):
    """(v_y*, v_z*) of the constant-pitch map at a nose-down pitch, in degrees.

    From the yaw-zero tick algebra: with L = cos^2 p, the vertical channel is autonomous,
    v_y' = k (v_y - 0.08 + 0.06 L) with k = 0.98 (1 - 0.1 L), and v_z is driven by it.
    """
    L = math.cos(math.radians(p)) ** 2
    k = DRAG_Y * (1.0 - 0.1 * L)
    gy = (0.06 * L - 0.08) / (1.0 - k)
    return k * gy, -(DRAG_Z * 0.09 / (1.0 - DRAG_Z)) * L * gy


def gamma(vy, vz):
    """Flight-path angle in degrees, positive descending (matches opt.rs::gamma)."""
    return math.degrees(math.atan2(-vy, vz))


def argmax_pitch(f, lo=0.01, hi=89.0, step=1e-3):
    """argmax of f(pitch) by dense scan, then a golden-section polish."""
    best, bp, x = -math.inf, lo, lo
    while x <= hi:
        v = f(x)
        if v > best:
            best, bp = v, x
        x += step
    a, b = bp - step, bp + step
    r = (math.sqrt(5.0) - 1.0) / 2.0
    for _ in range(200):
        c, d = b - r * (b - a), a + r * (b - a)
        if f(c) > f(d):
            b = d
        else:
            a = c
    return 0.5 * (a + b)


def pitch_with_gamma(g):
    """The steady glide whose flight-path angle is g. Monotone in pitch, so bisect."""
    lo, hi = 0.0, 89.99
    if g <= gamma(*eq_pitch(lo)):
        return 0.0
    if g >= gamma(*eq_pitch(hi)):
        return hi
    for _ in range(80):
        m = 0.5 * (lo + hi)
        lo, hi = (m, hi) if gamma(*eq_pitch(m)) < g else (lo, m)
    return 0.5 * (lo + hi)


# ---- segmenting one profile

def dive_bounds(ps):
    """(a, b): first dive tick after the entry spike, last before the snap ramp.

    Walks back from the flick to find the dive block, then trims the entry: the leading
    ticks above 60 deg, and the leading descent before the ramp turns monotone. Returns
    None when there is no flick or the dive is under 30 ticks.
    """
    flick = next((i for i, p in enumerate(ps) if p < FLICK_PITCH), None)
    if flick is None:
        return None
    i = flick
    while i > 0 and ps[i] <= DIVE_PITCH:
        i -= 1
    b = i
    while i > 0 and ps[i - 1] > DIVE_PITCH:
        i -= 1
    a = i
    while a < b and ps[a] > 60.0:
        a += 1
    while a < b and ps[a + 1] < ps[a]:
        a += 1
    return (a, b) if b - a > 30 else None


def measure(p):
    """Per-profile dive measurements, or None if the dive is not resolvable."""
    ps = p.pitches
    bb = dive_bounds(ps)
    if bb is None:
        return None
    a, b = bb
    _, _, vys, vzs = p.replay()
    vy0, vz0 = p.v0
    vs = [(vy0, vz0)] + list(zip(vys, vzs))          # vs[t] is the state entering tick t
    gs = [gamma(*vs[t]) for t in range(a, b + 1)]
    k = min(range(len(gs)), key=lambda i: gs[i])
    seg = ps[a:b + 1]
    end = b - 10                                      # clear of the 3-tick terminal spike
    evy, evz = eq_pitch(ps[end])
    return dict(
        n=p.n, lam=p.lam, v0=p.v0, T=b - a + 1,
        start=seg[0], mean=sum(seg) / len(seg), median=stat.median(seg),
        end_pitch=ps[end], end_vy=vs[end][0], end_vz=vs[end][1],
        end_speed=math.hypot(*vs[end]), end_gamma=gamma(*vs[end]),
        v_over_veq=math.hypot(*vs[end]) / math.hypot(evy, evz),
        vy_over_vyeq=vs[end][0] / evy,
        dive_gr=sum(vzs[t] for t in range(a, b + 1)) / -sum(vys[t] for t in range(a, b + 1)),
        gstar=gs[k], gstar_interior=5 < k < len(gs) - 6,
        p_inf=pitch_with_gamma(gs[k]),
    )


def harvest(root):
    out = []
    with open(os.path.join(root, "best.csv")) as f:
        for r in csv.DictReader(f):
            if r["structure"] != "cyclic":
                continue
            m = measure(load.load(os.path.join(root, "out", r["cell"], r["file"])))
            if m is not None:
                out.append(m)
    return out


# ---- reporting

def _bins(recs, width=40, lo0=60, hi0=400, floor=5):
    for lo in range(lo0, hi0, width):
        h = [r for r in recs if lo <= r["T"] < lo + width]
        if len(h) >= floor:
            yield f"{lo}..{lo + width - 1}", h


def report(tag, recs):
    if not recs:
        print(f"\n### {tag}: no resolvable dives")
        return
    v0s = {r["v0"] for r in recs}
    print(f"\n### {tag}: {len(recs)} cyclic profiles with a resolvable dive")
    print(f"    n {min(r['n'] for r in recs)}..{max(r['n'] for r in recs)}   "
          f"lambda {min(r['lam'] for r in recs):g}..{max(r['lam'] for r in recs):g}   "
          f"{len(v0s)} starting velocit{'y' if len(v0s) == 1 else 'ies'}   "
          f"dive length {min(r['T'] for r in recs)}..{max(r['T'] for r in recs)}")

    print(f"\n    ramp -- the mean is flat in T, the end is not (both medians)")
    print(f"    {'T bin':>11} {'cells':>6} {'start':>7} {'mean':>7} {'median':>7} {'terminal':>9} {'dive GR':>8}")
    for lab, h in _bins(recs):
        print(f"    {lab:>11} {len(h):>6} {stat.median(r['start'] for r in h):>7.2f} "
              f"{stat.median(r['mean'] for r in h):>7.2f} {stat.median(r['median'] for r in h):>7.2f} "
              f"{stat.median(r['end_pitch'] for r in h):>9.2f} {stat.median(r['dive_gr'] for r in h):>8.3f}")
    for key, lab in (("mean", "ramp mean"), ("median", "ramp median"), ("end_pitch", "terminal")):
        xs = sorted(r[key] for r in recs)
        print(f"    pooled {lab:>11}: median {stat.median(xs):6.2f}   "
              f"10-90% {xs[int(.1 * len(xs))]:.2f}..{xs[int(.9 * len(xs))]:.2f}")

    print(f"\n    switch -- what is invariant 10 ticks before the dive ends, by spread")
    print(f"    {'quantity':>14} {'median':>9} {'10-90% band':>18} {'rel spread':>11}")
    for key in ("end_pitch", "vy_over_vyeq", "end_vy", "end_gamma", "v_over_veq", "end_speed", "end_vz"):
        xs = sorted(r[key] for r in recs)
        md, lo, hi = stat.median(xs), xs[int(.1 * len(xs))], xs[int(.9 * len(xs))]
        print(f"    {key:>14} {md:>9.3f} {f'{lo:.3f}..{hi:.3f}':>18} {(hi - lo) / abs(md):>10.1%}")

    it = [r for r in recs if r["gstar_interior"]]
    if it:
        print(f"\n    chase -- the hold-gamma fixed point, and how far the ramp gets ({len(it)} with an interior trough)")
        print(f"    {'T bin':>11} {'cells':>6} {'gamma*':>7} {'p_inf':>7} {'terminal':>9} {'gap':>7} {'v/v_eq':>7}")
        for lab, h in _bins(it):
            print(f"    {lab:>11} {len(h):>6} {stat.median(r['gstar'] for r in h):>7.2f} "
                  f"{stat.median(r['p_inf'] for r in h):>7.2f} {stat.median(r['end_pitch'] for r in h):>9.2f} "
                  f"{stat.median(r['p_inf'] - r['end_pitch'] for r in h):>7.2f} "
                  f"{stat.median(r['v_over_veq'] for r in h):>7.3f}")


def criteria():
    print("### steady-glide critical points, for comparison")
    named = [
        ("max glide ratio       v_z/-v_y", lambda p: eq_pitch(p)[1] / -eq_pitch(p)[0]),
        ("distance after a zoom v_z|v|^2/-v_y", lambda p: (lambda v: v[1] * (v[0] ** 2 + v[1] ** 2) / -v[0])(eq_pitch(p))),
        ("max forward speed     v_z", lambda p: eq_pitch(p)[1]),
        ("kinetic energy x v_z  |v|^2 v_z", lambda p: (lambda v: (v[0] ** 2 + v[1] ** 2) * v[1])(eq_pitch(p))),
    ]
    print(f"    {'criterion':>38} {'pitch':>8} {'gamma':>7} {'v_z':>8} {'GR':>7}")
    for lab, f in named:
        p = argmax_pitch(f)
        vy, vz = eq_pitch(p)
        print(f"    {lab:>38} {p:>8.3f} {gamma(vy, vz):>7.3f} {vz:>8.4f} {vz / -vy:>7.3f}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    criteria()
    for root in sys.argv[1:]:
        report(os.path.basename(root.rstrip("/")), harvest(root))
