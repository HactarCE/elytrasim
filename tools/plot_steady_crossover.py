#!/usr/bin/env python3
"""Resolve the apparent v0-component crossover near peak steady climb rate.

Usage:
    python3 tools/plot_steady_crossover.py [SWEEP_DIR] [OUT_DIR]
"""

import csv
import glob
import os
import re
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np


ROOT = sys.argv[1] if len(sys.argv) > 1 else "runs/steady/nlamsweep"
OUT = sys.argv[2] if len(sys.argv) > 2 else "runs/steady/fig"

BG = "#14171a"
PANEL = "#1b1f24"
FG = "#e6e9ed"
DIM = "#b9c0c9"
GRID = "#5d6774"
RATE = "#d2a8ff"
BEST = "#55c1ff"
ALT = "#8b949e"
SWITCH = "#db6d28"


def header(path):
    values = {}
    with open(path) as fh:
        for line in fh:
            if not line.startswith("# "):
                break
            key, _, value = line[2:].partition(" ")
            values[key] = value.split("#", 1)[0].strip()
    return values


def load_best():
    path = os.path.join(ROOT, "best.csv")
    with open(path, newline="") as fh:
        rows = list(csv.DictReader(fh))
    parsed = []
    for row in rows:
        parsed.append({
            "n": int(row["n"]),
            "dJ": float(row["dJ"]),
            "vdiff": float(row["vy"]) - float(row["vz"]),
            "file": row["file"],
        })
    return sorted(parsed, key=lambda row: row["n"])


def load_candidates(ns):
    candidates = []
    for n in ns:
        pattern = os.path.join(ROOT, "out", f"n{n:04d}_lamP0", "tight_t*.pitches")
        cell = []
        for path in glob.glob(pattern):
            h = header(path)
            if "certified" not in h:
                continue
            vy, vz = (float(value) for value in h["v0"].split()[:2])
            tick = int(re.search(r"tight_t(\d+)", path).group(1))
            cell.append({"n": n, "tick": tick, "dJ": float(h["dJ"].split()[0]),
                         "vdiff": vy - vz})
        if cell:
            top = max(row["dJ"] for row in cell)
            for row in cell:
                row["loss"] = top - row["dJ"]
            candidates.extend(cell)
    return candidates


def style_axis(ax):
    ax.set_facecolor(PANEL)
    ax.grid(True, color=GRID, alpha=0.34, linewidth=0.8)
    ax.set_axisbelow(True)
    ax.tick_params(colors=DIM)
    for spine in ax.spines.values():
        spine.set_color(GRID)


def main():
    best = load_best()
    local = [row for row in best if 240 <= row["n"] <= 272]
    ns = np.array([row["n"] for row in local], dtype=float)
    rates = np.array([row["dJ"] / row["n"] for row in local])

    raw_i = int(np.argmax(rates))
    raw_peak = ns[raw_i]
    fit_mask = (ns >= 248) & (ns <= 264)
    fit = np.polyfit(ns[fit_mask], rates[fit_mask], 2)
    smooth_peak = -fit[1] / (2 * fit[0])
    fit_x = np.linspace(244, 268, 241)

    switches = []
    for left, right in zip(local, local[1:]):
        if left["vdiff"] * right["vdiff"] < 0:
            switches.append((left, right))
    if len(switches) != 1:
        raise RuntimeError(f"expected one local sign change, found {len(switches)}")
    left, right = switches[0]
    switch_interp = left["n"] - left["vdiff"] * (right["n"] - left["n"]) / (
        right["vdiff"] - left["vdiff"]
    )

    candidates = load_candidates([int(n) for n in ns])
    near = [row for row in candidates if row["loss"] <= 0.025]

    plt.rcParams.update({
        "figure.facecolor": BG,
        "savefig.facecolor": BG,
        "text.color": FG,
        "axes.labelcolor": FG,
        "axes.titlecolor": FG,
        "font.family": "sans-serif",
        "font.size": 10,
    })
    fig, axes = plt.subplots(2, 1, figsize=(11.0, 7.2), sharex=True,
                             gridspec_kw={"hspace": 0.12})
    fig.subplots_adjust(left=0.11, right=0.97, top=0.87, bottom=0.11)
    for ax in axes:
        style_axis(ax)
        ax.axvspan(left["n"], right["n"], color=SWITCH, alpha=0.10)

    ax = axes[0]
    ax.plot(ns, rates, color=RATE, linewidth=1.6, marker="o", markersize=3.5,
            label="sampled best profile")
    ax.plot(fit_x, np.polyval(fit, fit_x), color=FG, alpha=0.58, linewidth=1.0,
            linestyle="--", label="local quadratic fit, n = 248…264")
    ax.axvline(raw_peak, color=RATE, alpha=0.65, linewidth=1.0)
    ax.axvline(smooth_peak, color=FG, alpha=0.55, linewidth=1.0, linestyle="--")
    ax.set_ylabel("dJ / num_ticks\n(blocks/tick)")
    ax.legend(frameon=False, labelcolor=FG, loc="lower left")
    ax.annotate(f"sampled peak  {raw_peak:.0f}", (raw_peak, rates[raw_i]),
                xytext=(-8, 18), textcoords="offset points", ha="right", color=RATE)
    ax.text(smooth_peak + 0.4, np.polyval(fit, smooth_peak),
            f"fitted peak  {smooth_peak:.1f}", color=FG, fontsize=9, va="bottom")

    ax = axes[1]
    ax.axhline(0, color=FG, alpha=0.55, linewidth=0.9)
    ax.scatter([row["n"] for row in near], [row["vdiff"] for row in near],
               color=ALT, alpha=0.50, s=18, label="alternatives within 0.025 dJ")
    ax.plot(ns, [row["vdiff"] for row in local], color=BEST, linewidth=1.5,
            marker="o", markersize=4.0, label="selected best profile")
    ax.set_ylabel(r"$v_{0,y} - v_{0,z}$ (blocks/tick)")
    ax.set_xlabel("num_ticks")
    ax.legend(frameon=False, labelcolor=FG, loc="lower left")
    ax.annotate(
        f"selected branch changes sign\n{left['n']} → {right['n']} ticks",
        ((left["n"] + right["n"]) / 2, 0),
        xytext=(18, 24),
        textcoords="offset points",
        color=SWITCH,
        fontsize=9,
        arrowprops={"arrowstyle": "-", "color": SWITCH, "lw": 0.9},
    )
    ax.set_xlim(ns.min(), ns.max())

    fig.suptitle("the apparent crossover is a best-profile branch switch",
                 fontsize=16, fontweight="normal", y=0.96)
    fig.text(
        0.11,
        0.905,
        f"raw rate peak {raw_peak:.0f}  ·  local fitted peak {smooth_peak:.1f}  ·  "
        f"sign-change interpolation {switch_interp:.1f} ticks",
        color=DIM,
        fontsize=10,
    )

    os.makedirs(OUT, exist_ok=True)
    png = os.path.join(OUT, "steady-crossover.png")
    svg = os.path.join(OUT, "steady-crossover.svg")
    fig.savefig(png, dpi=180)
    fig.savefig(svg)
    print(f"wrote {png}")
    print(f"wrote {svg}")
    print(f"sampled_peak={raw_peak:.0f} fitted_peak={smooth_peak:.4f} "
          f"switch_interp={switch_interp:.4f}")


if __name__ == "__main__":
    main()
