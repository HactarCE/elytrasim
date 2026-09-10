#!/usr/bin/env python3
"""Describe the local optima reached by one atlas cell.

    python3 tools/atlas.py runs/atlas/cells/v00_n300_lam0 \
        --seeds runs/atlas/seeds/n300/seeds.json --fig atlas.png

Distances are deliberately inferred from the empty part of the empirical distribution. An
atlas with no clean gap is evidence we do not know how to cluster it, not an invitation to pick
a convenient cutoff.
"""

import argparse
import json
import math
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

import numpy as np


REQUIRED = ("dJ", "curv_l1", "curv_max", "lag1", "variation", "structure",
            "cycles", "dy", "dz", "certified")
GAP_LO = 0.1
GAP_HI = 30.0
GAP_RATIO = 3.0
SHUFFLES = 200


class AtlasError(Exception):
    pass


@dataclass
class Profile:
    path: Path
    pitches: np.ndarray
    header: dict
    family: str = "unknown"
    params: dict = None

    def number(self, key):
        try:
            value = float(self.header[key].split()[0])
        except (KeyError, ValueError, IndexError) as exc:
            raise AtlasError(f"{self.path}: bad numeric header field {key!r}") from exc
        if not math.isfinite(value):
            raise AtlasError(f"{self.path}: non-finite numeric header field {key!r}")
        return value


def header_fields(text):
    fields = {}
    for line in text.splitlines():
        if not line.startswith("# "):
            continue
        body = line[2:]
        key, sep, value = body.partition(" ")
        if sep:
            fields[key] = value.split("#", 1)[0].strip()
    return fields


def read_profile(path, require_certified=True):
    try:
        text = path.read_text()
    except OSError as exc:
        raise AtlasError(f"{path}: {exc}") from exc
    header = header_fields(text)
    if require_certified and "certified" not in header:
        return None
    missing = [key for key in REQUIRED if key not in header]
    if missing:
        raise AtlasError(f"{path}: missing header field(s): {', '.join(missing)}")
    try:
        values = [float(word) for line in text.splitlines()
                  for word in line.split("#", 1)[0].split()]
    except ValueError as exc:
        raise AtlasError(f"{path}: bad pitch: {exc}") from exc
    pitches = np.asarray(values, dtype=float)
    if not len(pitches):
        raise AtlasError(f"{path}: certified profile contains no pitches")
    if not np.all(np.isfinite(pitches)):
        raise AtlasError(f"{path}: pitches contain a non-finite value")
    if "n" in header:
        try:
            claimed = int(header["n"].split()[0])
        except (ValueError, IndexError) as exc:
            raise AtlasError(f"{path}: bad numeric header field 'n'") from exc
        if claimed != len(pitches):
            raise AtlasError(
                f"{path}: header says n = {claimed}, but file holds {len(pitches)} pitches")
    profile = Profile(path, pitches, header)
    # Parse all reported statistics now, so a malformed result cannot survive until one table.
    for key in ("dJ", "curv_l1", "curv_max", "lag1", "variation", "cycles", "dy", "dz"):
        profile.number(key)
    return profile


def load_profiles(directory):
    if not directory.is_dir():
        raise AtlasError(f"not a directory: {directory}")
    paths = sorted(directory.glob("*.pitches"))
    if not paths:
        raise AtlasError(f"{directory}: no *.pitches files")
    profiles = []
    skipped = []
    for path in paths:
        profile = read_profile(path)
        if profile is None:
            skipped.append(path)
        else:
            profiles.append(profile)
    print(f"Profiles: {len(profiles)} certified; {len(skipped)} skipped without # certified")
    if skipped:
        sample = ", ".join(path.name for path in skipped[:5])
        suffix = " ..." if len(skipped) > 5 else ""
        print(f"Skipped unfinished files: {sample}{suffix}")
    if not profiles:
        raise AtlasError("no certified profiles remain; refusing to analyze an empty atlas")
    lengths = Counter(len(profile.pitches) for profile in profiles)
    if len(lengths) != 1:
        detail = ", ".join(f"n={n}: {count}" for n, count in sorted(lengths.items()))
        raise AtlasError(
            "profiles have different tick counts; this is not one atlas cell " + f"({detail})")
    return profiles


