#!/usr/bin/env python3
"""Steady-state polish against the single-cycle optimum: schedules, and what the swap buys.

Each row is one cell. The two schedules are optimal for different ``v0`` -- the single-cycle
one for the cell's stated ``v0``, the steady one for its own fixed point -- so the panel plots
them on a common tick axis and the numbers score each at *its own* ``v0``, one lap.

y is inverted so up on the page is nose up, as elsewhere in this repo.

Usage:
    python3 tools/plot_steady.py ROWS_DIR CURVE_DIR [OUT.svg]
"""
import csv, glob, os, sys

ROWS = sys.argv[1]
CURVES = sys.argv[2]
OUT = sys.argv[3] if len(sys.argv) > 3 else "runs/steady/steady.svg"

# GitHub-dark, as in tools/plot_profiles.py. The two series steps are validated for the
# categorical checks against this surface (lightness band, chroma, CVD, contrast).
BG, FG, DIM, GRID = '#0d1117', '#e6edf3', '#8b949e', '#21262d'
BASE, STEADY = '#388bfd', '#db6d28'
CTRL = DIM   # a control, not a peer series: never drawn, only tabulated

PW, PH, AXW, LAB, GAPY, TOP = 620, 104, 40, 132, 26, 140
NUMW = 300


def rows():
    out = []
    for f in sorted(glob.glob(os.path.join(ROWS, "*.csv"))):
        with open(f) as fh:
            for r in csv.DictReader(fh):
                out.append(r)
    # n sweep first (lambda 0), then the lambda family at n=300
    out.sort(key=lambda r: (float(r["lambda"]) != 0.0, int(r["n"]), float(r["lambda"])))
    return out


def curve(stem, tag):
    p = os.path.join(CURVES, f"{stem}.{tag}.csv")
    if not os.path.exists(p):
        return None
    with open(p) as fh:
        return [float(r["pitch"]) for r in csv.DictReader(fh)]


R = rows()
if not R:
    sys.exit(f"no rows in {ROWS}")

GH = 210          # headline panel height
W = LAB + PW + NUMW + 28
H = TOP + GH + 52 + len(R) * (PH + GAPY) + 40
o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
     f'font-family="ui-monospace,SFMono-Regular,Menlo,monospace" font-size="10">',
     f'<rect width="{W}" height="{H}" fill="{BG}"/>',
     f'<text x="16" y="24" fill="{FG}" font-size="15" font-weight="600">'
     f'steady-state polish vs the single-cycle optimum</text>',
     f'<text x="16" y="42" fill="{DIM}">runs/atlas/nsweep, lambda 0, v0 (0,0), jitter 0, '
     f'mu 1e-4, limit 85. Same seed and same 200-pass budget for every variant.</text>',
     f'<text x="16" y="56" fill="{DIM}">&#8220;single-cycle&#8221; is the atlas profile, optimal '
     f'for the cell&#8217;s stated v0. &#8220;steady&#8221; re-solves v0 to the schedule&#8217;s '
     f'own fixed point after each pass.</text>',
     f'<text x="16" y="70" fill="{DIM}">&#8220;control&#8221; is that same seed re-polished with '
     f'steady off, so a gain cannot be mistaken for extra passes. It lands within 0.002 blocks '
     f'of the seed everywhere,</text>',
     f'<text x="16" y="84" fill="{DIM}">so it is tabulated but never drawn. Each variant is '
     f'scored at the v0 it is optimal for, one lap, in blocks of TE. y inverted: up is nose '
     f'up.</text>']

# legend -- always present for >= 2 series
lx = 16
LEGY = TOP + GH + 34
for col, name in ((BASE, 'single-cycle'), (STEADY, 'steady')):
    o.append(f'<line x1="{lx}" y1="{LEGY}" x2="{lx+16}" y2="{LEGY}" stroke="{col}" '
             f'stroke-width="2"/>')
    o.append(f'<text x="{lx+22}" y="{LEGY+4}" fill="{FG}">{name}</text>')
    lx += 26 + len(name) * 6.2 + 18

# ---- headline. One panel, one measure: what re-solving v0 is worth per lap.
GW = PW


