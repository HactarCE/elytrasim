#!/usr/bin/env python3
"""Emit a cells.tsv for an arbitrary (n, lambda, v0) grid.

    python3 tools/snapsweep_grid.py --ns 60:300:2 --lams 0:8:0.25 --v0s 0,0.4 > cells.tsv
    python3 tools/snapsweep_grid.py --ns 150,300,450 --lams=-2:2:1 \
        --v0s='-0.2,-0.2;-0.2,0;0.4,0.4' --label v0expand > cells.tsv

Ranges are `lo:hi:step` (inclusive) or a comma list. `--exclude` drops cells already on disk
elsewhere, so an expansion of an existing corpus does not redo what it already has.

lambda is a float here: the map work needs resolution finer than 1 near the dy = 0 frontier,
and the cell name has to stay a filesystem-safe unique label, so 4.25 becomes `lamP4p25`.
Velocity takes the same shape in tenths -- 0.2 is `02`, 0.05 is `00p50` -- which resolves a
0.001 grid. Finer than that is refused rather than silently collided; see the note in v_tag.
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
    # Velocity is tagged in tenths, so the two-digit form only spans a 0.1 grid. A finer grid
    # collides *silently* -- 0.15, 0.2 and 0.25 all used to render as "02", so three velocities
    # would share one cell directory and overwrite each other's profiles. Anything that is not a
    # whole tenth takes lam_tag's shape instead: whole units, "p", hundredths of a unit. Values
    # on the 0.1 grid keep the name they already have on disk.
    s = "m" if v < 0 else ""
    a = round(abs(v) * 10, 6)                       # tenths; 0.3 * 10 is 2.9999999999999996
    if a == int(a):
        return f"{s}{int(a):02d}"
    return f"{s}{int(a):02d}p{round((a - int(a)) * 100):02d}"


def tag(n, lam, vy, vz):
    return f"n{n:04d}_{lam_tag(lam)}_vy{v_tag(vy)}vz{v_tag(vz)}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ns", required=True)
    ap.add_argument("--lams", required=True)
    # One ';'-separated string rather than nargs="+": a v0 list starting with a negative
    # velocity ("-0.2,0.0") is read by argparse as an option flag, and `--v0s=...` does not
    # rescue a multi-value argument.
    ap.add_argument("--v0s", required=True, help='semicolon-separated "vy,vz" pairs')
    ap.add_argument("--label", default="grid")
    ap.add_argument("--exclude", default=None, help="a cells.tsv whose cells to skip")
    a = ap.parse_args()

    skip = set()
    if a.exclude:
        skip = {line.split("\t")[0] for line in open(a.exclude) if line.strip()}

    out = []
    # The cell name is the output *directory*, and the solver resumes by file existence, so two
    # parameter tuples sharing a name do not collide loudly -- they interleave into one directory
    # and the sweep reports success. The tag encodes velocity in tenths and lambda in hundredths,
    # so any grid finer than that quietly loses columns; refuse to emit rather than find out later.
    seen = {}
    for n in nums(a.ns, int):
        for lam in nums(a.lams, float):
            for v in a.v0s.split(";"):
                vy, vz = (float(x) for x in v.split(","))
                t = tag(n, lam, vy, vz)
                cell = (n, lam, vy, vz)
                if seen.setdefault(t, cell) != cell:
                    sys.exit(f"{t}: cell name collision, {seen[t]} and {cell} -- the grid is "
                             f"finer than the name can encode")
                if t in skip:
                    continue
                out.append(f"{t}\t{n}\t{lam:g}\t{vy:g}\t{vz:g}\t{a.label}")
    # Buffered to here so a collision leaves no half-written cells.tsv on stdout: the caller
    # redirects to a file, and a truncated grid that exited 1 still looks like a grid.
    print("\n".join(out))
    print(f"{len(out)} cells", file=sys.stderr)


if __name__ == "__main__":
    main()