def load_seed_metadata(path):
    try:
        document = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise AtlasError(f"{path}: cannot read seed metadata: {exc}") from exc
    seeds = document.get("seeds", document)
    if not isinstance(seeds, dict):
        raise AtlasError(f"{path}: seed metadata is not a mapping")
    return seeds


def infer_family(name):
    prefixes = {
        "const_": "const", "df_": "diveflick", "kcycle_": "kcycle",
        "ref_": "refvariant", "pwc_": "pwc", "noise_": "noise",
        "lerp_": "lerp", "bangzero_": "bangzero", "bang_": "bang",
        "fourlines_": "fourlines", "walk_": "randwalk", "tight_": "flicksweep",
        "policy_": "policy",
    }
    return next((family for prefix, family in prefixes.items() if name.startswith(prefix)),
                "unknown")


def attach_seed_metadata(profiles, requested):
    metadata = {}
    source = None
    if requested:
        source = Path(requested)
        metadata = load_seed_metadata(source)
    else:
        n = len(profiles[0].pitches)
        candidate = Path(__file__).resolve().parents[1] / "runs" / "atlas" / "seeds" / f"n{n}"
        candidate = candidate / "seeds.json"
        if candidate.is_file():
            source = candidate
            metadata = load_seed_metadata(source)
    matched = 0
    for profile in profiles:
        meta = metadata.get(profile.path.stem, {})
        if not isinstance(meta, dict):
            raise AtlasError(f"{source}: metadata for {profile.path.stem!r} is not an object")
        profile.family = str(meta.get("family", infer_family(profile.path.stem)))
        profile.params = meta
        matched += bool(meta)
    if source:
        print(f"Seed metadata: {source} ({matched}/{len(profiles)} names matched)")
    else:
        print("Seed metadata: none found; families inferred from filenames")


def pairwise_distances(profiles):
    pitches = np.stack([profile.pitches for profile in profiles])
    matrix = np.zeros((len(profiles), len(profiles)), dtype=float)
    values = []
    # Keeping only one N-by-N matrix avoids the much larger N-by-N-by-ticks temporary.
    for i in range(len(profiles) - 1):
        row = np.mean(np.abs(pitches[i + 1:] - pitches[i]), axis=1)
        matrix[i, i + 1:] = row
        matrix[i + 1:, i] = row
        values.extend(row.tolist())
    return matrix, np.asarray(values, dtype=float)


def print_histogram(distances):
    print("\nPairwise mean |pitch difference| (degrees):")
    zeros = int(np.count_nonzero(distances == 0))
    positive = distances[distances > 0]
    if zeros:
        print(f"  exactly zero      {zeros:7d}")
    if not len(positive):
        print("  no positive distances")
        return
    lo, hi = float(positive.min()), float(positive.max())
    if math.isclose(lo, hi):
        print(f"  {lo:9.4g}        {len(positive):7d}  {'#' * 40}")
        return
    bins = np.geomspace(lo, hi, 17)
    counts, edges = np.histogram(positive, bins=bins)
    scale = max(counts.max(), 1)
    for left, right, count in zip(edges[:-1], edges[1:], counts):
        bar = "#" * int(round(40 * count / scale))
        print(f"  [{left:9.4g}, {right:9.4g}) {count:7d}  {bar}")


def find_threshold(distances):
    if not len(distances):
        raise AtlasError("fewer than two certified profiles; pairwise clustering is undefined")
    ordered = np.sort(distances)
    if len(ordered) < 2:
        raise AtlasError("fewer than two pairwise distances; clustering is undefined")
    candidates = []
    for i, (left, right) in enumerate(zip(ordered[:-1], ordered[1:])):
        # The observed gap may straddle 30 degrees; it is the usable threshold, not both
        # observations bounding it, that must lie in the requested search interval.
        if left > 0 and GAP_LO <= math.sqrt(left * right) <= GAP_HI:
            candidates.append((right - left, i))
    if not candidates:
        raise AtlasError(
            f"no observed pairwise gap has a midpoint between {GAP_LO:g} and {GAP_HI:g} degrees")
    _, widest = max(candidates)
    left, right = float(ordered[widest]), float(ordered[widest + 1])
    ratio = right / left
    if right <= left or ratio < GAP_RATIO:
        raise AtlasError(
            "NO CLUSTERING: the widest observed empty gap with a midpoint in "
            f"[{GAP_LO:g}, {GAP_HI:g}] is {left:.4g}..{right:.4g} degrees "
            f"({ratio:.2f}x), below the required {GAP_RATIO:g}x. The atlas does not "
            "support a data-derived cluster threshold.")
    # A geometric midpoint treats the factor-of-three criterion symmetrically in degrees.
    return math.sqrt(left * right), left, right, ratio


