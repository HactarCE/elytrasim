"""The gain phase's lookahead rule at a price on distance: `argmax dJ` against `argmax dTE`.

    python3 tools/dj_phase.py runs/steady/nlamsweep
    python3 tools/dj_phase.py --stride 8 runs/atlas/mapsweep        # fewer cells
    python3 tools/dj_phase.py --csv /tmp/dj.csv runs/steady/nlamsweep

Each argument is a directory holding a `best.csv` and an `out/` tree; only rows with
`structure = cyclic` are used. Needs a built `target/release/myopic`, which does the physics --
this file only tabulates.

The rule under test. `argmax dTE over n held ticks` is elytrasim's own one-tick rule with the
horizon stretched, and it is blind to the price on distance: at `w != 0` it maximizes something
the cycle is not being paid for. The generalization holds the same pitch for the same `n` ticks
and maximizes the cycle's own objective change, `dJ = dTE + w*dz`. `w = 0` makes the two
identical, which is the check that the harness is wired up; the question is what happens above
and below it.

Both rules are scored against the optimum's own pitches over the ticks where its climb is
nose-up and off its own pitch bound (`free` in `myopic djn`). `n` stays a tuned parameter for
both, so the fair comparison is each rule at its own best `n` -- the `n*` columns.
"""

import csv
import os
import re
import statistics as stat
import subprocess
import sys
from concurrent.futures import ProcessPoolExecutor

MYOPIC = "./target/release/myopic"


def one(args):
    """Run `myopic djn` on one cell; return its per-`n` table plus what the cell is."""
    root, row, nmax = args
    path = os.path.join(root, "out", row["cell"], row["file"])
    # one rayon thread per worker: the pool is already saturating the machine, and nested
    # parallelism here costs about 3x in wall clock
    env = dict(os.environ, RAYON_NUM_THREADS="1")
    txt = subprocess.run([MYOPIC, "djn", path, str(nmax)],
                         capture_output=True, text=True, env=env).stdout.splitlines()
    body = [l for l in txt if not l.startswith("#")]
    head = "".join(l for l in txt if l.startswith("#"))
    if len(body) < 2:
        return None
    tab = list(csv.DictReader(body))
    free = int(re.search(r"free (\d+)", head).group(1))
    t0, t1 = (int(x) for x in re.search(r"arc (\d+)\.\.(\d+)", head).groups())
    if free < 10:                     # too few ticks to read an RMS off
        return None
    best = lambda k: min(tab, key=lambda r: float(r[k]))
    at = lambda r, k: float(r[k])
    bj, bt = best("dj_rms"), best("dte_rms")
    fixed = tab[19] if len(tab) >= 20 else tab[-1]      # the corpus-wide n = 20 stand-in
    # where the schedule's own cut falls inside the climb. A `--steady` profile is closed in
    # velocity but optimized with its terminal velocity free, so the ticks either side of the
    # cut are priced differently and its pitch jumps there; `seam` is the fraction of the climb
    # that lies before the cut, and `None` when the cut is outside the climb entirely.
    nn = int(row["n"])
    seam = (nn - t0) / (t1 - t0) if t0 < nn <= t1 else None
    ps = [float(x) for l in open(path) for x in l.split("#")[0].split()]
    return dict(cell=row["cell"], n=nn, lam=float(row["lam"]), seam=seam,
                jump=abs(ps[0] - ps[-1]),
                dy=float(row["dy"]), dz=float(row["dz"]), free=free,
                dj_n=int(bj["n"]), dj_rms=at(bj, "dj_rms"), dj_med=at(bj, "dj_med"),
                dte_n=int(bt["n"]), dte_rms=at(bt, "dte_rms"), dte_med=at(bt, "dte_med"),
                dj_early=at(bj, "dj_rms_early"), dte_early=at(bt, "dte_rms_early"),
                dj_late=at(bj, "dj_rms_late"), dte_late=at(bt, "dte_rms_late"),
                dj20=at(fixed, "dj_rms"), dte20=at(fixed, "dte_rms"),
                dj20_bias=at(fixed, "dj_bias"), dte20_bias=at(fixed, "dte_bias"))


def read(root, stride, nmax, jobs):
    rows = [r for r in csv.DictReader(open(os.path.join(root, "best.csv")))
            if r["structure"] == "cyclic"][::stride]
    with ProcessPoolExecutor(max_workers=jobs) as ex:
        out = list(ex.map(one, [(root, r, nmax) for r in rows], chunksize=4))
    return [r for r in out if r]


def by_lambda(rows, label, keys):
    """One row per lambda, the median of each key over its cells."""
    lams = sorted({r["lam"] for r in rows})
    print(f"\n  {label}")
    print("    " + "lambda".rjust(8) + "cells".rjust(7) + "".join(k.rjust(11) for k, _ in keys))
    for l in lams:
        g = [r for r in rows if r["lam"] == l]
        line = f"    {l:>8.2f}{len(g):>7}"
        for k, f in keys:
            v = [r[k] for r in g if r[k] == r[k]]         # drop NaN: a short arc has no late set
            line += (format(stat.median(v), f) if v else "-").rjust(11)
        print(line)


