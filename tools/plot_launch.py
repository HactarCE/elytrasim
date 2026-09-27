"""Plot `examples/booster.rs launch`: best launch pitch, best apex and time to apex against the
booster's speed, with the band of launch pitches whose apex is within 1 block of the best.
Pitches are Minecraft's, negative nose-up.

    TRIG=mth_lut LIM=89 cargo run --release --example booster -- launch > launch.csv
    python3 tools/plot_launch.py launch.csv out.png
"""
import csv
import sys
from collections import defaultdict

import matplotlib.pyplot as plt

INK, INK2, GRID, SURFACE = "#1f1f1e", "#5f5e58", "#e4e3dc", "#fcfcfb"
SERIES, BAND = "#2a78d6", "#b7d3f6"
BOOSTERS = [(1.65, "goldrush"), (3.0, "monster")]  # measured on EMC


def band_edges(grid, best, tol):
    """Launch pitches whose apex is within `tol` of `best`, interpolated off the grid."""
    grid = sorted(grid)
    inside = [i for i, (_, h) in enumerate(grid) if h >= best - tol]
    lo_i, hi_i = inside[0], inside[-1]

    def cross(i, j):
        (g0, h0), (g1, h1) = grid[i], grid[j]
        return g0 + (best - tol - h0) * (g1 - g0) / (h1 - h0)

    lo = cross(lo_i - 1, lo_i) if lo_i > 0 else grid[0][0]
    hi = cross(hi_i, hi_i + 1) if hi_i + 1 < len(grid) else grid[-1][0]
    return lo, hi


def main(path, out):
    grid, best = defaultdict(list), {}
    for r in csv.DictReader(open(path)):
        v = float(r["speed"])
        row = (float(r["pitch"]), float(r["apex"]), int(r["apex_tick"]))
        if r["kind"] == "grid":
            grid[v].append(row[:2])
        else:
            best[v] = row
    vs = sorted(best)
    pitch = [best[v][0] for v in vs]
    apex = [best[v][1] for v in vs]
    ticks = [best[v][2] for v in vs]
    edges = [band_edges(grid[v], best[v][1], 1.0) for v in vs]

    fig, axes = plt.subplots(1, 3, figsize=(16, 5), facecolor=SURFACE)
    for ax in axes:
        ax.set_facecolor(SURFACE)
        ax.grid(color=GRID, lw=0.8)
        ax.tick_params(colors=INK2, labelsize=9)
        for s in ax.spines.values():
            s.set_visible(False)
        for v, name in BOOSTERS:
            ax.axvline(v, color=INK2, lw=1, ls=(0, (2, 3)))
        ax.set_xlabel("booster speed |v|, blocks/tick  (×20 for b/s)", color=INK2)

    axes[0].fill_between(vs, [e[0] for e in edges], [e[1] for e in edges], color=BAND, lw=0,
                         label="apex within 1 block of best")
    axes[0].plot(vs, pitch, color=SERIES, lw=2, marker="o", ms=4, label="best launch pitch")
    axes[0].set(ylabel="look pitch at the booster, deg  (negative is up)", title="best launch pitch")
    axes[0].legend(frameon=False, fontsize=9, labelcolor=INK, loc="lower right")
    axes[1].plot(vs, apex, color=SERIES, lw=2, marker="o", ms=4)
    axes[1].set(ylabel="apex above booster, blocks", title="best achievable apex")
    axes[2].plot(vs, ticks, color=SERIES, lw=2, marker="o", ms=4)
    axes[2].set(ylabel="ticks from launch to apex  (÷20 for s)", title="time to that apex")

    for ax, ys in zip(axes, (pitch, apex, ticks)):
        ax.title.set_color(INK)
        ax.yaxis.label.set_color(INK2)
        top = ax.get_ylim()[1]
        for v, name in BOOSTERS:
            ax.annotate(name, (v, top), xytext=(3, -12), textcoords="offset points",
                        color=INK2, fontsize=8)
    fig.suptitle("Booster launch: optimal climb after a fixed-speed boost (vanilla trig, |pitch| ≤ 89)",
                 color=INK, fontsize=12)
    fig.tight_layout()
    fig.savefig(out, dpi=140, facecolor=SURFACE)


if __name__ == "__main__":
    main(*sys.argv[1:3])