def single_linkage(matrix, threshold, djs):
    parent = list(range(len(matrix)))

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    def union(a, b):
        a, b = find(a), find(b)
        if a != b:
            parent[b] = a

    for i in range(len(matrix) - 1):
        for j in np.flatnonzero(matrix[i, i + 1:] <= threshold) + i + 1:
            union(i, int(j))
    groups = {}
    for i in range(len(matrix)):
        groups.setdefault(find(i), []).append(i)
    clusters = list(groups.values())
    clusters.sort(key=lambda group: max(djs[group]), reverse=True)
    labels = np.empty(len(matrix), dtype=int)
    for label, group in enumerate(clusters, 1):
        labels[group] = label
    return clusters, labels


def counted(values):
    return ",".join(f"{key}:{count}" for key, count in sorted(Counter(values).items()))


def range_text(values, fmt=".1f"):
    lo, hi = min(values), max(values)
    if lo == hi:
        return format(lo, fmt)
    return f"{format(lo, fmt)}..{format(hi, fmt)}"


def print_clusters(profiles, clusters):
    print("\nClusters (sorted by best dJ):")
    for number, group in enumerate(clusters, 1):
        members = [profiles[i] for i in group]
        djs = [profile.number("dJ") for profile in members]
        best_i = max(range(len(members)), key=lambda i: djs[i])
        metric = lambda key: float(np.median([profile.number(key) for profile in members]))
        gates = [int(np.count_nonzero(np.abs(profile.pitches) > 84.9))
                 for profile in members]
        structures = sorted({profile.header["structure"].split()[0] for profile in members})
        cycles = [int(profile.number("cycles")) for profile in members]
        families = counted(profile.family for profile in members)
        print(f"  C{number:02d}  size {len(group):3d}  best dJ {max(djs):9.6f}  "
              f"spread {max(djs) - min(djs):.6g}")
        print(f"       median curv_l1 {metric('curv_l1'):.1f}  "
              f"curv_max {metric('curv_max'):.1f}  lag1 {metric('lag1'):+.3f}")
        print(f"       structure {','.join(structures)}  cycles {counted(cycles)}  "
              f"gate ticks median/range {float(np.median(gates)):.1f}/{range_text(gates, 'd')}")
        print(f"       families {families}")
        print(f"       best {members[best_i].path.name}")


def discovery_curve(labels):
    rng = np.random.default_rng(20260909)
    totals = np.zeros(len(labels), dtype=float)
    for _ in range(SHUFFLES):
        seen = set()
        for k, label in enumerate(rng.permutation(labels)):
            seen.add(int(label))
            totals[k] += len(seen)
    return totals / SHUFFLES


def print_discovery(curve, cluster_count):
    n = len(curve)
    points = {1, 2, 5, 10, 20, 50, 100, n, max(1, int(math.ceil(0.8 * n)))}
    points = sorted(k for k in points if k <= n)
    print(f"\nDiscovery curve ({SHUFFLES} random shuffles):")
    for k in points:
        print(f"  k={k:4d}: {curve[k - 1]:7.3f} expected clusters")
    k80 = max(1, int(math.ceil(0.8 * n)))
    tail = cluster_count - curve[k80 - 1]
    saturated = tail <= max(0.5, 0.05 * cluster_count)
    verdict = "appears to be saturating" if saturated else "is not yet saturating"
    print(f"Seed-set verdict: {verdict}; the final 20% adds {tail:.2f} expected clusters "
          f"toward {cluster_count} total.")


