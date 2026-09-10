#!/usr/bin/env python3
"""Generate structured basin-hopping seeds from converged atlas profiles.

The flick detector scores five-tick nose-up excursions, then reports the steepest single drop
inside the strongest window. A candidate must drop substantially, must do so on most steps in
the window, and must finish near the schedule's minimum. Those checks are intentionally
redundant: a steady glide has no flick, and treating its least-flat tick as one would turn an
empty result into apparent success.

  python3 tools/atlas_perturb.py runs/atlas/cell --out runs/atlas/perturbed
  python3 tools/atlas_perturb.py ignored --out /tmp/seeds --reps a.pitches b.pitches

Writes <out>/<name>.pitches plus <out>/seeds.json in the format used by atlas_seeds.py.
"""

import argparse
import json
import os
import re
from pathlib import Path

import numpy as np


LIMIT = 85.0


def read_pitches(path):
    try:
        text = Path(path).read_text()
        pitches = [float(x) for line in text.splitlines()
                   for x in line.split("#")[0].split()]
    except (OSError, ValueError) as exc:
        raise ValueError(f"{path}: could not read pitches: {exc}") from exc
    if not pitches:
        raise ValueError(f"{path}: no pitches")
    out = np.asarray(pitches, dtype=float)
    if not np.all(np.isfinite(out)):
        raise ValueError(f"{path}: pitches contain a non-finite value")
    return out


def source_name(path):
    name = re.sub(r"[^A-Za-z0-9]+", "_", Path(path).stem).strip("_")
    if not name:
        raise ValueError(f"{path}: filename has no usable source name")
    return name


def clamp(pitches):
    return np.clip(np.asarray(pitches, dtype=float), -LIMIT, LIMIT)


def resample(pitches, n):
    if n < 1:
        raise ValueError(f"cannot resample to length {n}")
    if len(pitches) == 1:
        return np.full(n, pitches[0])
    old = np.linspace(0.0, 1.0, len(pitches))
    new = np.linspace(0.0, 1.0, n)
    return np.interp(new, old, pitches)


def detect_flick(pitches, window=5):
    """Return the sharpest tick in a sustained drop that arrives near the global minimum."""
    n = len(pitches)
    if n <= window:
        raise ValueError(f"only {n} ticks; need more than the {window}-tick detector window")

    steps = np.diff(pitches)
    drops = pitches[:-window] - pitches[window:]
    order = np.argsort(drops)[::-1]
    span = float(np.ptp(pitches))
    need_drop = max(15.0, 0.25 * span)
    near_min = max(5.0, 0.10 * span)

    for start in order:
        stop = int(start) + window
        local = steps[start:stop]
        drop = float(drops[start])
        falling = int(np.count_nonzero(local < -0.25))
        if drop < need_drop:
            break
        if falling < 3:
            continue
        if pitches[stop] > float(np.min(pitches)) + near_min:
            continue
        return int(start) + int(np.argmin(local)), {
            "window_start": int(start),
            "window_stop": stop,
            "window_drop": drop,
        }

    best = float(drops[order[0]])
    raise ValueError(
        f"no sustained flick: best {window}-tick drop is {best:.2f} deg; "
        f"required {need_drop:.2f} deg, mostly falling, and ending near the minimum"
    )


def shift_flick(pitches, tick, delta):
    """Move one time landmark without inventing or deleting either surrounding phase."""
    n = len(pitches)
    moved = tick + delta
    if not 1 <= moved < n - 1:
        raise ValueError(f"shift {delta:+d} moves flick tick {tick} outside a {n}-tick schedule")

    target = np.arange(n, dtype=float)
    source = np.empty(n, dtype=float)
    left = target <= moved
    source[left] = target[left] * tick / moved
    source[~left] = tick + (target[~left] - moved) * (n - 1 - tick) / (n - 1 - moved)
    return np.interp(source, np.arange(n, dtype=float), pitches)


def splice(a, b, cut, blend_ticks=5):
    out = np.concatenate((a[:cut], b[cut:])).copy()
    width = min(blend_ticks, cut, len(a) - cut)
    start = cut - width // 2
    stop = start + width
    left = a[start - 1] if start else a[0]
    right = b[stop] if stop < len(b) else b[-1]
    # A hard join supplies a fake optimizer target; bridge neighboring real samples instead.
    out[start:stop] = np.linspace(left, right, width + 2)[1:-1]
    return out, width


