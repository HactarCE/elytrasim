#!/usr/bin/env python3
"""Generate the seed families for one atlas cell.

A seed is an initial pitch schedule. It is the only thing that decides which local optimum
`sweep polish` converges to -- nothing in the polish is stochastic, and `--seed` drives only the
velocity jitter, which the priced pipeline turns off. So the way to find qualitatively different
optima is to *state* qualitatively different strategies as seeds and polish each one under the
unchanged objective. Diversity lives in the seed, never in the utility function: a profile's
claim is that its pitches are a coordinate optimum of the objective in its own header, and a
novelty term would make every file an optimum of something that depends on which other files
existed when it ran.

Each family is a named strategy with parameters, so a converged optimum can be traced back to
the entrance it was reached through.

  python3 tools/atlas_seeds.py --n 300 --out runs/atlas/seeds/n300 --ref runs/veljit/ref300.pitches

Writes <out>/<name>.pitches plus <out>/seeds.json mapping name -> {family, params}.
"""

import argparse
import json
import os
import random

LIMIT = 85.0  # the admissible set the atlas polishes in; seeds are clamped to it


def clamp(p):
    return max(-LIMIT, min(LIMIT, p))


def read_pitches(path):
    txt = open(path).read()
    return [float(x) for line in txt.splitlines() for x in line.split('#')[0].split()]


def resample(p, n):
    """Linear resample to length n: a cycle of period len(p) becomes a cycle of period n."""
    if not p:
        return [0.0] * n
    if len(p) == 1:
        return [p[0]] * n
    m = len(p)
    out = []
    for i in range(n):
        x = i * (m - 1) / max(n - 1, 1)
        lo = int(x)
        hi = min(lo + 1, m - 1)
        f = x - lo
        out.append(p[lo] * (1 - f) + p[hi] * f)
    return out


def tile(p, n):
    """Repeat and cut to length n, preserving absolute phase durations."""
    return [p[i % len(p)] for i in range(n)]


def box(x, k):
    """Local mean over a 2k+1 window, shrinking at the ends."""
    if k <= 0:
        return list(x)
    out = []
    for i in range(len(x)):
        lo, hi = max(0, i - k), min(len(x), i + k + 1)
        out.append(sum(x[lo:hi]) / (hi - lo))
    return out


# ---------------------------------------------------------------- families

def fam_const(n):
    """Constant pitch. The entrance to the steady-glide turnpike; every value tested lands in
    the same basin, so this family is mostly a control -- it is here to show that it is one."""
    for p in range(-85, 86, 5):
        yield f"const_{p:+03d}", {"family": "const", "pitch": p}, [float(p)] * n


def fam_diveflick(n):
    """dive -> snap -> flick -> gain, parametrized by when the flick happens.

    This is the family that actually separates optima: the flick tick is a genuine degree of
    freedom and the polish does not slide it, so each flick time converges to its own local
    optimum. Measured at n=300: t=120 -> dJ 14.1, t=160 -> 18.4, t=200 -> 21.9, t=240 -> 21.1.
    """
    ramp = 6
    snap = 10
    for frac in [i / 20 for i in range(3, 19)]:      # 0.15n .. 0.90n
        tf = int(round(frac * n))
        for dive in (30.0, 50.0):
            for gain in (-20.0, -35.0):
                s = []
                for t in range(n):
                    if t < tf - snap:
                        s.append(dive)
                    elif t < tf:
                        s.append(0.0)
                    elif t < tf + ramp:
                        s.append(-LIMIT * (t - tf + 1) / ramp)
                    else:
                        s.append(gain)
                name = f"df_t{tf:04d}_d{int(dive):02d}_g{int(-gain):02d}"
                yield name, {"family": "diveflick", "flick_tick": tf, "dive": dive,
                             "gain": gain}, [clamp(v) for v in s]


