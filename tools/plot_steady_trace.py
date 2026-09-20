#!/usr/bin/env python3
"""The outer alternation, pass by pass: does v0 walk in, or ring?

One sweep at a frozen v0, then v0 re-solved to convergence -- the loop from
``cycle-optimizer``'s ``optimization_step``. Plots the v0 path in the (v_y, v_z) plane and the
per-pass drift, because the two answer different questions: the path shows *direction* (a
monotone walk is converging slowly; a reversal is ringing) and the drift shows *rate*.

Usage:
    python3 tools/plot_steady_trace.py TRACE.csv [OUT.svg]
"""
import csv, math, os, sys

SRC = sys.argv[1]
OUT = sys.argv[2] if len(sys.argv) > 2 else "runs/steady/trace.svg"

BG, FG, DIM, GRID = '#0d1117', '#e6edf3', '#8b949e', '#21262d'
PATH, DRIFT = '#db6d28', '#388bfd'

with open(SRC) as fh:
    R = list(csv.DictReader(fh))
vy = [float(r["v0y"]) for r in R]
vz = [float(r["v0z"]) for r in R]
dv = [float(r["dv0"]) for r in R]

PW, PH, PAD, TOP = 400, 300, 58, 96
W, H = PAD + PW + 80 + PW + PAD, TOP + PH + 66
o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
     f'font-family="ui-monospace,SFMono-Regular,Menlo,monospace" font-size="10">',
     f'<rect width="{W}" height="{H}" fill="{BG}"/>',
     f'<text x="16" y="24" fill="{FG}" font-size="15" font-weight="600">'
     f'the outer alternation, pass by pass</text>',
     f'<text x="16" y="42" fill="{DIM}">n=150, lambda=0. One sweep at a frozen v0, then v0 '
     f're-solved to its fixed point. {len(R)} passes.</text>',
     f'<text x="16" y="56" fill="{DIM}">The walk is monotone -- it converges slowly, it does not '
     f'ring -- so damping the update would only add the lag that causes ringing.</text>']

def axes(x0, title, sub):
    o.append(f'<text x="{x0}" y="{TOP-26}" fill="{FG}" font-weight="600">{title}</text>')
    o.append(f'<text x="{x0}" y="{TOP-12}" fill="{DIM}">{sub}</text>')
    o.append(f'<rect x="{x0}" y="{TOP}" width="{PW}" height="{PH}" fill="none" stroke="{GRID}"/>')

# --- left: the v0 path in (v_y, v_z)
x0 = PAD
axes(x0, 'v0 path', 'each dot one pass; start is filled, end is ringed')
ylo, yhi = min(vy) - 0.01, max(vy) + 0.01
zlo, zhi = min(vz) - 0.01, max(vz) + 0.01
px = lambda v: x0 + PW * (v - ylo) / (yhi - ylo)
py = lambda v: TOP + PH - PH * (v - zlo) / (zhi - zlo)
o.append(f'<polyline points="{" ".join(f"{px(a):.1f},{py(b):.1f}" for a, b in zip(vy, vz))}" '
         f'fill="none" stroke="{PATH}" stroke-width="2"/>')
o.append(f'<circle cx="{px(vy[0]):.1f}" cy="{py(vz[0]):.1f}" r="4.5" fill="{PATH}"/>')
o.append(f'<circle cx="{px(vy[-1]):.1f}" cy="{py(vz[-1]):.1f}" r="4.5" fill="{BG}" '
         f'stroke="{PATH}" stroke-width="2"/>')
o.append(f'<text x="{px(vy[0])+8:.0f}" y="{py(vz[0])+4:.0f}" fill="{FG}">pass 0</text>')
o.append(f'<text x="{px(vy[-1])+8:.0f}" y="{py(vz[-1])+4:.0f}" fill="{FG}">pass {len(R)-1}</text>')
for v in (ylo, (ylo+yhi)/2, yhi):
    o.append(f'<text x="{px(v):.0f}" y="{TOP+PH+16}" text-anchor="middle" fill="{DIM}" '
             f'font-size="9">{v:.3f}</text>')
for v in (zlo, (zlo+zhi)/2, zhi):
    o.append(f'<text x="{x0-6}" y="{py(v)+3:.0f}" text-anchor="end" fill="{DIM}" '
             f'font-size="9">{v:.3f}</text>')
o.append(f'<text x="{x0+PW/2:.0f}" y="{TOP+PH+34}" text-anchor="middle" fill="{DIM}">v_y</text>')

# --- right: drift per pass, log y
x0 = PAD + PW + 80
axes(x0, 'drift per pass', 'L1 |v0 change|, log scale')
lo, hi = math.log10(max(min(dv), 1e-9)), math.log10(max(dv))
lo, hi = math.floor(lo), math.ceil(hi)
px = lambda t: x0 + PW * t / max(len(dv) - 1, 1)
py = lambda d: TOP + PH - PH * (math.log10(max(d, 10**lo)) - lo) / (hi - lo)
for e in range(int(lo), int(hi) + 1):
    o.append(f'<line x1="{x0}" y1="{py(10**e):.1f}" x2="{x0+PW}" y2="{py(10**e):.1f}" stroke="{GRID}"/>')
    o.append(f'<text x="{x0-6}" y="{py(10**e)+3:.1f}" text-anchor="end" fill="{DIM}" '
             f'font-size="9">1e{e}</text>')
o.append(f'<polyline points="{" ".join(f"{px(t):.1f},{py(d):.1f}" for t, d in enumerate(dv))}" '
         f'fill="none" stroke="{DRIFT}" stroke-width="2"/>')
for t in (0, len(dv)//2, len(dv)-1):
    o.append(f'<text x="{px(t):.0f}" y="{TOP+PH+16}" text-anchor="middle" fill="{DIM}" '
             f'font-size="9">{t}</text>')
o.append(f'<text x="{x0+PW/2:.0f}" y="{TOP+PH+34}" text-anchor="middle" fill="{DIM}">pass</text>')

o.append('</svg>')
os.makedirs(os.path.dirname(OUT) or ".", exist_ok=True)
open(OUT, 'w').write('\n'.join(o))
print("wrote", OUT)
