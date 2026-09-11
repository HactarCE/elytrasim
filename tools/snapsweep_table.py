#!/usr/bin/env python3
"""What the cross product says, one row per cell.

    python3 tools/snapsweep_table.py [celldir] [--csv out.csv]

For each cell: its best single-cycle profile and that profile's flick tick and hold-0 length.
README-atlas reports the same columns for nine cells along the axes; the point of the cross
product is to ask whether the invariants it found there survive the other parameters moving.
"""
import csv
import glob
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plot_atlas


def header(text):
    h = {}
    for line in text.splitlines():
        if line.startswith("# "):
            k, _, v = line[2:].partition(" ")
            h[k] = v.split("#", 1)[0].strip()
    return h


def cell_row(d):
    best = {}
    n_prof = 0
    for f in glob.glob(f"{d}/tight_t*.pitches"):
        t = open(f).read()
        h = header(t)
        if "certified" not in h:
            continue
        n_prof += 1
        p = [float(w) for line in t.splitlines() for w in line.split("#", 1)[0].split()]
        s = h.get("structure", "?")
        dJ = float(h["dJ"].split()[0])
        if dJ > best.get(s, (-1e18,))[0]:
            best[s] = (dJ, p)
    if not best:
        return None
    row = {"cell": os.path.basename(d.rstrip("/")), "profiles": n_prof,
           "n_cyclic": 0, "n_multi": 0, "n_glide": 0}
    for f in glob.glob(f"{d}/tight_t*.pitches"):
        s = header(open(f).read()).get("structure", "?")
        row["n_cyclic"] += s == "cyclic"
        row["n_multi"] += s == "MULTICYCLE"
        row["n_glide"] += s == "COLLAPSED"
    for key, tag in (("cyclic", "cyc"), ("MULTICYCLE", "multi"), ("COLLAPSED", "glide")):
        if key in best:
            dJ, p = best[key]
            row[f"best_{tag}"] = round(dJ, 4)
            if key == "cyclic":
                row["flick"] = plot_atlas.flick_tick(p)
                row["hold0"] = plot_atlas.hold0(p)
    return row


def main():
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    src = a[0] if a else "runs/atlas/snapsweep/out"
    cols = ["cell", "n", "lambda", "vy", "vz", "profiles", "n_cyclic", "n_multi", "n_glide",
            "best_cyc", "flick", "hold0", "best_multi", "best_glide"]
    man = os.path.join(os.path.dirname(src.rstrip("/")), "cells.tsv")
    params = {}
    if os.path.exists(man):
        for line in open(man):
            f = line.rstrip("\n").split("\t")
            if len(f) >= 5:
                params[f[0]] = (int(f[1]), int(f[2]), float(f[3]), float(f[4]))
    else:
        sys.exit(f"no cells.tsv beside {src} -- the cell parameters have no authoritative source")
    rows = []
    for d in sorted(glob.glob(f"{src}/*/")):
        r = cell_row(d)
        if not r:
            continue
        # The cell's parameters come from the manifest, not from its name: a name is a label
        # and a parser over it is a second, silently divergent source of truth.
        if r["cell"] in params:
            n, lam, vy, vz = params[r["cell"]]
            r.update(n=n, **{"lambda": lam}, vy=vy, vz=vz)
        else:
            print(f"  (skipping {r['cell']}: not in cells.tsv)", file=sys.stderr)
            continue
        rows.append(r)
    if "--csv" in sys.argv:
        with open(sys.argv[sys.argv.index("--csv") + 1], "w", newline="") as f:
            w = csv.DictWriter(f, cols, extrasaction="ignore")
            w.writeheader()
            w.writerows(rows)
    hdr = f"{'cell':26s} {'n':>4} {'lam':>4} {'vy':>4} {'vz':>4} {'cyc':>4} {'mlt':>4} {'gld':>4} {'best 1-cyc':>11} {'flick':>6} {'hold0':>6}"
    print(hdr); print("-" * len(hdr))
    for r in rows:
        print(f"{r['cell']:26s} {r['n']:4d} {r['lambda']:+4d} {r['vy']:4.1f} {r['vz']:4.1f} "
              f"{r['n_cyclic']:4d} {r['n_multi']:4d} {r['n_glide']:4d} "
              f"{r.get('best_cyc', float('nan')):11.3f} {r.get('flick', -1):6d} {r.get('hold0', -1):6d}")
    print(f"\n{len(rows)} cells")
    h = [r["hold0"] for r in rows if "hold0" in r]
    if h:
        import collections
        print("hold-0 histogram across every cell:", dict(sorted(collections.Counter(h).items())))


if __name__ == "__main__":
    main()
