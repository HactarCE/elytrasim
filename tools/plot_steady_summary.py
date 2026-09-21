#!/usr/bin/env python3
"""Plot steady-state fixed point and gain against the allocated horizon.

Usage:
    python3 tools/plot_steady_summary.py [BEST.csv] [OUT_DIR]
"""

import csv
import os
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import FuncFormatter


SRC = sys.argv[1] if len(sys.argv) > 1 else "runs/steady/nlamsweep/best.csv"
OUT = sys.argv[2] if len(sys.argv) > 2 else "runs/steady/fig"

BG = "#14171a"
PANEL = "#1b1f24"
FG = "#e6e9ed"
DIM = "#b9c0c9"
GRID = "#5d6774"
VY = "#55c1ff"
VZ = "#db6d28"
GAIN = "#7ee787"
RATE = "#d2a8ff"


def load_rows(path):
    with open(path, newline="") as fh:
        rows = list(csv.DictReader(fh))
    required = {"n", "vy", "vz", "dJ"}
    missing = required - set(rows[0] if rows else ())
    if missing:
        raise RuntimeError(f"{path} is missing columns: {', '.join(sorted(missing))}")
    parsed = [
        {
            "n": int(row["n"]),
            "vy": float(row["vy"]),
            "vz": float(row["vz"]),
            "dJ": float(row["dJ"]),
        }
        for row in rows
    ]
    return sorted(parsed, key=lambda row: row["n"])


def style_axis(ax):
    ax.set_facecolor(PANEL)
    ax.grid(True, color=GRID, alpha=0.34, linewidth=0.8)
    ax.set_axisbelow(True)
    ax.tick_params(colors=DIM)
    for spine in ax.spines.values():
        spine.set_color(GRID)


def main():
    rows = load_rows(SRC)
    if not rows:
        raise RuntimeError(f"no rows in {SRC}")

    n = [row["n"] for row in rows]
    vy = [row["vy"] for row in rows]
    vz = [row["vz"] for row in rows]
    gain = [row["dJ"] for row in rows]
    rate = [row["dJ"] / row["n"] for row in rows]

    plt.rcParams.update({
        "figure.facecolor": BG,
        "savefig.facecolor": BG,
        "text.color": FG,
        "axes.labelcolor": FG,
        "axes.titlecolor": FG,
        "font.family": "sans-serif",
        "font.size": 10,
    })
    fig, axes = plt.subplots(
        3,
        1,
        figsize=(11.0, 8.0),
        sharex=True,
        gridspec_kw={"height_ratios": (1.15, 1.0, 1.0), "hspace": 0.10},
    )
    fig.subplots_adjust(left=0.12, right=0.97, top=0.90, bottom=0.10)

    for ax in axes:
        style_axis(ax)

    axes[0].plot(n, vy, color=VY, linewidth=1.65, label=r"$v_{0,y}$")
    axes[0].plot(n, vz, color=VZ, linewidth=1.65, label=r"$v_{0,z}$")
    axes[0].axhline(0, color=FG, alpha=0.30, linewidth=0.8)
    axes[0].set_ylabel("fixed-point $v_0$\n(blocks/tick)")
    axes[0].legend(frameon=False, ncols=2, loc="upper right", labelcolor=FG)

    axes[1].plot(n, gain, color=GAIN, linewidth=1.85)
    axes[1].set_ylabel("dJ (blocks)")

    axes[2].plot(n, rate, color=RATE, linewidth=1.85)
    axes[2].set_ylabel("dJ / num_ticks\n(blocks/tick)")
    axes[2].set_xlabel("num_ticks")
    axes[2].yaxis.set_major_formatter(FuncFormatter(lambda value, _: f"{value:.3f}"))

    peak = max(range(len(rate)), key=rate.__getitem__)
    axes[2].scatter(n[peak], rate[peak], s=28, color=RATE, edgecolor=BG, zorder=4)
    axes[2].annotate(
        f"peak at {n[peak]} ticks\n{rate[peak]:.4f} blocks/tick",
        (n[peak], rate[peak]),
        xytext=(12, 12),
        textcoords="offset points",
        color=FG,
        fontsize=9,
        arrowprops={"arrowstyle": "-", "color": DIM, "lw": 0.8},
    )

    axes[2].set_xlim(min(n), max(n))
    fig.suptitle("steady-state sweep", fontsize=16, fontweight="medium", y=0.965)
    fig.text(
        0.12,
        0.925,
        f"lambda = 0  ·  {len(rows)} optimized horizons  ·  fixed-point initial velocity",
        color=DIM,
        fontsize=10,
    )

    os.makedirs(OUT, exist_ok=True)
    png = os.path.join(OUT, "steady-summary.png")
    svg = os.path.join(OUT, "steady-summary.svg")
    fig.savefig(png, dpi=180)
    fig.savefig(svg)
    print(f"wrote {png}")
    print(f"wrote {svg}")


if __name__ == "__main__":
    main()
