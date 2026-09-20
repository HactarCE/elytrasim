#!/usr/bin/env python3
"""Best profile per cell: dJ, dy, dz. The input to any post-hoc constraint filter.

    python3 tools/snapsweep_best.py runs/atlas/mapsweep --csv runs/atlas/mapsweep/best.csv

--csv is resolved against the working directory, not against <root>. Spell it out: running this
for two sweeps in a row with a bare `--csv best.csv` writes both to the same file and the second
silently clobbers the first.

"Best" is plain argmax dJ over the cell's certified profiles -- the same thing the optimizer
would hand you at that cell. No structure filter: see snapsweep_pick.py for why.
"""
import argparse
import csv
import glob
import os
import sys
from concurrent.futures import ProcessPoolExecutor

KEEP = ("dJ", "dy", "dz", "structure", "cycles", "lag1", "curv_l1", "v0")


def cell_best(d):
    best = None
    n = 0
    for f in glob.glob(f"{d}/tight_t*.pitches"):
        h = {}
        for line in open(f):
            if not line.startswith("# "):
                break
            k, _, v = line[2:].partition(" ")
            h[k] = v.split("#", 1)[0].strip()
        if "certified" not in h:
            continue
        n += 1
        dJ = float(h["dJ"].split()[0])
        if best is None or dJ > best[0]:
            best = (dJ, {k: h.get(k, "") for k in KEEP}, os.path.basename(f))
    if best is None:
        return None
    row = {"cell": os.path.basename(d.rstrip("/")), "profiles": n, "file": best[2]}
    for k in KEEP:
        v = best[1][k]
        row[k] = (v if k == "v0" else v.split()[0]) if v else ""
    return row


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("root", help="a sweep dir holding cells.tsv and out/")
    ap.add_argument("--csv", required=True)
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args()

    params = {}
    for line in open(os.path.join(a.root, "cells.tsv")):
        f = line.rstrip("\n").split("\t")
        if len(f) >= 5:
            params[f[0]] = (int(f[1]), float(f[2]), float(f[3]), float(f[4]))
    dirs = sorted(glob.glob(os.path.join(a.root, "out", "*/")))
    rows = []
    with ProcessPoolExecutor(max_workers=a.jobs) as ex:
        for r in ex.map(cell_best, dirs, chunksize=32):
            if not r or r["cell"] not in params:
                continue
            n, lam, vy, vz = params[r["cell"]]
            # v0 comes from the winning profile's header, not from cells.tsv. For a fixed-v0
            # grid the two agree exactly and this changes nothing. Under `--steady` they do
            # not: cells.tsv holds the seed handed to the fixed-point iteration, while the
            # profile's v0 is the fixed point it actually reached, which is the velocity the
            # schedule is optimal at and the one `verify` replays from. Reporting the seed
            # would put a velocity in the table that no row of it was solved at.
            hv = r.pop("v0", "")
            if hv:
                vy, vz = (float(x) for x in hv.split()[:2])
            r.update(n=n, lam=lam, vy=vy, vz=vz)
            rows.append(r)
    cols = ["cell", "n", "lam", "vy", "vz", "profiles", "dJ", "dy", "dz",
            "structure", "cycles", "lag1", "curv_l1", "file"]
    with open(a.csv, "w", newline="") as fh:
        w = csv.DictWriter(fh, cols, extrasaction="ignore")
        w.writeheader()
        w.writerows(sorted(rows, key=lambda r: (r["n"], r["lam"], r["vy"], r["vz"])))
    print(f"{len(rows)} cells -> {a.csv}", file=sys.stderr)


if __name__ == "__main__":
    main()
