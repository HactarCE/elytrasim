#!/usr/bin/env python3
"""Pick the snap-timing window for one cell from its coarse scan, and record the choice.

    python3 tools/snapsweep_pick.py <cellout> <n> [--half 100] [--step 2]

Stage 1 scans flick ticks across the whole horizon at stride 10. This reads those profiles,
takes the best dJ, and prints the stride-2 window to scan around it.

The coarse profiles are deleted once stage 2 has run -- they are scaffolding, and keeping them
would put two different tick strides in one directory for every plot to have to separate. But a
window with no record of why it sits where it does is not reproducible, so the whole coarse
table (every tick, its dJ, its structure) is written to snap_window.json first. That file is a
few kB and makes the deletion safe: the decision survives even though its inputs do not.

dJ alone decides it. Filtering to `structure == cyclic` was considered and dropped -- at stride
10 across a whole horizon the winner is a cyclic profile in every cell we have ever looked at,
and a filter that never fires is a branch nobody tests.
"""
import json
import os
import re
import sys


def header(text):
    out = {}
    for line in text.splitlines():
        if not line.startswith("# "):
            continue
        key, _, val = line[2:].partition(" ")
        out[key] = val.split("#", 1)[0].strip()
    return out


def main():
    a = sys.argv[1:]
    cell, n = a[0], int(a[1])
    half = int(a[a.index("--half") + 1]) if "--half" in a else 100
    step = int(a[a.index("--step") + 1]) if "--step" in a else 2

    rows = []
    coarse = os.path.join(cell, "coarse")
    for name in sorted(os.listdir(coarse)):
        m = re.fullmatch(r"tight_t(\d+)\.pitches", name)
        if not m:
            continue
        h = header(open(os.path.join(coarse, name)).read())
        if "certified" not in h:
            continue
        rows.append({"tick": int(m.group(1)), "dJ": float(h["dJ"].split()[0]),
                     "structure": h.get("structure", "?")})
    if not rows:
        sys.exit(f"{coarse}: no certified coarse profiles")

    best = max(rows, key=lambda r: r["dJ"])
    lo = max(0, best["tick"] - half)
    lo += lo % step                      # stay on the stride grid anchored at 0
    hi = min(n, best["tick"] + half)

    with open(os.path.join(cell, "snap_window.json"), "w") as f:
        json.dump({"n": n, "coarse_step": 10, "half": half, "step": step,
                   "winner": best, "window": [lo, hi],
                   "coarse": sorted(rows, key=lambda r: r["tick"])}, f, indent=1)
    print(f"{lo} {hi}")


if __name__ == "__main__":
    main()
