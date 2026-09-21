"""The glide phase -- the pitch-0 hold -- and the one-tick rule that ends it.

    python3 tools/glide_phase.py runs/steady/nlamsweep runs/atlas/nsweepv0fine ...
    python3 tools/glide_phase.py --switch runs/steady/nlamsweep

Each argument is a directory holding a `best.csv` and an `out/` tree; only rows with
`structure = cyclic` are used. The hold's *pitch* has been known for a long time (it is zero);
what this measures is its *stopping time*. Four tables per corpus:

  exit      how many ticks the candidate rule misses the optimum's own departure by
  price     the same miss, split by lambda -- it is monotone in the price on distance
  menu      competing stopping rules, on a stride-sampled subset (the dTE scans are slow)
  entry     the state where the hold begins, ranked by spread -- nothing is invariant there,
            and `--switch` is why: the entry's threshold moves with `v_y`

`--switch` is the exact version, and needs a periodic corpus and a built `target/release/myopic`:
it solves the costate per cycle and checks the two corner conditions below against the ticks the
optimum actually enters and leaves the hold. Both land inside one tick on every steady cell.

The rule under test is `v_z has peaked`: leave on the first tick at which holding pitch 0 would
no longer raise forward speed. It takes no fitted constant and looks one tick ahead. In exact
rationals the hold moves `v_z` by `-0.01 v_z - 0.0891 v_y + 0.001782`, so it fires on the line
`v_z >= 0.1782 - 8.91 v_y` through the pitch-0 steady glide -- but the margin at the crossing is
only ~5e-4 b/tick and the sim's drags are `f32`, so ask the tick map, not the line.

The trajectory comes from `load.Profile.replay()` and the steady glide from `dive_pitch`, so
there is only one transcription of each in the tree. Standard library only.
"""

import csv
import math
import os
import statistics as stat
import sys
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import load
from dive_pitch import eq_pitch, gamma

DRAG_Y, DRAG_Z = 0.9800000190734863, 0.9900000095367432   # the sim's f32 drags, as load.py has them
GRAVITY = 0.08
BEST_GLIDE_GAMMA = 5.6533        # flight-path angle of the pitch-0 steady glide (`myopic crit`)
MENU_SAMPLE = 200                # profiles per corpus for the slow rules

# The two corner conditions. For p >= 0 the map depends on pitch only through L = cos^2 p, which
# is maximal at 0, so entering the hold is the sign of d(mu.f)/dL at L = 1; for p < 0 the
# forward-to-up branch switches on linearly, so leaving it is a one-sided derivative in lean and
# the v_z that multiplies both components cancels. Every constant is read off the tick map.
def s_in(vy, mu_y, mu_z):
    """Hold 0 rather than pitch down?"""
    return DRAG_Y * (0.056 - 0.1 * vy) * mu_y - 0.09 * DRAG_Z * (vy + 0.04) * mu_z


def s_out(vy, mu_y, mu_z):
    """Hold 0 rather than pitch up?  0.128/0.036 = (3.2/0.9) is the forward-to-up exchange rate."""
    return 0.036 * DRAG_Z * mu_z - 0.128 * DRAG_Y * mu_y


def switch_report(root):
    """Where the two corner conditions flip, against where the optimum actually switches."""
    import io
    import subprocess
    rows = [r for r in csv.DictReader(open(os.path.join(root, "best.csv")))
            if r["structure"] == "cyclic"]
    res = []
    for r in rows:
        path = os.path.join(root, "out", r["cell"], r["file"])
        p = load.load(path)
        bb = hold_bounds(p.pitches)
        if bb is None:
            continue
        start, out = bb
        txt = subprocess.run(["./target/release/myopic", "adjoint", path, "dump"],
                             capture_output=True, text=True).stdout
        if "not a closed cycle" in txt or "singular" in txt:
            continue          # the periodic adjoint needs a cycle that closes
        d = {int(x["t"]): x for x in csv.DictReader(io.StringIO(txt[txt.index("t,pitch,vy,vz"):]))}
        g = lambda t: (float(d[t]["vy"]), float(d[t]["mu_y"]), float(d[t]["mu_z"]))
        hi = min(len(p.pitches), out + 40)
        t_in = next((t for t in range(max(1, start - 40), out) if s_in(*g(t)) >= 0), None)
        t_out = next((t for t in range(start, hi) if s_out(*g(t)) < 0), None)
        if t_in is None or t_out is None:
            continue
        res.append((t_in - start, t_out - out))
    print(f"\n### {os.path.basename(root.rstrip('/'))}: {len(res)} periodic cycles")
    print(f"    {'condition':>14} {'median':>8} {'exact':>8} {'inside 1':>9}   distribution")
    for i, lab in ((0, "entry switch"), (1, "exit switch")):
        v = [x[i] for x in res]
        print(f"    {lab:>14} {stat.median(v):>+8.0f} {100 * v.count(0) / len(v):>7.1f}% "
              f"{100 * sum(1 for x in v if abs(x) <= 1) / len(v):>8.1f}%   "
              f"{dict(sorted(Counter(v).items()))}")


