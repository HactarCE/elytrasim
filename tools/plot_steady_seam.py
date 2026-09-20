#!/usr/bin/env python3
"""The seam, and the two regimes it falls into.

The seam is the pitch jump from p[n-1] back to p[0] -- a move a steady-state schedule's pilot
actually has to make, and one nothing in the objective charges for. Re-solving v0 closes it
anyway, up to the horizon where the schedule instead ends parked at the pitch limit.

Usage:
    python3 tools/plot_steady_seam.py ROWS_DIR CURVE_DIR [OUT.svg]
"""
import csv, glob, os, sys

ROWS, CURVES = sys.argv[1], sys.argv[2]
OUT = sys.argv[3] if len(sys.argv) > 3 else "runs/steady/seam.svg"

BG, FG, DIM, GRID = '#0d1117', '#e6edf3', '#8b949e', '#21262d'
BASE, STEADY = '#388bfd', '#db6d28'

R = []
for f in sorted(glob.glob(os.path.join(ROWS, "*.csv"))):
    with open(f) as fh:
        R += list(csv.DictReader(fh))
R.sort(key=lambda r: int(r["n"]))

PW, PH, LAB, TOP = 760, 300, 70, 196
ZW, ZH, ZGAP = 232, 150, 26
zoom_ns = [300, 376, 450]          # one from each side of the regime break, plus the break
W = LAB + PW + 60
H = TOP + PH + 96 + ZH + 78

o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
     f'font-family="ui-monospace,SFMono-Regular,Menlo,monospace" font-size="10">',
     f'<rect width="{W}" height="{H}" fill="{BG}"/>',
     f'<text x="16" y="24" fill="{FG}" font-size="15" font-weight="600">'
     f'the seam: what steady state closes, and what it cannot</text>',
     f'<text x="16" y="44" fill="{DIM}">The seam is |p[n-1] -&gt; p[0]|, the jump a repeated '
     f'schedule asks a hand to make.</text>',
     f'<text x="16" y="58" fill="{DIM}">runs/atlas/nsweep, lambda 0, mu 1e-4, limit 85. Nothing '
     f'here prices the seam; it is measured, not optimized.</text>',
     f'<text x="16" y="76" fill="{DIM}">Steady state closes the seam on its own -- at the fixed '
     f'point the cycle ends in the velocity it began in, so the</text>',
     f'<text x="16" y="90" fill="{DIM}">pitch it wants at tick 0 is near the pitch it wanted at '
     f'tick n-1.</text>',
     f'<text x="16" y="108" fill="{DIM}">Beyond n=376 the schedule instead ends parked at the '
     f'+85 limit and the seam stays wide. That is the free terminal</text>',
     f'<text x="16" y="122" fill="{DIM}">velocity, not roughness: nothing after tick n is priced, '
     f'so the last few ticks cash energy in against the limit.</text>']

lx = 16
for col, name in ((BASE, 'single-cycle'), (STEADY, 'steady')):
    o.append(f'<line x1="{lx}" y1="{TOP-28}" x2="{lx+16}" y2="{TOP-28}" stroke="{col}" stroke-width="2"/>')
    o.append(f'<text x="{lx+22}" y="{TOP-24}" fill="{FG}">{name}</text>')
    lx += 26 + len(name) * 6.2 + 20

ns = [int(r["n"]) for r in R]
series = [(BASE, [float(r["base_seam"]) for r in R]),
          (STEADY, [float(r["ss_seam"]) for r in R])]
hi = max(max(v) for _, v in series) * 1.08
px = lambda n: LAB + PW * (n - ns[0]) / (ns[-1] - ns[0])
py = lambda v: TOP + PH - PH * v / hi
o.append(f'<text x="{LAB}" y="{TOP-44}" fill="{FG}" font-weight="600">seam jump, degrees</text>')
o.append(f'<rect x="{LAB}" y="{TOP}" width="{PW}" height="{PH}" fill="none" stroke="{GRID}"/>')
for v in (0, 25, 50, 75, 100):
    if v <= hi:
        o.append(f'<line x1="{LAB}" y1="{py(v):.1f}" x2="{LAB+PW}" y2="{py(v):.1f}" stroke="{GRID}"/>')
        o.append(f'<text x="{LAB-6}" y="{py(v)+3:.1f}" text-anchor="end" fill="{DIM}" font-size="9">{v}</text>')
