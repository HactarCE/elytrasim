#!/usr/bin/env python3
"""Plot dJ gain rate against allocated horizon for the Atlas lambda=0 sweeps.

This deliberately does not read ``best.csv``.  It reads every certified
``tight_t*.pitches`` profile in the lambda=0 cells, then chooses the largest
dJ in each cell.  Line color is a bivariate encoding of the initial velocity.

Usage:
    python3 tools/plot_gain_efficiency.py [OUT_DIR]
"""

import csv
import colorsys
import glob
import os
import sys
from collections import defaultdict

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt


plt.style.use("dark_background")


ROOT = "runs/atlas"
OUT = sys.argv[1] if len(sys.argv) > 1 else f"{ROOT}/fig/efficiency"
TICKS_PER_SECOND = 20.0
N_MIN = 150
N_MAX = 350
SWEEP = "nsweepv0"
V_MIN = 0.0
V_MAX = 0.3


def header(path):
    values = {}
    with open(path) as fh:
        for line in fh:
            if not line.startswith("# "):
                break
            key, _, value = line[2:].partition(" ")
            values[key] = value.split("#", 1)[0].strip()
    return values


def lambda_zero_cells(sweep):
    path = os.path.join(ROOT, sweep, "cells.tsv")
    with open(path) as fh:
        for line in fh:
            cell, n, lam, vy, vz, *_ = line.rstrip("\n").split("\t")
            if (
                float(lam) == 0.0
                and N_MIN <= int(n) <= N_MAX
                and V_MIN <= float(vy) <= V_MAX
                and V_MIN <= float(vz) <= V_MAX
            ):
                yield cell, int(n), float(vy), float(vz)


def load_sweep(sweep):
    selected = []
    for cell, n, vy, vz in lambda_zero_cells(sweep):
        profiles = []
        pattern = os.path.join(ROOT, sweep, "out", cell, "tight_t*.pitches")
        for path in glob.glob(pattern):
            h = header(path)
            if "certified" not in h:
                continue
            row = {
                "sweep": sweep,
                "cell": cell,
                "n": n,
                "vy": vy,
                "vz": vz,
                "dJ": float(h["dJ"].split()[0]),
                "v_end_y": float(h["v_end"].split()[0]),
                "v_end_z": float(h["v_end"].split()[1]),
                "structure": h.get("structure", ""),
                "file": os.path.basename(path),
            }
            profiles.append(row)
        if not profiles:
            raise RuntimeError(f"no certified profiles in {sweep}/{cell}")
        selected.append(max(profiles, key=lambda row: row["dJ"]))
    return sorted(selected, key=lambda row: row["n"])


def rate(row):
    return row["dJ"] / row["n"] * TICKS_PER_SECOND


def velocity_color(vy, vz):
    """Map vy to hue and vz to a deliberately wide luminance range."""
    y_frac = (vy - V_MIN) / (V_MAX - V_MIN)
    z_frac = (vz - V_MIN) / (V_MAX - V_MIN)
    hue = (215.0 - 190.0 * y_frac) / 360.0
    lightness = 0.30 + 0.50 * z_frac
    return colorsys.hls_to_rgb(hue, lightness, 0.90)


def write_selected(rows):
    path = os.path.join(OUT, "selected.csv")
    cols = (
        "sweep", "cell", "n", "vy", "vz", "dJ", "v_end_y", "v_end_z", "structure", "file"
    )
    with open(path, "w", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=cols)
        writer.writeheader()
        writer.writerows(rows)
    return path


def main():
    os.makedirs(OUT, exist_ok=True)
    selected = load_sweep(SWEEP)
    by_velocity = defaultdict(list)
    for row in selected:
        by_velocity[(row["vy"], row["vz"])].append(row)

    fig = plt.figure(figsize=(11.0, 6.0))
    grid = fig.add_gridspec(1, 2, width_ratios=(8.5, 1.5), wspace=0.18)
    ax = fig.add_subplot(grid[0, 0])
    key = fig.add_subplot(grid[0, 1])

    peak_guides = []
    for (vy, vz), rows in sorted(by_velocity.items()):
        rows.sort(key=lambda row: row["n"])
        color = velocity_color(vy, vz)
        peak = max(rows, key=rate)
        peak_guides.append((peak["n"], rate(peak), color))
        ax.plot(
            [row["n"] for row in rows],
            [rate(row) for row in rows],
            lw=1.35,
            color=color,
            alpha=0.82,
            zorder=3,
        )

    y_limits = ax.get_ylim()
    for peak_n, peak_rate, color in peak_guides:
        ax.vlines(
            peak_n,
            y_limits[0],
            peak_rate,
            lw=0.55,
            color=color,
            alpha=0.38,
            zorder=1,
        )
    ax.set_ylim(y_limits)

    ax.axhline(0, color="#8b949e", lw=0.8, alpha=0.7)
    ax.set_xlim(N_MIN, N_MAX)
    ax.set_xlabel("allocated horizon, ticks")
    ax.set_ylabel("20 dJ / num_ticks, blocks/s")
    ax.set_title("gain efficiency by initial velocity", fontsize=13)
    ax.grid(alpha=0.18)

    resolution = 160
    color_key = [
        [
            velocity_color(
                V_MIN + (V_MAX - V_MIN) * x / (resolution - 1),
                V_MIN + (V_MAX - V_MIN) * z / (resolution - 1),
            )
            for x in range(resolution)
        ]
        for z in range(resolution)
    ]
    key.imshow(
        color_key,
        origin="lower",
        extent=(V_MIN, V_MAX, V_MIN, V_MAX),
        aspect="equal",
        interpolation="bilinear",
    )
    velocity_values = (0.0, 0.1, 0.2, 0.3)
    key.set_xticks(velocity_values)
    key.set_yticks(velocity_values)
    key.tick_params(axis="x", labelsize=8, labelrotation=45)
    key.tick_params(axis="y", labelsize=8)
    key.set_xlabel(r"$v_{0,y}$", fontsize=10)
    key.set_ylabel(r"$v_{0,z}$", fontsize=10)
    key.set_title("line color", fontsize=10)
    key.grid(False)

    png = os.path.join(OUT, "gain-efficiency.png")
    svg = os.path.join(OUT, "gain-efficiency.svg")
    fig.savefig(png, dpi=180)
    fig.savefig(svg)
    selected_csv = write_selected(sorted(selected, key=lambda row: (row["vy"], row["vz"], row["n"])))
    print(f"wrote {png}")
    print(f"wrote {svg}")
    print(f"wrote {selected_csv}")


if __name__ == "__main__":
    main()
