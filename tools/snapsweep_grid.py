#!/usr/bin/env python3
"""Emit a cells.tsv for an arbitrary (n, lambda, v0) grid.

    python3 tools/snapsweep_grid.py --ns 60:300:2 --lams 0:8:0.25 --v0s 0,0.4 > cells.tsv
    python3 tools/snapsweep_grid.py --ns 150,300,450 --lams -2:2:1 \
        --v0s -0.2,-0.2 -0.2,0 ... --tag v0expand > cells.tsv

Ranges are `lo:hi:step` (inclusive) or a comma list. `--exclude` drops cells already on disk
elsewhere, so an expansion of an existing corpus does not redo what it already has.

lambda is a float here: the map work needs resolution finer than 1 near the dy = 0 frontier,
and the cell name has to stay a filesystem-safe unique label, so 4.25 becomes `lamP4p25`.
"""
import argparse
import sys


def nums(spec, typ):
    if ":" in spec:
        lo, hi, st = (typ(x) for x in spec.split(":"))
        out, x = [], lo
        while x <= hi + 1e-9:
            out.append(round(x, 6) if typ is float else x)
            x += st
        return out
    return [typ(x) for x in spec.split(",")]


def lam_tag(lam):
    s = "P" if lam >= 0 else "M"
    a = abs(lam)
    return f"lam{s}{int(a)}" if a == int(a) else f"lam{s}{int(a)}p{round((a - int(a)) * 100):02d}"


def v_tag(v):
    s = "m" if v < 0 else ""
    return f"{s}{round(abs(v) * 10):02d}"


def tag(n, lam, vy, vz):
    return f"n{n:04d}_{lam_tag(lam)}_vy{v_tag(vy)}vz{v_tag(vz)}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ns", required=True)
    ap.add_argument("--lams", required=True)
    ap.add_argument("--v0s", nargs="+", required=True, help='each "vy,vz"')
    ap.add_argument("--label", default="grid")
    ap.add_argument("--exclude", default=None, help="a cells.tsv whose cells to skip")
    a = ap.parse_args()

    skip = set()
    if a.exclude:
        skip = {line.split("\t")[0] for line in open(a.exclude) if line.strip()}

    n_out = 0
    for n in nums(a.ns, int):
        for lam in nums(a.lams, float):
            for v in a.v0s:
                vy, vz = (float(x) for x in v.split(","))
                t = tag(n, lam, vy, vz)
                if t in skip:
                    continue
                print(f"{t}\t{n}\t{lam:g}\t{vy:g}\t{vz:g}\t{a.label}")
                n_out += 1
    print(f"{n_out} cells", file=sys.stderr)


if __name__ == "__main__":
    main()
