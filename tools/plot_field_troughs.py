#!/usr/bin/env python3
"""The one-tick energy field, with its troughs, ridges and creases drawn as curves.

    python3 tools/plot_field_troughs.py [out.png] [--ridges] [--kinds] [--pitch=DEG]
        [--window=vz_lo,vz_hi,vy_lo,vy_hi] [--samples=N | --stretch --samples=Nz,Ny]
        [--color=energy|pitch] [--legend=lower_right]

The field is plot_field_replay's: G(v), the most total energy any pitch makes from velocity v
in one tick. Windows are in blocks/second; the default is elytra-vario's chart, vy [-30, 40] by
vz [-10, 60] (VarioConfig's [-1.5, 2] by [-0.5, 3] blocks/tick). README-field.md has the zooms.

A trough is a local minimum of G along the direction G curves most across. G is smooth in
some places and creased in others, and the two need different tests:

Smooth: grad G . e = 0, with e the Hessian eigenvector of largest |eigenvalue| and that
eigenvalue positive (negative for a ridge). Creased: G has kinks of three kinds -- a tie between
two maxima in pitch, conversion switching on under a fixed pitch, and vz = 0 -- and a crease is
a trough where G rises on *both* sides along its normal (a V), a ridge where it falls on both,
and neither where the slope jumps but keeps its sign (a kink with no extremum, drawn dotted).

All of it is computed by src/bin/field.rs (its header and README-field.md have the method),
loaded through field_data.py, which caches by window. This only draws.
"""

import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.colors
import matplotlib.lines
import matplotlib.patches
import matplotlib.patheffects
import matplotlib.pyplot as plt
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import field_data as fd
from plot_field_replay import (AXIS_A, FIELD_ZERO, GRID_A, PITCH_LIMIT, VX_HI, VX_LO, VY_HI,
                               VY_LO, field_rgb)

TPS = 20                 # ticks per second: the axes are in blocks/second
# elytra-vario's chart, (vz_lo, vz_hi, vy_lo, vy_hi) in blocks/tick.
CHART = (VX_LO, VX_HI, VY_LO, VY_HI)
TROUGH, RIDGE, CREASE = "#f5ae68", "#54c7e8", "#e6e9ed"
LIMIT_EDGE = "#e6e9ed"    # the outline of each hatched region
SMOOTH_LS = (0, (5, 3))
PITCH_INK = "#7ad151"     # pitch contours: not energy, so not the field's colors
# Best pitch as a background (--color=pitch): its own diverging pair through the near-black
# zero, chosen off the curves' orange and cyan, which a nose-down orange would swallow.
NOSE_UP, NOSE_DOWN = "#6f8cff", "#e0607e"
PITCH_CMAP = matplotlib.colors.LinearSegmentedColormap.from_list(
    "pitch", [NOSE_UP, FIELD_ZERO, NOSE_DOWN])
HALO = [matplotlib.patheffects.withStroke(linewidth=3.2, foreground=FIELD_ZERO, alpha=.8)]


def grid_step(span):
    """The coarsest 1-2-5 step that puts at least five gridlines across span."""
    steps = sorted((m * 10.0 ** e for e in range(-4, 4) for m in (1, 2, 5)), reverse=True)
    return next(s for s in steps if span / s >= 5)


def chrome(ax, title, window=None):
    """Grid, limits and labels, in blocks/second; window is in blocks/tick, default the chart."""
    zlo, zhi, ylo, yhi = (np.array(window or CHART) * TPS).tolist()
    ax.set_facecolor("#14171a")
    for lo, hi, line, ticks in ((zlo, zhi, ax.axvline, ax.set_xticks),
                                (ylo, yhi, ax.axhline, ax.set_yticks)):
        step = grid_step(hi - lo)
        at = np.arange(np.ceil(lo / step) * step, hi + 1e-9, step)
        for v in at:
            line(v, color="w", lw=.8, alpha=AXIS_A if abs(v) < 1e-9 else GRID_A, zorder=1)
        ticks(at)
    ax.set_xlim(zlo, zhi)
    ax.set_ylim(ylo, yhi)
    ax.set_xlabel("vz (blocks/second)", color="#e6e9ed")
    ax.set_ylabel("vy (blocks/second)", color="#e6e9ed")
    ax.tick_params(colors="#b9c0c9")
    for s in ax.spines.values():
        s.set_color("#4a525c")
    ax.set_title(title, color="#e6e9ed")


