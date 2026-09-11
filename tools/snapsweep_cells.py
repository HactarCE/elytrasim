#!/usr/bin/env python3
"""The cell list for the two-stage snap-timing sweep.

Two families, unioned and deduplicated:

  main   the full cross product n x lambda x v0, not just the axes through the reference
         point. The axes answer "what does this parameter do from the reference cell"; the
         cross product answers "does that effect survive the other parameters moving", which
         is the question an interaction can hide from.
  sweep  num_ticks outward from the representative cell, stride 2, at lambda = 0 and v0 = 0.

n = 600 is deliberately absent: it is well inside the two-cycle-optimal zone, so its best
profile answers a different question than the single-cycle cells around it.

    python3 tools/snapsweep_cells.py > cells.tsv
"""
import sys

NS    = [150, 300, 450]
LAMS  = [-2, -1, 0, 1, 2]
V0S   = [(0.0, 0.0), (0.2, 0.0), (0.0, 0.2), (0.2, 0.2)]
SWEEP = range(150, 451, 2)


def tag(n, lam, vy, vz):
    s = "P" if lam >= 0 else "M"
    return f"n{n:04d}_lam{s}{abs(lam)}_vy{round(vy*10):02d}vz{round(vz*10):02d}"


def main():
    cells = {}
    for n in NS:
        for lam in LAMS:
            for vy, vz in V0S:
                cells[tag(n, lam, vy, vz)] = (n, lam, vy, vz, "main")
    for n in SWEEP:
        t = tag(n, 0, 0.0, 0.0)
        # A cell in both families is one cell, listed once, labeled as both.
        cells[t] = (n, 0, 0.0, 0.0, "main+sweep" if t in cells and cells[t][4] == "main" else "sweep")
    for t in sorted(cells):
        n, lam, vy, vz, fam = cells[t]
        print(f"{t}\t{n}\t{lam}\t{vy}\t{vz}\t{fam}")
    print(f"{len(cells)} cells", file=sys.stderr)


if __name__ == "__main__":
    main()
