#!/usr/bin/env python3
"""The figures of README-atlas.md. Reads runs/atlas directly, so they regenerate.

    python3 tools/plot_atlas.py [outdir]        default runs/atlas/fig

Pitch axes are inverted throughout, matching tools/plot_pitch_grid.py: positive pitch is nose
down, so an inverted axis puts nose-up at the top where a reader expects it.

Dark throughout. Set ATLAS_LIGHT=1 for the light versions.
"""

import glob
import json
import os
import re
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.lines
import matplotlib.pyplot as plt

CELL = "runs/atlas/cells/v00_n300_lam0"
FLICK = "runs/atlas/cells/flick_v00_n300"

# Not pure black: these are thin-line plots, and a 1px viridis line on #000 loses its low end
# entirely. A dark slate keeps the dark end of the colormap distinguishable from the ground.
DARK = not os.environ.get("ATLAS_LIGHT")
RULE = "0.55" if DARK else "0.35"     # zero line, turnpike line -- reference, not data
GUIDE = "0.55" if DARK else "0.6"     # the no-drift diagonal


def theme():
    if not DARK:
        return
    plt.style.use("dark_background")
    plt.rcParams.update({
        "figure.facecolor": "#14171a", "savefig.facecolor": "#14171a",
        "axes.facecolor": "#1b1f24", "axes.edgecolor": "#4a525c",
        "grid.color": "#5d6774", "text.color": "#e6e9ed",
        "axes.labelcolor": "#e6e9ed", "xtick.color": "#b9c0c9", "ytick.color": "#b9c0c9",
        "legend.facecolor": "#1b1f24", "legend.edgecolor": "#4a525c", "legend.framealpha": .9,
    })


def load(path):
    """(header dict, pitches) for a certified profile, or None if it never finished."""
    text = open(path).read()
    if "# certified" not in text:
        return None
    head = dict(re.findall(r"^# (\w+)\s+(\S+)", text, re.M))
    pitches = [float(x) for line in text.splitlines() for x in line.split("#")[0].split()]
    return head, pitches


def flick_tick(p):
    return min(range(len(p)), key=lambda i: p[i])


def hold0(p):
    """Length of the flat run immediately before the flick. 0 when there is no flick."""
    f = flick_tick(p)
    if p[f] > -30.0:
        return 0
    hi = f
    while hi > 0 and abs(p[hi]) > 1.0:
        hi -= 1
    hi += 1
    lo = hi - 1
    while lo > 0 and abs(p[lo - 1]) <= 1.0:
        lo -= 1
    return hi - lo


def fig_landscape(out):
    """Every optimum at one cell, by flick time and value, colored by the family that found it."""
    rows = []
    for f in sorted(glob.glob(f"{CELL}/*.pitches")):
        got = load(f)
        if not got:
            continue
        head, p = got
        fam = re.match(r"[a-z]+", os.path.basename(f)).group(0)
        rows.append((fam, float(head["dJ"]), flick_tick(p), p))
    fams = sorted({r[0] for r in rows})
    # tab20 alternates a saturated color with a pastel of the same hue, and the pastels wash out
    # against the dark ground. Take all ten saturated slots first, pastels only if we run out.
    cmap = plt.get_cmap("tab20")
    slots = list(range(0, 20, 2)) + list(range(1, 20, 2))
    fig, ax = plt.subplots(1, 2, figsize=(14, 5))
    for i, fam in enumerate(fams):
        sel = [r for r in rows if r[0] == fam]
        ax[0].scatter([r[2] for r in sel], [r[1] for r in sel], s=22, alpha=.85,
                      color=cmap(slots[i % 20]), edgecolor="none", label=fam)
    ax[0].set_xlabel("tick of the most nose-up pitch")
    ax[0].set_ylabel("dJ, blocks")
    ax[0].set_title(f"{len(rows)} converged optima at v0=0, n=300: two strategies, 42 blocks apart")
    ax[0].legend(fontsize=7, ncol=2)
    ax[0].grid(alpha=.3)
    best = max(rows, key=lambda r: r[1])
    worst = min(rows, key=lambda r: r[1])
    for r, lab, c in ((best, f"best cyclic, dJ {best[1]:.2f}", "tab:blue"),
                      (worst, f"the turnpike glide, dJ {worst[1]:.2f}", "tab:green")):
        ax[1].plot(r[3], lw=1.4, color=c, label=lab)
    ax[1].axhline(-13.052, color=RULE, ls=":", lw=1)
    ax[1].text(4, -13.6, "min-sink turnpike, -13.052 deg", fontsize=8, va="bottom")
    ax[1].invert_yaxis()
    ax[1].set_xlabel("tick")
    ax[1].set_ylabel("pitch, deg")
    ax[1].set_title("the two strategies")
    ax[1].legend(fontsize=8, loc="lower left")
    ax[1].grid(alpha=.3)
    plt.tight_layout()
    plt.savefig(f"{out}/01_landscape.png", dpi=110)
    plt.close()


