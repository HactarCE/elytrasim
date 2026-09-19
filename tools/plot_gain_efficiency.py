#!/usr/bin/env python3
"""Plot gain rate against allocated horizon for the Atlas lambda=0 sweeps.

This deliberately does not read ``best.csv``.  It reads every certified
``tight_t*.pitches`` profile in the lambda=0 cells, then chooses the largest
dJ in each cell.  The dy panel reports that same profile, so the two panels
compare two measurements of one selected schedule rather than two different
post-hoc optima.

Usage:
    python3 tools/plot_gain_efficiency.py [OUT_DIR]
"""

import csv
import glob
import os
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt


plt.style.use("dark_background")


ROOT = "runs/atlas"
OUT = sys.argv[1] if len(sys.argv) > 1 else f"{ROOT}/fig/efficiency"
TICKS_PER_SECOND = 20.0
SWEEPS = (
    ("mapsweep", "v0 = (0, 0.4)", "#1f77b4"),
    ("nsweep", "v0 = (0, 0)", "#d97706"),
)


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
            if float(lam) == 0.0:
                yield cell, int(n), float(vy), float(vz)


def load_sweep(sweep):
    selected, candidates = [], []
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
                "dy": float(h["dy"].split()[0]),
                "structure": h.get("structure", ""),
                "file": os.path.basename(path),
            }
            profiles.append(row)
            candidates.append(row)
        if not profiles:
            raise RuntimeError(f"no certified profiles in {sweep}/{cell}")
        selected.append(max(profiles, key=lambda row: row["dJ"]))
    return sorted(selected, key=lambda row: row["n"]), candidates


def rate(row, metric):
    return row[metric] / row["n"] * TICKS_PER_SECOND


def write_selected(rows):
    path = os.path.join(OUT, "selected.csv")
    cols = ("sweep", "cell", "n", "vy", "vz", "dJ", "dy", "structure", "file")
    with open(path, "w", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=cols)
        writer.writeheader()
        writer.writerows(rows)
    return path


def main():
    os.makedirs(OUT, exist_ok=True)
    loaded = [(name, label, color, *load_sweep(name)) for name, label, color in SWEEPS]

    fig, ax = plt.subplots(figsize=(9.2, 5.8))
    for _, label, color, selected, _ in loaded:
        ticks = [row["n"] for row in selected]
        dJ_peak = max(selected, key=lambda row: rate(row, "dJ"))
        dy_peak = max(selected, key=lambda row: rate(row, "dy"))
        ax.axvline(dJ_peak["n"], lw=0.8, ls="-", color=color, alpha=0.55, zorder=1)
        ax.axvline(dy_peak["n"], lw=0.8, ls="--", color=color, alpha=0.55, zorder=1)
        ax.plot(ticks, [rate(row, "dJ") for row in selected],
                lw=2.0, color=color, label=f"{label}  dJ", zorder=3)
        ax.plot(ticks, [rate(row, "dy") for row in selected],
                lw=1.8, ls="--", color=color, label=f"{label}  dy", zorder=3)

    ax.axhline(0, color="#8b949e", lw=0.8, alpha=0.7)
    ax.set_xlabel("allocated horizon, ticks")
    ax.set_ylabel("gain rate, blocks/s")
    ax.set_title("gain efficiency: dJ / num_ticks and dy / num_ticks", fontsize=13)
    ax.grid(alpha=0.18)
    ax.legend(frameon=False, fontsize=9, ncol=2, loc="lower right")
    fig.tight_layout()

    png = os.path.join(OUT, "gain-efficiency.png")
    svg = os.path.join(OUT, "gain-efficiency.svg")
    fig.savefig(png, dpi=180)
    fig.savefig(svg)
    selected_csv = write_selected([row for *_, selected, _ in loaded for row in selected])
    print(f"wrote {png}")
    print(f"wrote {svg}")
    print(f"wrote {selected_csv}")


if __name__ == "__main__":
    main()