def headline(gx, title, sub, pts, col):
    if not pts:
        return
    vs = [v for _, v in pts]
    lo_, hi_ = min(min(vs), 0.0), max(max(vs), 0.0)
    pad = (hi_ - lo_) * 0.14 or 0.02
    lo_, hi_ = lo_ - pad, hi_ + pad
    xs = [n for n, _ in pts]
    gx_ = lambda n: gx + GW * (n - min(xs)) / max(max(xs) - min(xs), 1)
    gy_ = lambda v: TOP + GH - GH * (v - lo_) / (hi_ - lo_)
    o.append(f'<text x="{gx}" y="{TOP-26}" fill="{FG}" font-weight="600">{title}</text>')
    o.append(f'<text x="{gx}" y="{TOP-12}" fill="{DIM}">{sub}</text>')
    o.append(f'<rect x="{gx}" y="{TOP}" width="{GW}" height="{GH}" fill="none" stroke="{GRID}"/>')
    o.append(f'<line x1="{gx}" y1="{gy_(0):.1f}" x2="{gx+GW}" y2="{gy_(0):.1f}" stroke="{DIM}" '
             f'stroke-dasharray="3 3"/>')
    # put the label on whichever side of the zero line has room
    zdy = 12 if gy_(0) - TOP < GH * 0.5 else -5
    o.append(f'<text x="{gx+4}" y="{gy_(0)+zdy:.1f}" fill="{DIM}" font-size="9">no change</text>')
    o.append(f'<polyline points="{" ".join(f"{gx_(n):.1f},{gy_(v):.1f}" for n, v in pts)}" '
             f'fill="none" stroke="{col}" stroke-width="2"/>')
    for n, v in pts:
        o.append(f'<circle cx="{gx_(n):.1f}" cy="{gy_(v):.1f}" r="3.4" fill="{col}" '
                 f'stroke="{BG}" stroke-width="1.5"/>')
    for n, v in (pts[0], pts[-1]):
        anc = 'start' if n == pts[0][0] else 'end'
        dx = 2 if anc == 'start' else -2
        o.append(f'<text x="{gx_(n)+dx:.0f}" y="{gy_(v)-9:.0f}" text-anchor="{anc}" '
                 f'fill="{FG}">{v:+.3f}</text>')
    for v in (lo_, hi_):
        o.append(f'<text x="{gx-6}" y="{gy_(v)+3:.0f}" text-anchor="end" fill="{DIM}" '
                 f'font-size="9">{v:+.3f}</text>')
    for n in (xs[0], xs[len(xs)//2], xs[-1]):
        o.append(f'<text x="{gx_(n):.0f}" y="{TOP+GH+16}" text-anchor="middle" fill="{DIM}" '
                 f'font-size="9">n {n}</text>')


L0 = [r for r in R if float(r["lambda"]) == 0.0]
g1 = sorted((int(r["n"]), float(r["ss_dte_ss"]) - float(r["ctrl_dte_ss"])) for r in L0)
headline(LAB, 'steady state vs single cycle',
         'steady minus control, blocks of TE per lap', g1, STEADY)

lo, hi = -95, 95
for ri, r in enumerate(R):
    top = TOP + GH + 52 + ri * (PH + GAPY)
    stem = r["file"].replace(".pitches", "")
    n, lam = int(r["n"]), float(r["lambda"])
    py = lambda v: top + PH * (v - lo) / (hi - lo)

    o.append(f'<text x="{LAB-12}" y="{top+PH/2-2:.0f}" text-anchor="end" fill="{FG}" '
             f'font-weight="600">n {n}</text>')
    o.append(f'<text x="{LAB-12}" y="{top+PH/2+12:.0f}" text-anchor="end" fill="{DIM}">'
             f'lambda {lam:g}</text>')
    for v in (-90, 0, 90):
        o.append(f'<line x1="{LAB}" y1="{py(v):.1f}" x2="{LAB+PW}" y2="{py(v):.1f}" stroke="{GRID}"/>')
        o.append(f'<text x="{LAB-4}" y="{py(v)+3:.1f}" text-anchor="end" fill="{DIM}" '
                 f'font-size="9">{v}</text>')

    for tag, col, w, op in (("base", BASE, 1.4, 0.8), ("steady", STEADY, 1.6, 1.0)):
        v = curve(stem, tag)
        if not v:
            continue
        px = lambda t: LAB + PW * t / max(len(v) - 1, 1)
        pts = ' '.join(f'{px(t):.1f},{py(y):.1f}' for t, y in enumerate(v))
        o.append(f'<polyline points="{pts}" fill="none" stroke="{col}" stroke-width="{w}" '
                 f'opacity="{op}"/>')

    for t in (0, n // 2, n):
        o.append(f'<text x="{LAB + PW*t/n:.0f}" y="{top+PH+13:.0f}" text-anchor="middle" '
                 f'fill="{DIM}" font-size="9">{t}</text>')

    b, c, s = (float(r["base_dte_ss"]), float(r["ctrl_dte_ss"]), float(r["ss_dte_ss"]))
    nx = LAB + PW + 16
    o.append(f'<text x="{nx}" y="{top+12}" fill="{DIM}" font-size="9">dTE at own v0, blocks</text>')
    rowvals = [(BASE, 'single-cycle', b), (CTRL, 'control', c), (STEADY, 'steady', s)]
    for i, (col, name, val) in enumerate(rowvals):  # control: number only, never a curve
        y = top + 28 + i * 14
        o.append(f'<line x1="{nx}" y1="{y-3}" x2="{nx+12}" y2="{y-3}" stroke="{col}" stroke-width="2"/>')
        o.append(f'<text x="{nx+18}" y="{y}" fill="{FG}">{name}</text>')
        o.append(f'<text x="{nx+NUMW-30}" y="{y}" text-anchor="end" fill="{FG}">{val:+.3f}</text>')
    o.append(f'<text x="{nx}" y="{top+PH-10}" fill="{FG}" font-size="10" font-weight="600">'
             f'steady - control {s-c:+.3f}</text>')
    o.append(f'<text x="{nx}" y="{top+PH+4}" fill="{DIM}" font-size="9">'
             f'fixed point ({float(r["ss_v0y"]):+.3f}, {float(r["ss_v0z"]):+.3f})   '
             f'{r["passes"]} passes</text>')
    if r.get("ss_seam"):
        # Measured, not priced: nothing in the objective charges for p[n-1] -> p[0].
        o.append(f'<text x="{nx}" y="{top+PH+17}" fill="{DIM}" font-size="9">'
                 f'seam {float(r["base_seam"]):.0f} &#8594; {float(r["ss_seam"]):.1f} deg</text>')

o.append('</svg>')
os.makedirs(os.path.dirname(OUT) or ".", exist_ok=True)
open(OUT, 'w').write('\n'.join(o))
print("wrote", OUT, f"({len(R)} cells)")