def fig_ridge(out):
    """Is the flick-time family connected? Seed flick against converged flick, and J*(t)."""
    seeds = json.load(open("runs/atlas/seeds/flick300/seeds.json"))["seeds"]
    rows = []
    for f in sorted(glob.glob(f"{FLICK}/tight_t*.pitches")):
        got = load(f)
        if not got:
            continue
        head, p = got
        name = os.path.basename(f)[:-8]
        rows.append((seeds[name]["flick_tick"], flick_tick(p), float(head["dJ"])))
    rows.sort()
    fig, ax = plt.subplots(1, 2, figsize=(13, 4.6))
    ax[0].plot([r[0] for r in rows], [r[1] for r in rows], "o", ms=4, color="tab:blue")
    ax[0].plot([r[0] for r in rows], [r[0] for r in rows], "--", lw=1, color=GUIDE,
               label="no drift")
    ax[0].set_xlabel("seed flick tick")
    ax[0].set_ylabel("converged flick tick")
    ax[0].set_title("a connected ridge with walls, not separate basins")
    ax[0].legend(fontsize=8)
    ax[0].grid(alpha=.3)
    cyc = [r for r in rows if r[2] > 0]
    ax[1].plot([r[1] for r in cyc], [r[2] for r in cyc], "o", ms=4, color="tab:red")
    ax[1].set_xlabel("converged flick tick")
    ax[1].set_ylabel("dJ, blocks")
    ax[1].set_title("J*(t): the cost of choosing a different flick time")
    ax[1].grid(alpha=.3)
    plt.tight_layout()
    plt.savefig(f"{out}/02_ridge.png", dpi=110)
    plt.close()


def fig_holdzero(out, sweeps):
    """Value against robustness as the hold-0 length varies, at two starting velocities."""
    fig, ax = plt.subplots(1, 3, figsize=(16, 4.6))
    for (lab, rows, c) in sweeps:
        ax[0].plot([r[0] for r in rows], [r[1] for r in rows], "o-", ms=3, color=c, label=lab)
        ax[1].plot([r[0] for r in rows], [r[2] for r in rows], "o-", ms=3, color=c, label=lab)
        ax[2].plot([r[1] for r in rows], [r[2] for r in rows], "o-", ms=3, color=c, label=lab)
    ax[0].set_xlabel("hold-0 length, ticks")
    ax[0].set_ylabel("dJ, blocks")
    ax[0].set_title("value peaks at 13")
    ax[1].set_xlabel("hold-0 length, ticks")
    ax[1].set_ylabel("correlated tremor, 5th pct loss")
    ax[1].set_title("robustness peaks at 5, and is not monotone")
    ax[2].set_xlabel("dJ, blocks")
    ax[2].set_ylabel("correlated tremor, 5th pct loss")
    ax[2].set_title("the frontier between them")
    for a in ax:
        a.grid(alpha=.3)
        a.legend(fontsize=8)
    plt.tight_layout()
    plt.savefig(f"{out}/03_hold0.png", dpi=110)
    plt.close()


def read_human(path, pos):
    """Parse an examples/human.rs table into (key, dJ, tremor p05) rows."""
    lines = open(path).read().splitlines()
    head = [t for t in lines[0].split() if t != "|"]
    rows = []
    for line in lines[1:]:
        if ".pitches" not in line:
            continue
        tok = [t for t in line.split() if t != "|"]
        d = dict(zip(head[2:], tok[2:]))
        rows.append((int(tok[0][pos:pos + 2]), float(tok[1]), float(d["trP1"])))
    return sorted(rows)


MODES = ("COLLAPSED", "MULTICYCLE", "cyclic")   # the `structure` header, worst mode first
# autumn runs red -> yellow, and its yellow end is indistinguishable from viridis's. Stop at
# orange so the two classes stay separable no matter what their values are.
RAMP = {"COLLAPSED": ("gray", .42, .82), "MULTICYCLE": ("autumn", 0., .45),
        "cyclic": ("viridis", 0., 1.)}
