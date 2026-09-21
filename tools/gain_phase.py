"""The gain phase -- the climb after the flick -- and the prices that set its pitch.

    python3 tools/gain_phase.py runs/steady/nlamsweep
    python3 tools/gain_phase.py --stride 20 runs/steady/nlamsweep      # fewer cells

Each argument is a directory holding a `best.csv` and an `out/` tree; only rows with
`structure = cyclic` are used. Needs a built `target/release/myopic`, which does the physics
and the costate solve -- this file only tabulates.

The gain phase's pitch is an explicit function of the price vector and `v_z` (see `gain_pitch`
in `src/opt.rs`), and on the climbing arc the price of upward velocity is a pure clock running
at `DRAG_Y`. So the phase needs exactly two numbers it cannot read off its own state: the apex,
which its own trajectory locates, and `kappa = mu_z(apex)`, the price the dive will pay for the
forward speed the climb hands it. Three tables:

  clocks    the two costate recursions, as residuals -- these are identities, not fits
  kappa     the handoff, against cycle length and the price on distance
  corner    where the climb stops being nose-up, against `mu_z/mu_y = GAIN_RATE`

`kappa` is the reason this is not yet a rule you can fly from the state alone: it is not a
constant. It moves by an order of magnitude across the corpus, monotonically in both axes.
"""

import csv
import os
import re
import statistics as stat
import subprocess
import sys

MYOPIC = "./target/release/myopic"
DRAG_Z = 0.9900000095367432
Y_REF, Z_REF = 21.5, 330.0          # `w = lambda * Y_REF / Z_REF`, as the sweep header says


def read(root, stride):
    """Run `myopic gain` on every cyclic cell and parse what it prints."""
    rows = [r for r in csv.DictReader(open(os.path.join(root, "best.csv")))
            if r["structure"] == "cyclic"][::stride]
    out = []
    for r in rows:
        path = os.path.join(root, "out", r["cell"], r["file"])
        txt = subprocess.run([MYOPIC, "gain", path], capture_output=True, text=True).stdout
        mu = re.search(r"apex prices mu = \(([-\d.]+), ([-\d.]+)\)", txt)
        arc = re.search(r"climbing arc (\d+)\.\.(\d+) \((\d+) ticks\)", txt)
        clk = re.search(r"mu_y over\s+(\d+) branch-off ticks ([\d.e+-]+)\s+"
                        r"mu_z over\s+(\d+) nose-down ticks ([\d.e+-]+)", txt)
        cor = re.search(r"fires at \d+ \(([+-]\d+) ticks\)", txt)
        if not (mu and arc and clk):
            continue
        lam = float(r["lam"])
        out.append(dict(n=int(r["n"]), lam=lam, w=lam * Y_REF / Z_REF,
                        mu_y=float(mu.group(1)), kappa=float(mu.group(2)),
                        arc=int(arc.group(3)),
                        res_y=float(clk.group(2)), res_z=float(clk.group(4)),
                        corner=int(cor.group(1)) if cor else None))
    return out


BIN = 50            # cycle lengths binned so the table stays readable under `--stride`


def grid(rows, key, fmt):
    """One cell per (n rounded to BIN, lambda), the median over everything in it."""
    b = lambda r: BIN * round(r["n"] / BIN)
    ns, lams = sorted({b(r) for r in rows}), sorted({r["lam"] for r in rows})
    print("    " + "".join(f"{l:>8.0f}" for l in lams) + "     lambda")
    for n in ns:
        line = f"{n:>4}"
        for l in lams:
            g = [key(r) for r in rows if b(r) == n and r["lam"] == l]
            line += format(stat.median(g), fmt) if g else "       -"
        print(line)


def report(root, stride):
    rows = read(root, stride)
    print(f"\n### {os.path.basename(root.rstrip('/'))}: {len(rows)} periodic cycles")
    if not rows:
        return

    print("\n  clocks -- mu_y = 1 + DRAG_Y mu_y' where the down-to-forward branch is off,")
    print("            mu_z = w + DRAG_Z mu_z' where the pitch is not nose-up")
    for lab, k in (("mu_y", "res_y"), ("mu_z", "res_z")):
        v = [r[k] for r in rows]
        print(f"    {lab}: median residual {stat.median(v):.1e}, worst {max(v):.1e}")

    print("\n  kappa = mu_z(apex), the price handed to the dive")
    grid(rows, lambda r: r["kappa"], ">8.2f")
    print("\n  kappa - w/(1 - DRAG_Z), the same with the price's own fixed point removed")
    grid(rows, lambda r: r["kappa"] - r["w"] / (1 - DRAG_Z), ">8.2f")
    print("\n  mu_y(apex) -- zero is the claim; it is what makes the apex the right horizon")
    grid(rows, lambda r: r["mu_y"], ">8.2f")
    print("\n  climbing-arc length, ticks")
    grid(rows, lambda r: r["arc"], ">8.0f")

    cs = [r["corner"] for r in rows if r["corner"] is not None]
    if cs:
        inside = 100 * sum(1 for x in cs if abs(x) <= 1) / len(cs)
        print(f"\n  corner mu_z/mu_y = GAIN_RATE against the tick the climb stops being nose-up:")
        print(f"    {len(cs)} of {len(rows)} cells return to pitch 0 at all; "
              f"median {stat.median(cs):+.0f} ticks, {inside:.0f}% inside one tick")


if __name__ == "__main__":
    args = sys.argv[1:]
    stride = 1
    if "--stride" in args:
        i = args.index("--stride")
        stride = int(args[i + 1])
        del args[i:i + 2]
    if not args:
        sys.exit(__doc__)
    for root in args:
        report(root, stride)
