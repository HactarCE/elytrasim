#!/usr/bin/env python3
"""Build the seeds and work list for one stage, in a single interpreter.

    RUN=... python3 tools/snapsweep_build.py 1 [--jobs 8]
    RUN=... python3 tools/snapsweep_build.py 2 --half 20 [--jobs 8]

This replaced a shell version (snapsweep_build.sh, removed 2026-09-19) that spawned one Python
per cell: invisible at 208 cells (55s), eleven minutes at 3993, and stage 2 paid it twice
because it also picks each cell's window. Here the helpers are imported once and called in a
loop, and the cells (which are independent) go over a process pool. 3406 cells build in 39s.

The helpers are invoked through their own main() with sys.argv patched rather than by reaching
into their internals, so there is still exactly one implementation of what a seed file is.
"""
import argparse
import contextlib
import io
import os
import sys
from concurrent.futures import ProcessPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import atlas_seeds
import snapsweep_pick


def call(mod, argv):
    old = sys.argv
    sys.argv = argv
    try:
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            mod.main()
        return buf.getvalue()
    finally:
        sys.argv = old


def one(job):
    # lam/vy/vz stay the verbatim strings from cells.tsv rather than being reformatted: the
    # work list is compared against the shell builder's, and resume is by output-file existence,
    # so "0.0" must not silently become "0".
    stage, half, name, n, lam, vy, vz = job
    if stage == 1:
        sd, od, step, lo, hi = f"seeds/{name}/coarse", f"out/{name}/coarse", 10, 0, n
    else:
        sd, od, step = f"seeds/{name}/fine", f"out/{name}", 2
        try:
            w = call(snapsweep_pick, ["snapsweep_pick", f"out/{name}", str(n),
                                      "--half", str(half), "--step", str(step)])
        except SystemExit as e:
            return (name, None, f"pick failed: {e}")
        lo, hi = (int(x) for x in w.split())
    if not os.path.exists(f"{sd}/seeds.json"):
        call(atlas_seeds, ["atlas_seeds", "--n", str(n), "--flick-step", str(step),
                           "--flick-lo", str(lo), "--flick-hi", str(hi), "--out", sd])
    os.makedirs(od, exist_ok=True)
    files = sorted(f for f in os.listdir(sd) if f.startswith("tight_t") and f.endswith(".pitches"))
    return (name, [f"{sd}/{f} {od}/{f} {n} {lam} {vy} {vz}" for f in files], None)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("stage", type=int, choices=(1, 2))
    # No default: --half decides what the corpus is, not merely how long it takes, and the two
    # widths produce populations that are not comparable. Stage 1 never uses it.
    ap.add_argument("--half", type=int, default=(int(os.environ["HALF"]) if "HALF" in os.environ
                                                 else None))
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args()
    run = os.environ.get("RUN") or sys.exit("set RUN to the run directory")
    if a.stage == 2 and a.half is None:
        sys.exit("stage 2 needs --half (or HALF=) -- 20 keeps the optimum, 100 keeps the population")
    os.chdir(run)
    for d in ("seeds", "out", "work", "logs"):
        os.makedirs(d, exist_ok=True)

    jobs = []
    for line in open("cells.tsv"):
        f = line.rstrip("\n").split("\t")
        if len(f) >= 5:
            jobs.append((a.stage, a.half, f[0], int(f[1]), f[2], f[3], f[4]))

    lines, skipped = [], []
    with ProcessPoolExecutor(max_workers=a.jobs) as ex:
        for name, out, err in ex.map(one, jobs, chunksize=16):
            if err:
                skipped.append((name, err))
            else:
                lines.extend(out)
    work = f"work/stage{a.stage}.tsv"
    with open(work, "w") as fh:
        fh.write("\n".join(lines) + ("\n" if lines else ""))
    print(f"stage {a.stage}: {len(lines)} solves queued in {work}"
          + (f"  ({len(skipped)} cells skipped)" if skipped else ""))
    for name, err in skipped[:5]:
        print(f"  SKIP {name}: {err}", file=sys.stderr)


if __name__ == "__main__":
    main()