# Below TOL two profiles are the same optimum found twice, not two optima -- it is the same
# threshold the band split uses. A class whose entire spread is under it gets one flat color,
# because ramping across it draws the solver's own residual as if it were structure. It must NOT
# be larger than that: at 0.5 blocks this rule flattened the whole 68-profile cyclic mode at
# n=150, whose spread is 0.19 blocks -- twenty times the worst certification residual in the
# catalog, and the only thing worth seeing in that cell.
TOL = 0.05


def mode_scales(rows):
    """A separate color scale per structure class, so one mode cannot eat the whole range.

    At the reference cell the glide sits 36 blocks below the ridge. Under a single linear norm
    that gap takes 87% of the colormap and the 123 ridge profiles -- the part worth looking at --
    all land in the top eighth as the same yellow. Scaling within each class fixes that. A class
    whose entire spread is under TOL gets a flat color rather than a ramp over nothing: the 140
    reference glides agree to 0.013 blocks, and ramping them would draw noise as structure.

    Returns {class: (colorfn, legend text, colorbar mappable or None)}.
    """
    out = {}
    for cls in MODES:
        djs = [r[0] for r in rows if r[2] == cls]
        if not djs:
            continue
        lo, hi = min(djs), max(djs)
        name, a, b = RAMP[cls]
        cmap = plt.get_cmap(name)
        if hi - lo < TOL:
            c = cmap((a + b) / 2)
            out[cls] = (lambda dj, c=c: c,
                        f"{cls.lower()}, n={len(djs)}, dJ {lo:+.2f}"
                        + (f" to {hi:+.2f}" if hi > lo else ""), None)
        else:
            norm = matplotlib.colors.Normalize(vmin=lo, vmax=hi)
            sub = matplotlib.colors.LinearSegmentedColormap.from_list(
                cls, cmap(__import__("numpy").linspace(a, b, 256)))
            out[cls] = (lambda dj, n=norm, m=sub: m(n(dj)),
                        f"{cls.lower()}, n={len(djs)}",
                        matplotlib.cm.ScalarMappable(norm=norm, cmap=sub))
    return out


