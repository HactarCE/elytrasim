#!/usr/bin/env python3
"""Derivatives of the one-tick energy field G, over elytra-vario's chart, as small multiples.

    python3 tools/plot_field_gradient.py [out.png] [--cycle]

Panels: the two partials of G, |grad G| with its streamlines, the best pitch the gradient is
taken at, the Laplacian (= div grad G), the curl of grad G, det H, and the Hessian eigenvalue of
largest magnitude (how sharply G curves across its steepest-bending direction). `--cycle` draws
the reference cycle on every panel.

Every derivative is exact at its sample, from src/bin/field.rs's envelope formulas (loaded via
field_data.py), except the curl: that one is the grid curl of the exact gradient field. A
gradient has no curl by definition, so the panel is a check on the derivatives, computed a
different way; it should be blank but for the creases.

The creases (white) are where G has a kink, and every second derivative there is a line of
infinite density that no pixel can show: the Laplacian's smooth part is drawn, its crease part
is the white line, with the troughs among them in orange. Units are per block/second, since
the axes are: grad G in blocks per (b/s), second derivatives in blocks per (b/s)^2.
"""

import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.colors as mcolors
import matplotlib.pyplot as plt
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import field_data as fd
import plot_field_troughs as ft
from plot_field_replay import FIELD_GAIN, FIELD_LOSS, FIELD_ZERO, PITCH_LIMIT

TPS = ft.TPS
BG, INK, MUTED, EDGE = "#14171a", "#e6e9ed", "#b9c0c9", "#4a525c"


def signed_norm(x, q=75, top=99.5):
    """The field's own compression, |x|/(|x|+s), with s this panel's q-th percentile of |x|.

    The ends are the top-th percentile, not the max: the Hessian has rare huge values where a
    pitch branch is born (g_pp -> 0 in the envelope correction), and scaling to them would
    give the whole panel to a handful of pixels.
    """
    s = float(np.nanpercentile(np.abs(x), q)) or 1.0
    m = float(np.nanpercentile(np.abs(x), top)) or 1.0
    fwd = lambda v: np.asarray(v) / (np.abs(v) + s)
    inv = lambda c: s * np.asarray(c) / (1 - np.abs(c))
    return mcolors.FuncNorm((fwd, inv), vmin=-m, vmax=m)


DIVERGING = mcolors.LinearSegmentedColormap.from_list("vario", [FIELD_LOSS, FIELD_ZERO, FIELD_GAIN])
PITCH = ft.PITCH_CMAP     # shared with plot_field_troughs --color=pitch
SEQ = mcolors.LinearSegmentedColormap.from_list("seq", [FIELD_ZERO, "#8fd3ea"])