def tick(vy, vz, pitch):
    """One tick of the yaw-zero map. Same transcription as `load.Profile.replay`."""
    lean = math.radians(pitch)
    look_z = math.cos(lean)
    look_hor = abs(look_z)
    move_hor = abs(vz)
    lift = look_z ** 2
    vy += GRAVITY * (-1.0 + lift * 0.75)
    if vy < 0.0 and look_hor > 0.0:
        conv = vy * -0.1 * lift
        vy += conv
        vz += look_z * conv / look_hor
    if lean < 0.0 and look_hor > 0.0:
        conv = move_hor * -math.sin(lean) * 0.04
        vy += conv * 3.2
        vz -= look_z * conv / look_hor
    if look_hor > 0.0:
        vz += (look_z / look_hor * move_hor - vz) * 0.1
    return vy * DRAG_Y, vz * DRAG_Z


def held_dte(vy, vz, pitch, n):
    """Total-energy change, in blocks, of holding `pitch` for `n` ticks. KE = |v|^2/2g, PE = y."""
    y, vy0, vz0 = 0.0, vy, vz
    for _ in range(n):
        vy, vz = tick(vy, vz, pitch)
        y += vy
    return (vy * vy + vz * vz - vy0 * vy0 - vz0 * vz0) / (2.0 * GRAVITY) + y


def dte_argmax_left_zero(vy, vz, n, step=2.0):
    """Has the n-tick dTE argmax left pitch 0? Coarse: the question is which side, not where."""
    best, bp, p = -math.inf, 0.0, -90.0
    while p <= 10.0:
        v = held_dte(vy, vz, p, n)
        if v > best:
            best, bp = v, p
        p += step
    return bp < -1.0


# ---- the menu. Each rule answers "leave now?" from the state entering a tick.

RULES = [
    ("v_z has peaked          dv_z(0) <= 0", lambda y, z: tick(y, z, 0.0)[1] <= z),
    ("speed has peaked        d|v|(0) <= 0",
     lambda y, z: math.hypot(*tick(y, z, 0.0)) <= math.hypot(y, z)),
    ("glide ratio at its steady max", lambda y, z: gamma(y, z) <= BEST_GLIDE_GAMMA),
    ("v_y >= -0.260   (the policy's constant)", lambda y, z: y >= -0.260),
    ("1-tick dTE argmax leaves 0", lambda y, z: dte_argmax_left_zero(y, z, 1)),
    ("20-tick dTE argmax leaves 0", lambda y, z: dte_argmax_left_zero(y, z, 20)),
]
HEADLINE = 0


# ---- segmenting one profile

def hold_bounds(ps, flat=5.0, leave=-5.0):
    """(first tick of the hold, first tick of the departure), or None.

    Anchored on the flick, which is unambiguous: walk back from the first tick below -30 through
    the departure ramp to the last tick still above `leave`, then back through the flat run. The
    band matters less than it looks -- `--flat` reruns everything at other widths.
    """
    committed = next((i for i, p in enumerate(ps) if p < -30.0), None)
    if committed is None or committed == 0:
        return None
    out = committed
    while out > 0 and ps[out - 1] < leave:
        out -= 1
    start = out - 1
    if start < 1:
        return None
    while start > 0 and abs(ps[start - 1]) <= flat:
        start -= 1
    return (start, out) if out - start >= 3 else None


def measure(p, flat, menu):
    """Per-profile hold measurements, or None when the hold is not resolvable."""
    bb = hold_bounds(p.pitches, flat=flat)
    if bb is None:
        return None
    start, out = bb
    _, _, vys, vzs = p.replay()
    vs = [p.v0] + list(zip(vys, vzs))                 # vs[t] is the state entering tick t
    hi = min(len(p.pitches), out + 40)
    rules = RULES if menu else RULES[HEADLINE:HEADLINE + 1]
    offs = []
    for _, fires in rules:
        t = next((t for t in range(start, hi) if fires(*vs[t])), None)
        offs.append(None if t is None else t - out)
    vy, vz = vs[start]
    eq_vy, _ = eq_pitch(p.pitches[start - 1])         # the dive pitch the hold was entered from
    lo = max(1, start - 30)
    return dict(
        n=p.n, lam=p.lam, v0=p.v0, hold=out - start, offs=offs,
        vy=vy, vz=vz, speed=math.hypot(vy, vz), gam=gamma(vy, vz), gr=vz / -vy,
        vy_over_vyeq=vy / eq_vy,
        vy_min_off=min(range(lo, out), key=lambda t: vs[t][0]) - start,
    )