def compare_profile(path, profiles, clusters, threshold):
    external = read_profile(Path(path))
    if external is None:
        raise AtlasError(f"{path}: external profile has no # certified line")
    n = len(profiles[0].pitches)
    if len(external.pitches) != n:
        raise AtlasError(
            f"{path}: external profile has {len(external.pitches)} ticks; atlas has {n}")
    distances = np.asarray(
        [np.mean(np.abs(external.pitches - profile.pitches)) for profile in profiles])
    nearest = int(np.argmin(distances))
    cluster_of = {member: number for number, group in enumerate(clusters, 1)
                  for member in group}
    distance = float(distances[nearest])
    print("\nExternal comparison:")
    if distance <= threshold:
        print(f"  {path} falls into C{cluster_of[nearest]:02d}; nearest member "
              f"{profiles[nearest].path.name} at {distance:.4f} degrees")
    else:
        print(f"  {path} falls into no cluster; nearest is C{cluster_of[nearest]:02d} "
              f"({profiles[nearest].path.name}) at {distance:.4f} degrees, above the "
              f"{threshold:.4f}-degree threshold")


def make_figure(path, profiles, clusters, labels, curve):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError as exc:
        raise AtlasError("--fig requires matplotlib, which is not installed") from exc

    colors = plt.get_cmap("tab20")(np.linspace(0, 1, max(len(clusters), 2)))
    fig, axes = plt.subplots(3, 1, figsize=(12, 11))
    for i, profile in enumerate(profiles):
        axes[0].plot(profile.pitches, color=colors[labels[i] - 1], lw=0.7, alpha=0.55)
    axes[0].axhline(0, color="0.8", lw=0.6)
    for pitch in (-85, 85):
        axes[0].axhline(pitch, color="0.55", lw=0.7, ls=":")
    axes[0].set_ylim(90, -90)       # positive pitch is nose down, so nose up belongs on top
    axes[0].set_xlim(0, len(profiles[0].pitches))
    axes[0].set_ylabel("pitch, deg\n(up = nose up)")
    axes[0].set_xlabel("tick")
    axes[0].set_title(f"All schedules, colored by {len(clusters)} single-linkage clusters")

    dive = [(profile.params.get("flick_tick"), profile.number("dJ"), labels[i])
            for i, profile in enumerate(profiles)
            if profile.family == "diveflick" and "flick_tick" in profile.params]
    if dive:
        for tick, dj, label in dive:
            axes[1].scatter(tick, dj, color=colors[label - 1], s=24, alpha=0.8)
        axes[1].set_xlabel("diveflick seed flick_tick")
        axes[1].set_ylabel("converged dJ")
    else:
        axes[1].text(0.5, 0.5, "No diveflick flick_tick metadata", ha="center", va="center",
                     transform=axes[1].transAxes)
        axes[1].set_axis_off()
    axes[1].set_title("Diveflick entrance time and resulting optimum")

    axes[2].plot(np.arange(1, len(curve) + 1), curve, color="#2ca02c", lw=1.8)
    axes[2].set_xlim(1, len(curve))
    axes[2].set_ylim(0, len(clusters) * 1.05)
    axes[2].set_xlabel("seeds drawn without replacement")
    axes[2].set_ylabel("expected distinct clusters")
    axes[2].set_title(f"Discovery curve, mean of {SHUFFLES} shuffles")
    axes[2].grid(color="0.9", lw=0.6)
    fig.tight_layout()
    fig.savefig(path, dpi=120)
    plt.close(fig)
    print(f"Figure: {path}")


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("celldir", type=Path)
    parser.add_argument("--seeds", help="seeds.json mapping seed names to families and params")
    parser.add_argument("--compare", help="external certified profile to place in the atlas")
    parser.add_argument("--fig", help="write the three-panel summary figure")
    return parser.parse_args()


def main():
    args = parse_args()
    profiles = load_profiles(args.celldir)
    attach_seed_metadata(profiles, args.seeds)
    matrix, distances = pairwise_distances(profiles)
    print_histogram(distances)
    threshold, left, right, ratio = find_threshold(distances)
    print(f"\nThreshold: {threshold:.4f} degrees (empty gap {left:.4f}..{right:.4f}, "
          f"{ratio:.2f}x)")

    djs = np.asarray([profile.number("dJ") for profile in profiles])
    clusters, labels = single_linkage(matrix, threshold, djs)
    print_clusters(profiles, clusters)
    curve = discovery_curve(labels)
    print_discovery(curve, len(clusters))
    if args.compare:
        compare_profile(args.compare, profiles, clusters, threshold)
    if args.fig:
        make_figure(args.fig, profiles, clusters, labels, curve)


if __name__ == "__main__":
    try:
        main()
    except AtlasError as exc:
        print(f"\nATLAS ERROR: {exc}", file=sys.stderr)
        sys.exit(2)