def fam_kcycle(n, ref):
    """k copies of the reference cycle compressed into the horizon, k = 1..5.

    The corpus calls more than one cycle degenerate and excludes it. That is a statement about
    which question the corpus asks, not about whether these are optima -- so they go in the
    atlas, labeled.
    """
    if not ref:
        return
    for k in range(1, 6):
        period = max(2, n // k)
        one = resample(ref, period)
        s = tile(one, n)
        yield f"kcycle_{k}", {"family": "kcycle", "k": k, "period": period}, [clamp(v) for v in s]


def fam_refvariant(n, ref):
    """The reference cycle rescaled to the horizon, tiled at its true period, and phase-shifted.

    Phase shift matters because the entry is the one phase with no known rule: starting the
    schedule partway through the cycle asks whether the entry is what picks the basin.
    """
    if not ref:
        return
    yield "ref_rescale", {"family": "refvariant", "kind": "rescale"}, [clamp(v) for v in resample(ref, n)]
    yield "ref_tile", {"family": "refvariant", "kind": "tile"}, [clamp(v) for v in tile(ref, n)]
    for num, den in [(1, 8), (1, 4), (3, 8), (1, 2), (5, 8), (3, 4)]:
        off = int(round(len(ref) * num / den))
        s = tile(ref[off:] + ref[:off], n)
        yield (f"ref_shift{num}_{den}", {"family": "refvariant", "kind": "shift",
                                         "offset": off}, [clamp(v) for v in s])


def fam_lerp(n):
    """A straight ramp from start to end. `PitchesUtil::new_lerp` in the cycle-optimizer.

    The simplest schedule that is not constant, and the one with no corner anywhere: if a ramp
    converges somewhere the dive-flick family never reaches, that is a basin whose entrance is
    smoothness rather than structure.
    """
    grid = (-80, -40, 0, 40, 80)
    for a in grid:
        for b in grid:
            if a == b:
                continue                     # that is the const family
            s = [a + (b - a) * i / max(n - 1, 1) for i in range(n)]
            yield f"lerp_{a:+03d}_{b:+03d}", {"family": "lerp", "start": a, "end": b}, s


def fam_bang(n):
    """+L held, then -L held: `PitchesUtil::new_4040`, generalized off 40 degrees.

    Two levels and one switch. This is the crudest possible statement of "dive, then climb", and
    it is worth having precisely because it says nothing about the snap, the flick shape or the
    gain -- so whatever it converges to was found by the optimizer rather than supplied by us.
    """
    for level in (20.0, 40.0, 60.0, 85.0):
        for cut in (0.2, 0.35, 0.5, 0.65, 0.8):
            mid = int(n * cut)
            s = [level] * mid + [-level] * (n - mid)
            yield (f"bang_L{int(level):02d}_c{int(cut * 100):02d}",
                   {"family": "bang", "level": level, "cut": cut}, s)


def fam_bangzero(n):
    """+L, then level, then -L: `PitchesUtil::new_40zero40`. The snap, stated crudely."""
    for level in (40.0, 85.0):
        for lo, hi in ((0.5, 0.55), (0.65, 0.70), (0.8, 0.85), (0.65, 0.80)):
            a, b = int(n * lo), int(n * hi)
            s = [level] * a + [0.0] * (b - a) + [-level] * (n - b)
            yield (f"bangzero_L{int(level):02d}_{int(lo * 100):02d}_{int(hi * 100):02d}",
                   {"family": "bangzero", "level": level, "left": lo, "right": hi}, s)


def fam_fourlines(n):
    """The cycle-optimizer's own default seed: four straight lines fitted by eye to the optimal
    curve (`default_pitches` in cycle-optimizer/src/main.rs). Ramp 10 -> 50, hold 0, ramp
    -85 -> -30, ramp -30 -> -10, with the cuts as parameters.

    It is the shape a person drew after looking at the answer, which makes it a different kind of
    entrance from either a policy rollout or a parametrized manoeuvre.
    """
    for lo, mid, hi in ((0.5, 0.55, 0.70), (0.65, 0.70, 0.80), (0.8, 0.85, 0.90)):
        a, b, c = int(n * lo), int(n * mid), int(n * hi)
        def ramp(k, x, y):
            return [x + (y - x) * i / max(k - 1, 1) for i in range(k)] if k > 0 else []
        s = ramp(a, 10.0, 50.0) + [0.0] * (b - a) + ramp(c - b, -85.0, -30.0) \
            + ramp(n - c, -30.0, -10.0)
        yield (f"fourlines_{int(lo * 100):02d}_{int(mid * 100):02d}_{int(hi * 100):02d}",
               {"family": "fourlines", "cuts": [lo, mid, hi]}, [clamp(v) for v in s])


def fam_randwalk(n, rng, draws=4):
    """`PitchesUtil::new_rand_walk`. Unlike white noise it has persistent drift, so it can wander
    into a sustained dive or climb by accident -- which is the only way an unstructured seed has
    ever reached anything but the steady glide."""
    for step in (1.0, 3.0, 10.0):
        for d in range(draws):
            p = rng.uniform(-LIMIT, LIMIT)
            s = []
            for _ in range(n):
                p = clamp(p + rng.uniform(-step, step))
                s.append(p)
            yield f"walk_s{int(step):02d}_{d}", {"family": "randwalk", "step": step, "draw": d}, s


def fam_policy(n):
    """Placeholder: the four tuned dive rules are flown by the Rust side (`seed_from_policy`),
    which needs the physics. `atlas_run.sh` adds them directly from the binary."""
    return
    yield


def fam_pwc(n, rng, draws=12):
    """Piecewise-constant with random breakpoints and levels.

    This is the honest randomized control. White noise in R^n is not a control a hand could
    produce and it collapses into the glide every time (measured: 8 of 8); a piecewise-constant
    schedule lives in the same sparse-curvature space the real answers do, so if there is a
    basin the named families missed, this is the family with a chance of falling into it.
    """
    for m in (2, 3, 4, 6, 8, 12):
        for d in range(draws):
            cuts = sorted(rng.sample(range(1, n), m - 1)) if m > 1 else []
            bounds = [0] + cuts + [n]
            levels = [rng.uniform(-LIMIT, LIMIT) for _ in range(m)]
            s = []
            for i in range(m):
                s += [levels[i]] * (bounds[i + 1] - bounds[i])
            yield (f"pwc_m{m:02d}_{d:02d}", {"family": "pwc", "segments": m, "draw": d,
                                             "levels": [round(v, 3) for v in levels],
                                             "cuts": cuts}, s)


def fam_noise(n, rng, draws=4):
    """White and low-pass noise. Kept as a documented control: every one of these collapses to
    the steady glide, which is the measurement that says random multi-start does not work here.
    """
    for k in (0, 5, 20, 40):
        for d in range(draws):
            raw = [rng.uniform(-LIMIT, LIMIT) for _ in range(n)]
            s = box(raw, k)
            yield f"noise_k{k:02d}_{d}", {"family": "noise", "smooth": k, "draw": d}, s


# ---------------------------------------------------------------- main

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--ref", default=None, help="reference cycle .pitches, for the cyclic families")
    ap.add_argument("--rng-seed", type=int, default=20260910)
    ap.add_argument("--pwc-draws", type=int, default=12)
    ap.add_argument("--flick-step", type=int, default=0,
                    help="emit ONLY a fine dive-flick sweep, stepping the flick tick by this "
                         "many ticks, at one (dive, gain). Answers whether the flick-time "
                         "optima are separate basins or one connected ridge: if the converged "
                         "flick tick tracks the seed's flick tick continuously it is a ridge, "
                         "and if it snaps to a few values they are separate basins.")
    ap.add_argument("--flick-lo", type=int, default=None,
                    help="first flick tick (default int(0.12n)). The default window is a guess "
                         "about where the manoeuvre belongs; pass 0 to scan the whole horizon "
                         "and let the result say so instead.")
    ap.add_argument("--flick-hi", type=int, default=None,
                    help="last flick tick, inclusive (default int(0.96n))")
    args = ap.parse_args()

    ref = read_pitches(args.ref) if args.ref else None
    rng = random.Random(args.rng_seed + args.n)
    os.makedirs(args.out, exist_ok=True)

    manifest = {}
    if args.flick_step > 0:
        ramp, snap, dive, gain = 6, 10, 30.0, -20.0
        lo = int(0.12 * args.n) if args.flick_lo is None else args.flick_lo
        hi = int(0.96 * args.n) if args.flick_hi is None else args.flick_hi
        lo, hi = max(0, lo), min(args.n, hi)
        assert lo <= hi, f"empty flick range {lo}..{hi}"
        for tf in range(lo, hi + 1, args.flick_step):
            s = []
            for t in range(args.n):
                if t < tf - snap:
                    s.append(dive)
                elif t < tf:
                    s.append(0.0)
                elif t < tf + ramp:
                    s.append(-LIMIT * (t - tf + 1) / ramp)
                else:
                    s.append(gain)
            name = f"tight_t{tf:04d}"
            with open(os.path.join(args.out, name + ".pitches"), "w") as f:
                f.write(" ".join(f"{clamp(v):.5f}" for v in s))
            manifest[name] = {"family": "flicksweep", "flick_tick": tf,
                              "dive": dive, "gain": gain}
        assert manifest, "flick sweep produced nothing"
        with open(os.path.join(args.out, "seeds.json"), "w") as f:
            json.dump({"n": args.n, "limit": LIMIT, "flick_step": args.flick_step,
                       "flick_lo": lo, "flick_hi": hi, "seeds": manifest}, f, indent=1)
        print(f"{len(manifest)} flick-sweep seeds -> {args.out} "
              f"(ticks {lo}..{hi} step {args.flick_step})")
        return

    families = [
        fam_const(args.n),
        fam_diveflick(args.n),
        fam_kcycle(args.n, ref),
        fam_refvariant(args.n, ref),
        fam_lerp(args.n),
        fam_bang(args.n),
        fam_bangzero(args.n),
        fam_fourlines(args.n),
        fam_randwalk(args.n, rng),
        fam_pwc(args.n, rng, args.pwc_draws),
        fam_noise(args.n, rng),
    ]
    count = 0
    for fam in families:
        for name, meta, sched in fam:
            assert len(sched) == args.n, f"{name}: {len(sched)} pitches, wanted {args.n}"
            assert all(abs(v) <= LIMIT + 1e-9 for v in sched), f"{name}: outside the limit"
            with open(os.path.join(args.out, name + ".pitches"), "w") as f:
                f.write(" ".join(f"{v:.5f}" for v in sched))
            manifest[name] = meta
            count += 1
    # An empty family is the failure mode this codebase keeps finding: it reads as success.
    seen = {m["family"] for m in manifest.values()}
    want = {"const", "diveflick", "pwc", "noise", "lerp", "bang", "bangzero", "fourlines",
            "randwalk"} | ({"kcycle", "refvariant"} if ref else set())
    missing = want - seen
    assert not missing, f"families produced nothing: {sorted(missing)}"
    with open(os.path.join(args.out, "seeds.json"), "w") as f:
        json.dump({"n": args.n, "limit": LIMIT, "rng_seed": args.rng_seed,
                   "ref": args.ref, "seeds": manifest}, f, indent=1)
    by_fam = {}
    for m in manifest.values():
        by_fam[m["family"]] = by_fam.get(m["family"], 0) + 1
    print(f"{count} seeds -> {args.out}")
    for k in sorted(by_fam):
        print(f"  {k:12s} {by_fam[k]:4d}")


if __name__ == "__main__":
    main()
