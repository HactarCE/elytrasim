#!/usr/bin/env python3
"""How much is each feature of an optimal schedule worth?

The corpus reports one optimum per cell, which says what to fly and nothing about which parts of
it matter. This asks the complementary question: take a converged schedule, replace one feature
with something else, and measure what it costs.

Two numbers per substitution, and the pair is the point:

  hold   J of the substituted schedule with nothing else allowed to move. The feature's raw
         contribution, i.e. what you lose by flying this part wrong on an otherwise perfect run.
  adapt  J after re-polishing the rest for a short budget. What you lose when the rest of the
         schedule is allowed to compensate. The gap between the two is how replaceable the
         feature is -- a large gap means the schedule can route around it, a small one means the
         feature is load-bearing on its own.

Neither number needs the polish to converge, which is why the budget is deliberately small. We
are measuring a difference between two schedules optimized the same way, not certifying either.

  python3 tools/atlas_ablate.py <profile> --out <seeddir>          # write the substitutions
  then tools/atlas_ablate.sh <seeddir> <profile>                   # evaluate hold and adapt

Segments are detected from the schedule rather than assumed, so this works on any member of the
cyclic family, and refuses (loudly, per feature) when a feature is not present -- a steady glide
has no flick, and a detector that quietly returned tick 0 would be the emptiness-reads-as-success
bug this codebase keeps finding.
"""

import argparse
import json
import os

LIMIT = 85.0
TURNPIKE = -13.052   # min-sink steady glide, from `myopic crit`


def clamp(p):
    return max(-LIMIT, min(LIMIT, p))


def read_pitches(path):
    txt = open(path).read()
    return [float(x) for line in txt.splitlines() for x in line.split('#')[0].split()]


def ramp(k, a, b):
    return [a + (b - a) * i / max(k - 1, 1) for i in range(k)] if k > 0 else []


def lsq_line(y):
    """The least-squares straight line through a segment.

    Fitting beats interpolating the endpoints whenever the segment is not monotone, and the dive
    is not: an endpoint ramp on a rising-then-falling segment points the wrong way and costs 30
    blocks for reasons that have nothing to do with the feature under test.
    """
    k = len(y)
    if k < 2:
        return list(y)
    xm = (k - 1) / 2.0
    ym = sum(y) / k
    den = sum((i - xm) ** 2 for i in range(k))
    m = sum((i - xm) * (y[i] - ym) for i in range(k)) / den if den else 0.0
    return [ym + m * (i - xm) for i in range(k)]


def segments(p, entry_len=40, tail_len=10):
    """Split a cyclic schedule into entry / dive / snap / flick / gain / ending.

    The flick is the most nose-up tick. The snap is the flat run just before it: walking back
    from the flick, the ticks whose pitch is within a few degrees of level. The dive is whatever
    lies between the entry and the snap. Returns None for a schedule with no flick at all.
    """
    n = len(p)
    tf = min(range(n), key=lambda i: p[i])
    if p[tf] > -30.0:
        return None                      # never flicks: a glide, not a member of this family
    # walk back from the flick to the last tick that was still committed to the dive
    t = tf
    while t > 0 and p[t] < 5.0:
        t -= 1
    snap_start, snap_end = t + 1, tf
    # The pitch does not step from the dive to the snap: it falls through it, 45 degrees to 15
    # in a handful of ticks. Those transition ticks belong to the snap, not the dive -- left in
    # the dive they make its endpoints unrepresentative of its shape, which is how a substitution
    # meant to test curvature ends up testing direction instead.
    while snap_start > 1 and p[snap_start - 1] - p[snap_start] > 2.0:
        snap_start -= 1
    # the flick itself is the descent from the snap to the most nose-up tick, plus a few ticks
    # of the recovery; the gain is everything after until the ending.
    flick_end = min(tf + 6, n)
    seg = {
        "entry": (0, min(entry_len, snap_start)),
        "dive": (min(entry_len, snap_start), snap_start),
        "snap": (snap_start, snap_end),
        "flick": (snap_end, flick_end),
        "gain": (flick_end, max(flick_end, n - tail_len)),
        "ending": (max(flick_end, n - tail_len), n),
    }
    return tf, seg