def draw_limits(ax, F):
    """Hatch where the best pitch is a corner: / for -89 (nose up), \\ for +89 (nose down),
    - for level. Exact equality is right: field.rs snaps a max at a corner onto it."""
    for p, hatch in ((-PITCH_LIMIT, "///"), (PITCH_LIMIT, "\\\\\\"), (0.0, "---")):
        at = (F.p1 == p).astype(float)
        ax.contourf(F.vz * TPS, F.vy * TPS, at, levels=[0.5, 1.5], colors="none",
                    hatches=[hatch], zorder=1.5)
        ax.contour(F.vz * TPS, F.vy * TPS, at, levels=[0.5], colors=LIMIT_EDGE, linewidths=.6,
                   alpha=.6, zorder=1.5)


def option(name, default=None):
    """The value of --name=value on the command line, or default."""
    for a in sys.argv[1:]:
        if a.startswith(f"--{name}="):
            return a.split("=", 1)[1]
    return default


def main():
    argv = [a for a in sys.argv[1:] if not a.startswith("--")]
    out = argv[0] if argv else "runs/atlas/fig/field_troughs.png"
    os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
    plt.rcParams["hatch.color"] = (1, 1, 1, 0.22)
    plt.rcParams["hatch.linewidth"] = 0.6
    ridges = "--ridges" in sys.argv
    # Draw each kind of trough (tie, conversion, smooth) in its own style.
    kinds = "--kinds" in sys.argv
    # Pitch contours every this many degrees where the best pitch is free (not on a corner).
    pitch_step = float(option("pitch", "0"))
    # Unequal scales: each axis gets its own resolution and the plot is not square in velocity.
    stretch = "--stretch" in sys.argv
    # The background: "energy" (the gain G, elytra-vario's colors) or "pitch" (the best pitch).
    by_pitch = option("color", "energy") == "pitch"

    window_bps = fd.CHART_BPS
    if option("window"):
        window_bps = tuple(float(x) for x in option("window").split(","))
    window = tuple(x / TPS for x in window_bps)
    span_z, span_y = window[1] - window[0], window[3] - window[2]
    samples = tuple(int(x) for x in option("samples", "1050").split(","))
    if len(samples) == 2 and not stretch:
        sys.exit("--samples=Nz,Ny needs --stretch: without it the grid is square")

    F = fd.load(window_bps, samples)
    width = 12.0
    height = width * (0.9 if stretch else span_y / span_z) + 0.8
    fig, ax = plt.subplots(figsize=(width, height))
    fig.patch.set_facecolor("#14171a")
    if by_pitch:
        im = ax.imshow(F.p1, extent=F.ext(), origin="lower", interpolation="nearest",
                       aspect="auto" if stretch else "equal", zorder=0, cmap=PITCH_CMAP,
                       vmin=-PITCH_LIMIT, vmax=PITCH_LIMIT)
        cb = fig.colorbar(im, ax=ax, fraction=.035, pad=.02)
        cb.set_label("best pitch (degrees, − is nose up)", color="#e6e9ed")
        cb.ax.tick_params(colors="#b9c0c9")
        cb.outline.set_edgecolor("#4a525c")
    else:
        ax.imshow(field_rgb(F.G), extent=F.ext(), origin="lower", interpolation="nearest",
                  aspect="auto" if stretch else "equal", zorder=0)
    chrome(ax, "", window)
    ax.contour(F.vz * TPS, F.vy * TPS, F.G, levels=[0.0], colors="w", linewidths=.8, alpha=.45,
               zorder=1)
    draw_limits(ax, F)
    if pitch_step:
        free = np.ma.masked_where(F.stuck, F.p1)
        levels = np.arange(-90 + pitch_step, 90, pitch_step)
        cs = ax.contour(F.vz * TPS, F.vy * TPS, free, levels=levels, colors=PITCH_INK,
                        linewidths=.6, alpha=.7, zorder=2)
        ax.clabel(cs, fmt="%g°", fontsize=8, colors=PITCH_INK)
        ax.contour(F.vz * TPS, F.vy * TPS, (~F.stuck).astype(float), levels=[0.5],
                   colors=PITCH_INK, linewidths=1.0, zorder=2)

    # On the pitch map the curves cross both ends of a bright ramp; outline them.
    halo = HALO if by_pitch else None
    counts = {}
    for q, cls, kind in F.curves:
        counts[(kind, cls)] = counts.get((kind, cls), 0) + len(q)
        x, y = q[:, 0] * TPS, q[:, 1] * TPS
        if cls == 1:
            ls = "-"
            if kinds and kind == "conversion":
                ls = (0, (6, 2, 1, 2))
            elif kinds and kind == "smooth":
                ls = SMOOTH_LS
            ax.plot(x, y, color=TROUGH, lw=1.8, ls=ls, zorder=4, path_effects=halo)
        elif cls == -1 and ridges:
            ax.plot(x, y, color=RIDGE, lw=1.4, ls="--", zorder=3, path_effects=halo)
        elif cls == 0:
            ax.plot(x, y, color=CREASE, lw=1.0, ls=(0, (1, 2)), alpha=.8, zorder=3,
                    path_effects=halo)
    print("curve points by (kind, class):", dict(sorted(counts.items(), key=str)))

    line = matplotlib.lines.Line2D
    handles = [
        *([line([], [], color=TROUGH, lw=1.8, label="trough: tie (best pitch jumps)"),
           line([], [], color=TROUGH, lw=1.8, ls=(0, (6, 2, 1, 2)),
                label="trough: conversion switches on under a fixed pitch"),
           line([], [], color=TROUGH, lw=1.8, ls=SMOOTH_LS, label="trough: smooth valley")]
          if kinds else [line([], [], color=TROUGH, lw=1.8, label="trough")]),
        *([line([], [], color=RIDGE, lw=1.4, ls="--", label="ridge")] if ridges else []),
        line([], [], color=CREASE, lw=1.0, ls=(0, (1, 2)),
             label="kink with no extremum: slope changes, same sign both sides"),
        line([], [], color="w", lw=.8, alpha=.6, label="zero gain"),
        *([line([], [], color=PITCH_INK, lw=1.0,
                label=f"edge of free best pitch; contours every {pitch_step:g}°")]
          if pitch_step else []),
        matplotlib.patches.Patch(facecolor="none", hatch="///", edgecolor=(1, 1, 1, .4),
                                 label="best pitch −89 (nose-up limit)"),
        matplotlib.patches.Patch(facecolor="none", hatch="\\\\\\", edgecolor=(1, 1, 1, .4),
                                 label="best pitch +89 (nose-down limit)"),
        # Denser than the region's: three lines in a swatch this small land on its border.
        matplotlib.patches.Patch(facecolor="none", hatch="-----", edgecolor=(1, 1, 1, .4),
                                 label="best pitch 0 (level)"),
    ]
    # Underscores for spaces, so a shell needs no quoting: --legend=lower_right.
    loc = option("legend", "upper_left").replace("_", " ")
    leg = ax.legend(handles=handles, loc=loc, facecolor="#1b1f24", edgecolor="#4a525c",
                    labelcolor="#e6e9ed", fontsize=9, framealpha=.92)
    leg.set_zorder(10)
    title = ("best pitch for one-tick energy" if by_pitch
             else "one-tick energy field (best pitch)")
    if stretch:
        title += "  ·  axes stretched: vy and vz scales differ"
    ax.set_title(title, color="#e6e9ed")
    fig.savefig(out, dpi=110, bbox_inches="tight", facecolor=fig.get_facecolor())
    print(out)


if __name__ == "__main__":
    main()