def harvest(root, flat=5.0, menu_sample=None):
    """Every cyclic profile, with the slow rules run on `menu_sample` of the lambda = 0 ones."""
    rows = [r for r in csv.DictReader(open(os.path.join(root, "best.csv")))
            if r["structure"] == "cyclic"]
    menu = set()
    if menu_sample:
        # the menu is a like-for-like comparison, so hold the price fixed where the corpus lets us
        pool = [i for i, r in enumerate(rows) if float(r["lam"]) == 0.0] or list(range(len(rows)))
        menu = set(pool[::max(1, len(pool) // menu_sample)])
    out = []
    for i, r in enumerate(rows):
        m = measure(load.load(os.path.join(root, "out", r["cell"], r["file"])), flat, i in menu)
        if m is not None:
            out.append(m)
    return out


# ---- reporting

def _spread(recs, key):
    xs = sorted(r[key] for r in recs)
    return stat.median(xs), xs[int(.1 * len(xs))], xs[int(.9 * len(xs))]


def _score(offs):
    """(median, mean, exact %, within-1 %) of a rule's miss, in ticks."""
    v = [o for o in offs if o is not None]
    if not v:
        return None
    return (stat.median(v), sum(v) / len(v),
            100 * v.count(0) / len(v), 100 * sum(1 for x in v if abs(x) <= 1) / len(v))


def report(tag, recs, flats):
    if not recs:
        print(f"\n### {tag}: no resolvable holds")
        return
    lams = sorted({r["lam"] for r in recs})
    print(f"\n### {tag}: {len(recs)} cyclic profiles with a resolvable hold")
    print(f"    n {min(r['n'] for r in recs)}..{max(r['n'] for r in recs)}   "
          f"lambda {min(lams):g}..{max(lams):g}   "
          f"{len({r['v0'] for r in recs})} starting velocities   "
          f"hold {min(r['hold'] for r in recs)}..{max(r['hold'] for r in recs)} ticks, "
          f"median {stat.median(r['hold'] for r in recs):.0f}")

    print(f"\n    exit -- `{RULES[HEADLINE][0].split('  ')[0]}` against the optimum's own departure")
    print(f"    {'hold band':>12} {'cells':>6} {'median':>8} {'mean':>7} {'exact':>7} {'|miss|<=1':>10}")
    for lab, h in flats:
        m = _score([r["offs"][HEADLINE] for r in h])
        print(f"    {lab:>12} {len(h):>6} {m[0]:>8.1f} {m[1]:>7.2f} {m[2]:>6.1f}% {m[3]:>9.1f}%")

    if len(lams) > 1:
        print(f"\n    price -- the miss is monotone in lambda: paid for distance, the optimum")
        print(f"             holds the glide past the peak; charged for it, it leaves early")
        print(f"    {'lambda':>8} {'cells':>6} {'median':>8} {'mean':>7} {'exact':>7} {'|miss|<=1':>10}")
        by = defaultdict(list)
        for r in recs:
            by[r["lam"]].append(r["offs"][HEADLINE])
        for k in sorted(by):
            m = _score(by[k])
            print(f"    {k:>8g} {len(by[k]):>6} {m[0]:>8.1f} {m[1]:>7.2f} {m[2]:>6.1f}% {m[3]:>9.1f}%")

    menu = [r for r in recs if len(r["offs"]) == len(RULES)]
    if menu:
        lam = f"{menu[0]['lam']:g}" if len({r['lam'] for r in menu}) == 1 else "all"
        print(f"\n    menu -- other stopping rules, {len(menu)} profiles at lambda {lam}")
        print(f"    {'rule':>42} {'median':>8} {'mean':>7} {'exact':>7} {'|miss|<=1':>10}")
        for i, (name, _) in enumerate(RULES):
            m = _score([r["offs"][i] for r in menu])
            if m is None:
                print(f"    {name:>42} {'never fires':>34}")
            else:
                print(f"    {name:>42} {m[0]:>8.1f} {m[1]:>7.2f} {m[2]:>6.1f}% {m[3]:>9.1f}%")

    print(f"\n    entry -- what is invariant where the hold begins, by spread")
    print(f"    {'quantity':>26} {'median':>9} {'10-90% band':>20} {'rel spread':>11}")
    for key, lab in (("gam", "gamma"), ("gr", "glide ratio v_z/-v_y"), ("vy", "v_y"),
                     ("speed", "speed"), ("vz", "v_z"), ("vy_over_vyeq", "v_y / eq_v_y(dive pitch)")):
        md, lo, hi = _spread(recs, key)
        print(f"    {lab:>26} {md:>9.3f} {f'{lo:.3f}..{hi:.3f}':>20} {(hi - lo) / abs(md):>10.1%}")
    c = Counter(r["vy_min_off"] for r in recs)
    print(f"    the sink rate bottoms out {-stat.median(r['vy_min_off'] for r in recs):.0f} ticks "
          f"before the hold begins ({100 * max(c.values()) / len(recs):.0f}% on the modal offset)")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--switch"]
    if not args:
        sys.exit(__doc__)
    if "--switch" in sys.argv:
        for root in args:
            switch_report(root)
        sys.exit(0)
    for root in args:
        recs = harvest(root, menu_sample=MENU_SAMPLE)
        # the flat band is an instrument, not a fact about the schedule: rerun at other widths
        bands = [(f"+-{f:g} deg", harvest(root, flat=f)) for f in (2.0, 5.0, 10.0)]
        report(os.path.basename(root.rstrip("/")), recs, bands)