def local_kick(n, amplitude, rng):
    modes = int(rng.integers(3, 7))
    frequencies = np.sort(rng.choice(np.arange(1, 9), size=modes, replace=False))
    phases = rng.uniform(0.0, 2.0 * np.pi, size=modes)
    weights = rng.normal(size=modes)
    x = np.arange(n, dtype=float) / max(n - 1, 1)
    kick = sum(w * np.cos(2.0 * np.pi * f * x + phase)
               for f, phase, w in zip(frequencies, phases, weights))
    peak = float(np.max(np.abs(kick)))
    if peak == 0.0:
        raise RuntimeError("random cosine kick was identically zero")
    kick *= amplitude / peak
    meta = {
        "modes": modes,
        "frequencies": [int(v) for v in frequencies],
        "phases": [round(float(v), 8) for v in phases],
        "weights": [round(float(v), 8) for v in weights],
    }
    return kick, meta


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("celldir", help="directory of converged .pitches profiles")
    ap.add_argument("--out", required=True, help="output seed directory")
    ap.add_argument("--reps", nargs="+", help="explicit representatives instead of celldir")
    ap.add_argument("--rng-seed", type=int, default=20260910)
    args = ap.parse_args()

    paths = [Path(p) for p in args.reps] if args.reps else sorted(
        Path(args.celldir).glob("*.pitches")
    )
    if not paths:
        raise SystemExit(f"no representatives found in {args.celldir}")

    profiles = [(source_name(path), path, read_pitches(path)) for path in paths]
    names = [name for name, _, _ in profiles]
    if len(set(names)) != len(names):
        raise SystemExit(f"representative names collide after sanitizing: {names}")
    lengths = {len(pitches) for _, _, pitches in profiles}
    if len(lengths) != 1:
        detail = ", ".join(f"{path}={len(pitches)}" for _, path, pitches in profiles)
        raise SystemExit(f"representatives must have one schedule length for splicing: {detail}")
    n = lengths.pop()
    if n < 8:
        raise SystemExit(f"schedules have only {n} ticks; need at least 8")

    os.makedirs(args.out, exist_ok=True)
    rng = np.random.default_rng(args.rng_seed)
    manifest = {}
    by_family = {}

    def emit(name, family, meta, pitches):
        pitches = clamp(pitches)
        if len(pitches) != n:
            raise AssertionError(f"{name}: {len(pitches)} pitches, wanted {n}")
        if not np.all(np.isfinite(pitches)):
            raise AssertionError(f"{name}: non-finite pitch")
        if name in manifest:
            raise AssertionError(f"duplicate seed name: {name}")
        with open(Path(args.out) / f"{name}.pitches", "w") as handle:
            handle.write(" ".join(f"{value:.5f}" for value in pitches))
        manifest[name] = {"family": family, **meta}
        by_family[family] = by_family.get(family, 0) + 1

    for source, path, pitches in profiles:
        try:
            tick, detection = detect_flick(pitches)
        except ValueError as exc:
            print(f"flick: skipping {source} ({path}): {exc}")
        else:
            print(
                f"flick: {source} tick {tick} "
                f"({detection['window_drop']:.2f} deg over ticks "
                f"{detection['window_start']}..{detection['window_stop']})"
            )
            for amount in (5, 10, 20, 40):
                for delta in (-amount, amount):
                    try:
                        shifted = shift_flick(pitches, tick, delta)
                    except ValueError as exc:
                        print(f"flick: skipping {source} shift {delta:+d}: {exc}")
                        continue
                    name = f"{source}__flick_{delta:+d}"
                    emit(name, "flick_shift", {
                        "source": str(path), "source_flick_tick": tick,
                        "shift": delta, "target_flick_tick": tick + delta,
                        "detector": detection,
                    }, shifted)

    for source_a, path_a, a in profiles:
        for source_b, path_b, b in profiles:
            if source_a == source_b:
                continue
            for numerator in (1, 2, 3):
                cut = int(round(n * numerator / 4))
                mixed, width = splice(a, b, cut)
                name = f"{source_a}__{source_b}__splice_q{numerator}"
                emit(name, "splice", {
                    "source_a": str(path_a), "source_b": str(path_b),
                    "cut": cut, "fraction": numerator / 4, "blend_ticks": width,
                }, mixed)

    for source, path, pitches in profiles:
        for copies in (2, 3):
            period = max(2, n // copies)
            one = resample(pitches, period)
            cycled = np.resize(one, n)
            emit(f"{source}__cycle_x{copies}", "cycle_count", {
                "source": str(path), "kind": "tile", "copies": copies,
                "period": period,
            }, cycled)
        half = pitches[:max(2, n // 2)]
        emit(f"{source}__cycle_half_stretch", "cycle_count", {
            "source": str(path), "kind": "first_half_stretch",
            "source_ticks": len(half),
        }, resample(half, n))

        mean = float(np.mean(pitches))
        for scale in (0.5, 0.75, 1.25, 1.5):
            scaled = mean + scale * (pitches - mean)
            label = str(scale).replace(".", "p")
            emit(f"{source}__amplitude_{label}", "amplitude", {
                "source": str(path), "scale": scale, "mean": mean,
            }, scaled)

        emit(f"{source}__reverse", "time_reversal", {
            "source": str(path),
        }, pitches[::-1])

        for amplitude in (5.0, 10.0, 20.0):
            for draw in range(4):
                kick, kick_meta = local_kick(n, amplitude, rng)
                emit(f"{source}__kick_a{int(amplitude):02d}_d{draw}", "local_kick", {
                    "source": str(path), "amplitude": amplitude, "draw": draw,
                    **kick_meta,
                }, pitches + kick)

    wanted = {
        "flick_shift", "splice", "cycle_count", "amplitude", "time_reversal", "local_kick",
    }
    missing = wanted - set(by_family)
    if missing:
        raise AssertionError(f"families produced nothing: {sorted(missing)}")

    with open(Path(args.out) / "seeds.json", "w") as handle:
        json.dump({
            "n": n, "limit": LIMIT, "rng_seed": args.rng_seed,
            "representatives": [str(path) for _, path, _ in profiles],
            "seeds": manifest,
        }, handle, indent=1)
    print(f"{len(manifest)} seeds -> {args.out}")
    for family in sorted(by_family):
        print(f"  {family:14s} {by_family[family]:4d}")


if __name__ == "__main__":
    main()