def main():
    argv = [a for a in sys.argv[1:] if not a.startswith("--")]
    out = argv[0] if argv else "runs/atlas/fig/field_gradient.png"
    os.makedirs(os.path.dirname(out) or ".", exist_ok=True)

    F = fd.load()
    cyc = fd.cycle() if "--cycle" in sys.argv else None
    creases = [c for c in F.curves if c[2] != "smooth"]
    s1, s2 = 1 / TPS, 1 / TPS ** 2                      # per block/tick -> per block/second
    gz, gy = F.gz * s1, F.gy * s1
    hzz, hyz, hyy = F.hzz * s2, F.hyz * s2, F.hyy * s2
    lap = hzz + hyy
    det = hzz * hyy - hyz * hyz
    mean, half = (hzz + hyy) / 2, np.hypot((hzz - hyy) / 2, hyz)
    major = np.where(np.abs(mean + half) >= np.abs(mean - half), mean + half, mean - half)
    h_bps = F.hz * TPS   # square grid here: hz == hy
    curl = np.gradient(gy, h_bps, axis=1) - np.gradient(gz, h_bps, axis=0)
    # Away from creases, the curl measured against the size of the second derivatives.
    near = np.zeros(F.p1.shape, bool)
    for q, _, _ in creases:
        r, c = F.nearest(q)
        for dr in (-2, -1, 0, 1, 2):
            for dc in (-2, -1, 0, 1, 2):
                near[np.clip(r + dr, 0, near.shape[0] - 1),
                     np.clip(c + dc, 0, near.shape[1] - 1)] = True
    near |= F.jump
    view = ((F.Z >= ft.VX_LO) & (F.Z <= ft.VX_HI) & (F.Y >= ft.VY_LO) & (F.Y <= ft.VY_HI))
    # Per pixel, against that pixel's own |H|, so one huge Hessian elsewhere cannot hide it.
    local = np.abs(curl) / np.maximum(np.sqrt(hzz ** 2 + 2 * hyz ** 2 + hyy ** 2), 1e-300)
    rel = np.percentile(local[view & ~near], [50, 99])
    print(f"|curl| / |H| per pixel off the creases: median {rel[0]:.1e}, 99th {rel[1]:.1e}")

    panels = [
        ("∂G/∂vz   blocks per b/s", gz, DIVERGING, signed_norm(gz)),
        ("∂G/∂vy   blocks per b/s", gy, DIVERGING, signed_norm(gy)),
        ("|∇G| and its direction   blocks per b/s", np.hypot(gz, gy), SEQ,
         mcolors.PowerNorm(0.5, vmin=0, vmax=float(np.percentile(np.hypot(gz, gy)[view], 99.5)))),
        ("best pitch p*   degrees, − is nose up", F.p1, PITCH,
         mcolors.Normalize(-PITCH_LIMIT, PITCH_LIMIT)),
        ("Laplacian = div ∇G   blocks per (b/s)²", lap, DIVERGING, signed_norm(lap)),
        ("curl ∇G (a check: should be 0)   scale of the Laplacian", curl, DIVERGING,
         signed_norm(lap)),
        ("det H   bowl/dome (+) vs saddle (−)", det, DIVERGING, signed_norm(det)),
        ("largest-|·| Hessian eigenvalue   blocks per (b/s)²", major, DIVERGING, signed_norm(major)),
    ]
    fig, axes = plt.subplots(2, 4, figsize=(26, 14.5))
    fig.patch.set_facecolor(BG)
    ext = F.ext()
    for ax, (title, data, cmap, norm) in zip(axes.flat, panels):
        im = ax.imshow(data, extent=ext, origin="lower", interpolation="nearest", aspect="equal",
                       cmap=cmap, norm=norm, zorder=0)
        ft.chrome(ax, title)
        ax.title.set_fontsize(11)
        for q, cls, _ in creases:
            ax.plot(q[:, 0] * TPS, q[:, 1] * TPS, lw=1.3 if cls == 1 else .7,
                    color=ft.TROUGH if cls == 1 else "w", alpha=1 if cls == 1 else .55, zorder=3)
        if cyc is not None:
            ft.draw_cycle(ax, *cyc, lw=1.3)
        cb = fig.colorbar(im, ax=ax, fraction=.046, pad=.02)
        cb.ax.tick_params(colors=MUTED, labelsize=8)
        cb.outline.set_edgecolor(EDGE)
    # Streamlines of grad G: the direction of steepest gain, on a coarser copy of the grid.
    k = 6
    axes.flat[2].streamplot(F.vz[::k] * TPS, F.vy[::k] * TPS, gz[::k, ::k], gy[::k, ::k],
                            color=(1, 1, 1, .55), linewidth=.6, density=1.6, arrowsize=.7, zorder=2)
    axes.flat[5].text(.03, .03, f"|curl| / |H| off the creases: median {rel[0]:.0e}, "
                      f"99th pct {rel[1]:.0e}",
                      transform=axes.flat[5].transAxes, color=INK, fontsize=10, zorder=5,
                      bbox=dict(facecolor="#1b1f24", edgecolor=EDGE))
    fig.suptitle("one-tick energy field G: derivatives  ·  white = creases (G kinks), "
                 "orange = the creases that are troughs"
                 + ("  ·  green = the reference cycle, dot every "
                    f"{ft.CYCLE_DOT_EVERY} ticks, ring at tick 0" if cyc is not None else ""),
                 color=INK, fontsize=14)
    fig.tight_layout(rect=(0, 0, 1, .97))
    fig.savefig(out, dpi=90, facecolor=BG)
    print(out)


if __name__ == "__main__":
    main()