# the regime break
bx = px(388)
o.append(f'<line x1="{bx:.1f}" y1="{TOP}" x2="{bx:.1f}" y2="{TOP+PH}" stroke="{DIM}" stroke-dasharray="3 3"/>')
o.append(f'<text x="{bx-6:.0f}" y="{TOP+16}" text-anchor="end" fill="{DIM}" font-size="9">'
         f'beyond here: parked at the limit &#8594;</text>')
for col, vals in series:
    o.append(f'<polyline points="{" ".join(f"{px(n):.1f},{py(v):.1f}" for n, v in zip(ns, vals))}" '
             f'fill="none" stroke="{col}" stroke-width="2"/>')
    for n, v in zip(ns, vals):
        o.append(f'<circle cx="{px(n):.1f}" cy="{py(v):.1f}" r="3.2" fill="{col}" stroke="{BG}" stroke-width="1.5"/>')
for n in (ns[0], 300, 376, ns[-1]):
    o.append(f'<text x="{px(n):.0f}" y="{TOP+PH+16}" text-anchor="middle" fill="{DIM}" font-size="9">n {n}</text>')

# ---- zooms across the seam: last 12 ticks, then the first 12, laid end to end
ZY = TOP + PH + 96
o.append(f'<text x="{LAB}" y="{ZY-16}" fill="{FG}" font-weight="600">across the seam</text>')
o.append(f'<text x="{LAB+150}" y="{ZY-16}" fill="{DIM}">last 12 ticks | first 12 ticks. '
         f'y inverted: up is nose up.</text>')
K = 12
for i, target in enumerate(zoom_ns):
    row = min(R, key=lambda r: abs(int(r["n"]) - target))
    stem = row["file"].replace(".pitches", "")
    x0 = LAB + i * (ZW + ZGAP)
    o.append(f'<text x="{x0}" y="{ZY-2}" fill="{FG}" font-weight="600">n {row["n"]}</text>')
    o.append(f'<rect x="{x0}" y="{ZY+6}" width="{ZW}" height="{ZH}" fill="none" stroke="{GRID}"/>')
    zy = lambda v: ZY + 6 + ZH * (v + 95) / 190
    for v in (-85, 0, 85):
        o.append(f'<line x1="{x0}" y1="{zy(v):.1f}" x2="{x0+ZW}" y2="{zy(v):.1f}" stroke="{GRID}"/>')
        if i == 0:
            o.append(f'<text x="{x0-5}" y="{zy(v)+3:.1f}" text-anchor="end" fill="{DIM}" font-size="9">{v}</text>')
    o.append(f'<line x1="{x0+ZW/2:.1f}" y1="{ZY+6}" x2="{x0+ZW/2:.1f}" y2="{ZY+6+ZH}" '
             f'stroke="{DIM}" stroke-dasharray="2 3"/>')
    for tag, col in (("base", BASE), ("steady", STEADY)):
        p = os.path.join(CURVES, f"{stem}.{tag}.csv")
        if not os.path.exists(p):
            continue
        with open(p) as fh:
            v = [float(x["pitch"]) for x in csv.DictReader(fh)]
        seg = v[-K:] + v[:K]
        zx = lambda j: x0 + ZW * j / (len(seg) - 1)
        o.append(f'<polyline points="{" ".join(f"{zx(j):.1f},{zy(y):.1f}" for j, y in enumerate(seg))}" '
                 f'fill="none" stroke="{col}" stroke-width="1.8"/>')
    o.append(f'<text x="{x0+ZW/2:.0f}" y="{ZY+6+ZH+14}" text-anchor="middle" fill="{DIM}" '
             f'font-size="9">seam  ({float(row["base_seam"]):.0f} &#8594; '
             f'{float(row["ss_seam"]):.1f} deg)</text>')

o.append('</svg>')
os.makedirs(os.path.dirname(OUT) or ".", exist_ok=True)
open(OUT, 'w').write('\n'.join(o))
print("wrote", OUT)