def variants(p, seg, tf):
    """The substitutions, one per (feature, alternative). Each yields a whole schedule."""
    n = len(p)

    def sub(a, b, vals):
        assert len(vals) == b - a, f"segment {a}:{b} wanted {b - a} values, got {len(vals)}"
        return [clamp(v) for v in (p[:a] + list(vals) + p[b:])]

    # ---- entry: the phase README-myopic calls the open problem, "pitches hard down, unexplained"
    a, b = seg["entry"]
    if b > a:
        yield "entry_const_end", "entry", sub(a, b, [p[b]] * (b - a))
        yield "entry_turnpike", "entry", sub(a, b, [TURNPIKE] * (b - a))
        yield "entry_level", "entry", sub(a, b, [0.0] * (b - a))
        yield "entry_ramp", "entry", sub(a, b, ramp(b - a, 0.0, p[b]))
        yield "entry_harddive", "entry", sub(a, b, [50.0] * (b - a))

    # ---- dive: is the shape of the descent doing work, or only its average?
    a, b = seg["dive"]
    if b > a + 2:
        mean = sum(p[a:b]) / (b - a)
        yield "dive_const_mean", "dive", sub(a, b, [mean] * (b - a))
        # Least squares, not an endpoint interpolation. The dive is not monotone -- it rises
        # from about 24 to 45 degrees and then falls into the snap -- so a ramp between the
        # first and last tick of the segment runs the wrong way entirely, and what it measures
        # is the size of the perturbation rather than the importance of the dive's shape.
        yield "dive_linear", "dive", sub(a, b, lsq_line(p[a:b]))
        for off in (-8.0, -4.0, +4.0, +8.0):
            yield f"dive_offset{off:+.0f}", "dive", sub(a, b, [x + off for x in p[a:b]])

    # ---- snap: the flat that separates the dive from the flick
    a, b = seg["snap"]
    if b > a:
        yield "snap_removed", "snap", sub(a, b, [p[a - 1] if a > 0 else 0.0] * (b - a))
        yield "snap_level", "snap", sub(a, b, [0.0] * (b - a))
        # lengthen it by eating into the dive, shorten it by eating into itself
        k = b - a
        if a - k >= 0:
            yield "snap_double", "snap", sub(a - k, b, [0.0] * (2 * k))
        if k > 2:
            yield "snap_half", "snap", sub(a, b, [p[a - 1]] * (k - k // 2) + [0.0] * (k // 2))

    # ---- flick: how deep, and how fast you get there
    a, b = seg["flick"]
    if b > a:
        for depth in (-45.0, -60.0, -70.0):
            yield f"flick_depth{depth:+.0f}", "flick", \
                sub(a, b, [max(depth, x) for x in p[a:b]])
        yield "flick_instant", "flick", sub(a, b, [p[tf]] * (b - a))
        slow = ramp(b - a, p[a], p[tf])
        yield "flick_slow", "flick", sub(a, b, slow)

    # ---- gain: the decay back toward the turnpike
    a, b = seg["gain"]
    if b > a + 2:
        mean = sum(p[a:b]) / (b - a)
        yield "gain_const_mean", "gain", sub(a, b, [mean] * (b - a))
        yield "gain_turnpike", "gain", sub(a, b, [TURNPIKE] * (b - a))
        yield "gain_linear", "gain", sub(a, b, lsq_line(p[a:b]))

    # ---- ending: the terminal nose-down break, which exists only because v_n is free
    a, b = seg["ending"]
    if b > a:
        yield "ending_hold", "ending", sub(a, b, [p[a - 1] if a > 0 else 0.0] * (b - a))
        yield "ending_turnpike", "ending", sub(a, b, [TURNPIKE] * (b - a))
        yield "ending_level", "ending", sub(a, b, [0.0] * (b - a))
        yield "ending_dive", "ending", sub(a, b, [LIMIT] * (b - a))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("profile")
    ap.add_argument("--out", required=True)
    ap.add_argument("--entry-len", type=int, default=40)
    ap.add_argument("--tail-len", type=int, default=10)
    args = ap.parse_args()

    p = read_pitches(args.profile)
    got = segments(p, args.entry_len, args.tail_len)
    if got is None:
        raise SystemExit(f"{args.profile}: no flick (most nose-up pitch is {min(p):.1f} deg), so "
                         f"this schedule has no cyclic features to ablate")
    tf, seg = got
    os.makedirs(args.out, exist_ok=True)
    print(f"{args.profile}: n {len(p)}, flick at tick {tf}")
    for k, (a, b) in seg.items():
        print(f"  {k:<7} ticks {a:>4}..{b:<4} ({b - a:>3} ticks)")

    manifest, by_feature = {}, {}
    for name, feature, sched in variants(p, seg, tf):
        assert len(sched) == len(p), f"{name}: length changed"
        with open(os.path.join(args.out, name + ".pitches"), "w") as f:
            f.write(" ".join(f"{v:.5f}" for v in sched))
        a, b = seg[feature]
        # Raw dJ conflates "this feature matters" with "I perturbed it a lot", so
        # record how big each substitution actually is and let the analysis divide.
        dsum = sum(abs(x - y) for x, y in zip(sched, p))
        manifest[name] = {"feature": feature, "segment": [a, b],
                          "pert_sum_deg": round(dsum, 4),
                          "pert_mean_deg": round(dsum / max(b - a, 1), 4)}
        by_feature[feature] = by_feature.get(feature, 0) + 1
    # A feature that produced no substitution is a detector that quietly failed.
    missing = [k for k, (a, b) in seg.items() if b > a and k not in by_feature]
    assert not missing, f"segments present but no substitutions generated: {missing}"
    with open(os.path.join(args.out, "ablations.json"), "w") as f:
        json.dump({"profile": os.path.abspath(args.profile), "n": len(p), "flick_tick": tf,
                   "segments": {k: list(v) for k, v in seg.items()},
                   "ablations": manifest}, f, indent=1)
    print(f"\n{len(manifest)} substitutions -> {args.out}")
    for k in sorted(by_feature):
        print(f"  {k:<8} {by_feature[k]:>3}")


if __name__ == "__main__":
    main()
