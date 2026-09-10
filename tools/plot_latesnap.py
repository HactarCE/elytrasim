#!/usr/bin/env python3
"""Figure 04: what a late snap costs, priced by a stopping time rather than a constraint.

    python3 tools/plot_latesnap.py [outdir]      default runs/atlas/fig

Free optimization will not hold a late flick. Seeded past tick 216 the polish walks it back to
215-216, so the whole late region is unsampled -- and on a figure, unsampled reads as absent,
which reads as bad. Three different things.

The instrument here is simply to stop early: seed the flick late, run a few coordinate passes,
keep what you get. Nothing is forbidden, so nothing can be gamed, and the price of that is only
that the answer depends on the budget -- which is why every budget is drawn. The alternative
instruments (a hard --flick-at constraint, and freezing the manoeuvre and translating it) were
tried and abandoned; see README-atlas.md for why, and note that the constraint turned out to be
satisfiable by a schedule that is not late at all.

Read the spread between the budget curves as the width of the answer, not as noise.
"""

import glob
import os
import re
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.colors as mcolors

# Budget, cell, color. Ordered by budget so the legend reads as a progression.
BUDGETS = [(3, "flicksoft3_v00_n300", "#8ecae6"),
           (10, "flicksoft10_v00_n300", "#4cc9a4"),
           (30, "flicksoft30_v00_n300", "#ffd166"),
           (400, "flick_v00_n300", "#ff5d73")]
WINDOW = 20     # ticks past the optimum worth caring about: half a second, not a second and a half


def theme():
    plt.style.use("dark_background")
    plt.rcParams.update({
        "figure.facecolor": "#14171a", "savefig.facecolor": "#14171a",
        "axes.facecolor": "#1b1f24", "axes.edgecolor": "#4a525c", "grid.color": "#5d6774",
        "text.color": "#e6e9ed", "axes.labelcolor": "#e6e9ed",
        "xtick.color": "#b9c0c9", "ytick.color": "#b9c0c9",
        "legend.facecolor": "#1b1f24", "legend.edgecolor": "#4a525c", "legend.framealpha": .92,
    })


def read(cell):
    """(flick tick, dJ, pitches) for every cyclic schedule that actually flicks."""
    out = []
    for f in sorted(glob.glob(f"runs/atlas/cells/{cell}/*.pitches")):
        text = open(f).read()
        if "# certified" not in text:
            continue
        head = dict(re.findall(r"^# (\w+)\s+(\S+)", text, re.M))
        if head["structure"] != "cyclic":
            continue
        p = [float(x) for l in text.splitlines() for x in l.split("#")[0].split()]
        t = next((i for i, v in enumerate(p) if v <= -80.0), None)
        if t is not None:
            out.append((t, float(head["dJ"]), p))
    return sorted(out)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "runs/atlas/fig"
    os.makedirs(out, exist_ok=True)
    theme()
    series = [(b, read(c), col) for b, c, col in BUDGETS]
    missing = [b for b, rows, _ in series if not rows]
    assert not missing, f"no cyclic profiles for budgets {missing}; run tools/atlas_run.sh first"

    ref = max(series, key=lambda s: s[0])[1]          # the fully-converged cell sets the peak
    peak = max(r[1] for r in ref)
    tpeak = min(r[0] for r in ref if r[1] == peak)

    fig, ax = plt.subplots(1, 2, figsize=(16, 6.4))
    ax[0].axvspan(tpeak, tpeak + WINDOW, color="#2a3441", zorder=0)
    ax[0].text(tpeak + 1, 2.0, f"the window worth caring about:\nup to {WINDOW} ticks late",
               fontsize=8.5, color="#9fb0c4")
    for b, rows, color in series:
        # The converged series is the point of the figure -- it is the one that stops short --
        # so draw it last, thicker, and in a color none of the budgets uses.
        style, ms, lw, z = ("o-", 4, 1.4, 2) if b < 400 else ("o", 5.5, 0, 5)
        ax[0].plot([r[0] for r in rows], [r[1] for r in rows], style, ms=ms, lw=lw, color=color,
                   alpha=.95, zorder=z, label=f"{b} passes" + (" (converged)" if b == 400 else "")
                   + f"   n={len(rows)}, flick {rows[0][0]}-{rows[-1][0]}")
    ax[0].axvline(tpeak, color="0.5", ls=":", lw=1)
    ax[0].axhline(0, color="0.45", lw=.8)
    ax[0].set_xlabel("tick of the flick (first crossing of -80 deg)")
    ax[0].set_ylabel("dJ, blocks")
    ax[0].set_title(f"more optimization destroys the late flick rather than improving it",
                    fontsize=11)
    ax[0].legend(fontsize=8, loc="lower left", title="coordinate-pass budget", title_fontsize=8)
    ax[0].grid(alpha=.3)

    # Right: the schedules inside the window, from the largest budget that still reaches the end
    # of it -- the most-optimized honest picture of a late snap.
    win = []
    for b, rows, _ in sorted(series, reverse=True):
        win = [r for r in rows if tpeak <= r[0] <= tpeak + WINDOW]
        if win and max(r[0] for r in win) >= tpeak + WINDOW - 2:
            budget = b
            break
    lo, hi = min(r[1] for r in win), max(r[1] for r in win)
    norm = mcolors.Normalize(lo, hi)
    cmap = plt.get_cmap("viridis")
    for r in win:
        ax[1].plot(r[2], lw=1.1, alpha=.9, color=cmap(norm(r[1])))
    ax[1].axhline(0, color="0.55", lw=.8)
    ax[1].invert_yaxis()
    ax[1].set_xlabel("tick")
    ax[1].set_ylabel("pitch, deg  (nose-up at top)")
    ax[1].set_title(f"the {len(win)} schedules in that window at {budget} passes "
                    f"(flick {min(r[0] for r in win)}-{max(r[0] for r in win)}, "
                    f"dJ {hi:+.2f} to {lo:+.2f})", fontsize=11)
    ax[1].grid(alpha=.3)
    fig.colorbar(matplotlib.cm.ScalarMappable(norm=norm, cmap=cmap), ax=ax[1], label="dJ, blocks")

    fig.suptitle(
        "PRICED BY A STOPPING TIME, NOT A CONSTRAINT. Free optimization slides a late flick back "
        "to tick 215-216, so the late region cannot be reached\nby optimizing harder -- it is "
        "reached by optimizing less. Nothing is forbidden here, so nothing can be gamed; the cost "
        "is that the answer depends\non the pass budget, by about half a block at +20 ticks. The "
        "spread between the curves is the width of the answer, not noise.",
        fontsize=9.5, color="#ffd166", y=.985, va="top", linespacing=1.45)
    plt.tight_layout()
    fig.subplots_adjust(top=.845)
    plt.savefig(f"{out}/04_latesnap.png", dpi=110)
    plt.close()

    print(f"wrote {out}/04_latesnap.png")
    for b, rows, _ in series:
        pk = max(r[1] for r in rows)
        print(f"  {b:>3} passes: n={len(rows):>3}  flick {rows[0][0]}-{rows[-1][0]}  peak {pk:.3f}")


if __name__ == "__main__":
    main()