def report(root, rows):
    print(f"\n### {os.path.basename(root.rstrip('/'))}: {len(rows)} periodic cycles scored")
    if not rows:
        return
    by_lambda(rows, "each rule at its own best lookahead, and at a shared n = 20", [
        ("dj_n", ">11.0f"), ("dj_rms", ">11.2f"), ("dte_n", ">11.0f"), ("dte_rms", ">11.2f"),
        ("dj20", ">11.2f"), ("dte20", ">11.2f")])
    print("      dj_n/dte_n: the lookahead each rule wants.  *_rms: degrees, at that lookahead.")
    by_lambda(rows, "the same errors split along the arc: first three quarters, last quarter", [
        ("dj_early", ">11.2f"), ("dte_early", ">11.2f"),
        ("dj_late", ">11.2f"), ("dte_late", ">11.2f")])
    print("      the climb's last ticks, where the pitch comes back to 0, are a different problem "
          "from its body.")

    print("\n  does dJ beat dTE? (negative = dJ is closer to the optimum)")
    print("    " + "lambda".rjust(8) + "cells".rjust(7) + "best-n d".rjust(11)
          + "n=20 d".rjust(11) + "win%".rjust(8) + "win% n=20".rjust(11))
    for l in sorted({r["lam"] for r in rows}):
        g = [r for r in rows if r["lam"] == l]
        d1 = [r["dj_rms"] - r["dte_rms"] for r in g]
        d2 = [r["dj20"] - r["dte20"] for r in g]
        print(f"    {l:>8.2f}{len(g):>7}{stat.median(d1):>11.3f}{stat.median(d2):>11.3f}"
              f"{100 * sum(x < 0 for x in d1) / len(g):>8.0f}"
              f"{100 * sum(x < 0 for x in d2) / len(g):>11.0f}")

    # The seam and lambda move together across the corpus -- the cut lands mid-climb more often
    # at a high price on distance -- so bin on both, or the table only re-reads the lambda column.
    bins = [(0, 60), (60, 80), (80, 100), (100, 101)]     # 100 is the "cut outside the climb" bin
    lab = lambda b: "outside" if b[0] == 100 else f"{b[0]}-{b[1]}%"
    frac = lambda r: 100.0 if r["seam"] is None else 100 * r["seam"]
    for key, name in (("dte_rms", "dTE"), ("dj_rms", "dJ")):
        print(f"\n  {name} at its best n, against where the schedule's own cut falls in the climb")
        print("    " + "lambda".rjust(8) + "".join(lab(b).rjust(12) for b in bins))
        for l in sorted({r["lam"] for r in rows}):
            line = f"    {l:>8.2f}"
            for b in bins:
                g = [r for r in rows if r["lam"] == l and b[0] <= frac(r) < b[1]]
                line += (f"{stat.median([r[key] for r in g]):>8.2f}/{len(g):<3}" if g
                         else "        -   ")
            print(line)
    print("      each cell is `rms/cells`. A `--steady` profile is closed in velocity but "
          "optimized with its\n      terminal velocity free, so its pitch jumps at the cut; "
          "the fit degrades with how much of\n      the climb sits on the far side of it.")

    flat = sorted((r for r in rows if abs(r["dy"]) < 1.0), key=lambda r: abs(r["dy"]))
    if flat:
        print(f"\n  the cells that neither climb nor sink (|dy| < 1 block over the cycle): "
              f"{len(flat)} of {len(rows)}")
        print("    " + "cell".ljust(16) + "lambda".rjust(7) + "dy".rjust(8) + "free".rjust(6)
              + "dj n*".rjust(8) + "dj rms".rjust(9) + "dte n*".rjust(8) + "dte rms".rjust(9))
        for r in flat:
            print(f"    {r['cell']:<16}{r['lam']:>7.2f}{r['dy']:>8.2f}{r['free']:>6}"
                  f"{r['dj_n']:>8}{r['dj_rms']:>9.2f}{r['dte_n']:>8}{r['dte_rms']:>9.2f}")


if __name__ == "__main__":
    args = sys.argv[1:]
    opt = dict(stride=1, nmax=48, jobs=os.cpu_count(), csv=None)
    for k in list(opt):
        if f"--{k}" in args:
            i = args.index(f"--{k}")
            opt[k] = args[i + 1] if k == "csv" else int(args[i + 1])
            del args[i:i + 2]
    if not args:
        sys.exit(__doc__)
    every = []
    for root in args:
        rows = read(root, opt["stride"], opt["nmax"], opt["jobs"])
        report(root, rows)
        every += [dict(r, root=root) for r in rows]
    if opt["csv"] and every:
        with open(opt["csv"], "w", newline="") as f:
            wr = csv.DictWriter(f, fieldnames=list(every[0]))
            wr.writeheader()
            wr.writerows(every)
        print(f"\nper-cell rows written to {opt['csv']}")