def fig_profiles(outdir, cell):
    """Every converged schedule at one cell, drawn as a schedule rather than a point.

    Figure 01 reduces each profile to (flick tick, dJ); this keeps the whole curve, which is the
    only way to see that the value modes are also *shapes*. Colored by dJ within each structure
    class -- see mode_scales -- on scales local to the cell, since the lam != 0 cells price z
    differently and their dJ is not comparable to lam = 0's.
    """
    tag = os.path.basename(cell.rstrip("/"))
    # A cell run to a small pass budget holds partial optima, not converged ones, and saying
    # "converged" over them would be false in exactly the way a stale figure is false. The budget
    # is recorded in the cell's own log, so read it rather than inferring from the directory name.
    passes = None
    log = os.path.join(cell, "atlas.log")
    if os.path.exists(log):
        m = re.search(r"^# opts\s+--passes\s+(\d+)", open(log).read(), re.M)
        if m and int(m.group(1)) < 100:
            passes = int(m.group(1))
    rows = []
    for f in sorted(glob.glob(f"{cell}/*.pitches")):
        got = load(f)
        if not got:
            continue
        head, p = got
        rows.append((float(head["dJ"]), os.path.basename(f)[:-8], head["structure"], p))
    if not rows:
        print(f"  {tag}: no certified profiles, skipping")
        return
    rows.sort(key=lambda r: r[0])  # ascending dJ, so the good ones are drawn last, on top
    lo, hi = rows[0][0], rows[-1][0]
    scales = mode_scales(rows)
    col = lambda r: scales[r[2]][0](r[0])

    fig, ax = plt.subplots(figsize=(13.5, 5.5))
    for r in rows:
        ax.plot(r[3], lw=.7, alpha=.5, color=col(r))
    ax.axhline(0, color=RULE, lw=.8)
    ax.invert_yaxis()
    ax.set_xlabel("tick")
    ax.set_ylabel("pitch, deg  (nose-up at top)")
    ax.set_title(f"{tag}: all {len(rows)} schedules, " + (
        f"each stopped after {passes} coordinate passes -- partial optima, not converged"
        if passes else "converged"))
    ax.grid(alpha=.3)
    ax.legend(handles=[matplotlib.lines.Line2D([], [], color=scales[c][0](
        [r[0] for r in rows if r[2] == c][-1]), lw=2.5, label=scales[c][1])
        for c in MODES if c in scales], fontsize=8, loc="lower left")
    for cls in MODES:
        if cls in scales and scales[cls][2] is not None:
            fig.colorbar(scales[cls][2], ax=ax, label=f"dJ, blocks -- {cls.lower()}", pad=.015)
    plt.tight_layout()
    plt.savefig(f"{outdir}/{tag}_overlay.png", dpi=110)
    plt.close()

    # Small multiples, one band per equal share of *distinct* optima. Equal width would leave most
    # panels empty: the distribution is bimodal. Equal count is no better, because one mode is a
    # spike -- at the reference cell 140 of 264 profiles are the same glide within 0.013 blocks,
    # and equal count spent six of twelve panels redrawing it. So single-link the dJ values at
    # TOL first and split the *levels* evenly; a 140-fold duplicate then costs one panel, not six.
    tol = TOL
    levels = [[rows[0]]]
    for r in rows[1:]:
        (levels[-1] if r[0] - levels[-1][-1][0] < tol else levels.append([]) or levels[-1]).append(r)
    nb = min(12, len(levels))
    edges = sorted({round(i * len(levels) / nb) for i in range(nb + 1)})
    bands = [[r for lv in levels[a:b] for r in lv] for a, b in zip(edges, edges[1:])]
    ncol = 4
    nrow = -(-len(bands) // ncol)
    fig, axes = plt.subplots(nrow, ncol, figsize=(4 * ncol, 3 * nrow), squeeze=False,
                             sharex=True, sharey=True)
    flat = axes.ravel()
    for a in flat[len(bands):]:
        a.axis("off")
    for band, a in zip(bands, flat):
        for r in band:
            a.plot(r[3], lw=.8, alpha=.75, color=col(r))
        a.axhline(0, color=RULE, lw=.8)
        a.set_title(f"dJ {band[0][0]:+.2f} to {band[-1][0]:+.2f}   (n = {len(band)})", fontsize=9)
        a.grid(alpha=.3)
    flat[0].invert_yaxis()
    for a in flat:
        if a.axison:
            a.set_xlabel("tick")
    for a in axes[:, 0]:
        a.set_ylabel("pitch, deg")
    fig.suptitle(f"{tag}: the same {len(rows)} schedules, split evenly over distinct dJ levels "
                 f"(worst {lo:+.2f} to best {hi:+.2f})"
                 + (f"   [stopped after {passes} passes]" if passes else ""))
    plt.tight_layout()
    plt.savefig(f"{outdir}/{tag}_bands.png", dpi=110)
    plt.close()
    counts = "  ".join(f"{c} {sum(1 for r in rows if r[2] == c)}" for c in MODES
                       if any(r[2] == c for r in rows))
    print(f"  {tag}: {len(rows)} profiles, dJ {lo:+.2f} .. {hi:+.2f}   [{counts}]")


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "runs/atlas/fig"
    os.makedirs(out, exist_ok=True)
    theme()
    fig_landscape(out)
    fig_ridge(out)
    prof = os.path.join(out, "profiles")
    os.makedirs(prof, exist_ok=True)
    for cell in sorted(glob.glob("runs/atlas/cells/*")):
        if os.path.isdir(cell) and not os.path.basename(cell).startswith("flickpin"):
            fig_profiles(prof, cell)
    # The hold-0 sweeps are produced outside this script (examples/human.rs), so they live as
    # copied-in tables under runs/atlas/hold0. They used to be read from a session scratchpad via
    # ATLAS_SCRATCH, which meant that once the scratchpad went away this figure silently kept its
    # last render -- and a stale PNG on disk is indistinguishable from a fresh one. Warn loudly
    # and delete the stale file rather than leave a lie in the directory.
    scratch = os.environ.get("ATLAS_SCRATCH", "runs/atlas/hold0")
    sweeps = []
    for name, tag, c in (("hd_b.txt", "v0 = (0, 0)", "tab:blue"),
                         ("h_rep2.txt", "v0 = (0.2, 0.2)", "tab:orange")):
        path = os.path.join(scratch, name)
        if scratch and os.path.exists(path):
            sweeps.append((tag, read_human(path, 3), c))
        else:
            print(f"  WARNING hold-0 sweep {name} not found under {scratch}")
    stale = f"{out}/03_hold0.png"
    if sweeps:
        fig_holdzero(out, sweeps)
    elif os.path.exists(stale):
        os.remove(stale)
        print(f"  WARNING no hold-0 data; removed stale {stale} rather than leave it")
    print(f"wrote figures to {out}/")
    for f in sorted(glob.glob(f"{out}/*.png") + glob.glob(f"{out}/profiles/*.png")):
        print(f"  {os.path.relpath(f, out)}  {os.path.getsize(f) // 1024} KB")


if __name__ == "__main__":
    main()
