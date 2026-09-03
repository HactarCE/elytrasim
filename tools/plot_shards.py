#!/usr/bin/env python3
"""Pitch across num_ticks for two starting velocities, side by side.

x is the fraction of the horizon so cycles of different length line up. y is inverted, so up on
the page is nose up. Both statistics are printed per panel because they disagree: lag-1 was
calibrated at n = 300 and reads negative on schedules that are visibly smooth at other n, while
TV/tick separates the chattering profiles from the clean ones by about 4x with nothing between.

Usage: plot_shards.py <corpus> <shardA> <shardB> [out.svg]
"""
import sys, os, re, statistics as st

BG, FG, DIM, GRID = '#0d1117', '#e6edf3', '#8b949e', '#21262d'
REF, CLEAN, DIRTY = '#58a6ff', '#3fb950', '#f85149'
PW, PH, AXW, GAPX, GAPY, TOP, LAB = 430, 116, 30, 26, 18, 76, 74

corpus, sa, sb = sys.argv[1], sys.argv[2], sys.argv[3]
out = sys.argv[4] if len(sys.argv) > 4 else 'runs/veljit/shards.svg'
NS = [100, 150, 200, 250, 300, 350, 400, 450, 500, 550, 600]

def load(p):
    h, v = {}, []
    for line in open(p):
        if line.startswith('#'):
            q = line[1:].split('#')[0].split()
            if len(q) >= 2: h[q[0]] = q[1]
        else: v += [float(x) for x in line.split()]
    return h, v

def stats(v):
    d = [v[i+1]-v[i] for i in range(len(v)-1)]
    m = st.mean(d); den = sum((x-m)**2 for x in d)
    l1 = (sum((d[i]-m)*(d[i+1]-m) for i in range(len(d)-1))/den) if den else 1.0
    nc, ar = 0, False
    for q in v:
        if q > 20.0: ar = True
        elif ar and q < -30.0: nc += 1; ar = False
    return l1, sum(abs(x) for x in d)/max(len(d), 1), nc

ref = load('runs/veljit/ref300.pitches')[1]
lo, hi = -95, 95
W = LAB + 2*(PW+GAPX)
H = TOP + len(NS)*(PH+GAPY) + 26
o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
     f'font-family="ui-monospace,SFMono-Regular,Menlo,monospace" font-size="10">',
     f'<rect width="{W}" height="{H}" fill="{BG}"/>',
     f'<text x="14" y="22" fill="{FG}" font-size="14" font-weight="600">'
     f'one cycle across num_ticks, at two starting velocities</text>',
     f'<text x="14" y="39" fill="{DIM}">lambda 0, 8 passes, jitter 0.1. x is the fraction of the '
     f'horizon. y inverted: up is nose up. <tspan fill="{REF}">blue</tspan> = the 300-tick reference.</text>',
     f'<text x="14" y="54" fill="{DIM}">line colored by TV/tick: '
     f'<tspan fill="{CLEAN}">under 3, clean</tspan> / <tspan fill="{DIRTY}">over 3, chattering</tspan>. '
     f'lag-1 shown too, and it disagrees.</text>']

for ci, sh in enumerate((sa, sb)):
    x0 = LAB + ci*(PW+GAPX)
    o.append(f'<text x="{x0+AXW}" y="{TOP-8}" fill="{FG}" font-weight="600">v0 = {sh}</text>')

for ri, n in enumerate(NS):
    top = TOP + ri*(PH+GAPY)
    py = lambda y: top + PH*(y-lo)/(hi-lo)
    o.append(f'<text x="{LAB-10}" y="{top+PH/2+4:.0f}" text-anchor="end" fill="{FG}">n {n}</text>')
    for ci, sh in enumerate((sa, sb)):
        x0 = LAB + ci*(PW+GAPX)
        for y in (-90, 0, 90):
            o.append(f'<line x1="{x0+AXW}" y1="{py(y):.1f}" x2="{x0+PW}" y2="{py(y):.1f}" stroke="{GRID}"/>')
            if ci == 0:
                o.append(f'<text x="{x0+AXW-4}" y="{py(y)+3:.1f}" text-anchor="end" fill="{DIM}" font-size="8">{y}</text>')
        px = lambda t, m: x0 + AXW + (PW-AXW)*t/max(m-1, 1)
        o.append('<polyline points="{}" fill="none" stroke="{}" stroke-width="0.9" opacity="0.35"/>'.format(
            ' '.join(f'{px(t,len(ref)):.1f},{py(y):.1f}' for t, y in enumerate(ref)), REF))
        f = os.path.join(corpus, sh, f'n{n:04d}_lam+0.000000.pitches')
        if not os.path.exists(f):
            o.append(f'<text x="{x0+AXW+6}" y="{top+PH/2}" fill="{DIM}">missing</text>'); continue
        h, v = load(f)
        l1, tvt, nc = stats(v)
        col = CLEAN if tvt < 3.0 else DIRTY
        o.append('<polyline points="{}" fill="none" stroke="{}" stroke-width="1.2"/>'.format(
            ' '.join(f'{px(t,len(v)):.1f},{py(y):.1f}' for t, y in enumerate(v)), col))
        o.append(f'<text x="{x0+PW-3}" y="{top+11}" text-anchor="end" fill="{DIM}" font-size="9">'
                 f'dTE {float(h["dte"]):+.1f}  cyc {nc}  TV/t <tspan fill="{col}">{tvt:.2f}</tspan>'
                 f'  lag-1 {l1:+.2f}</text>')
o.append('</svg>')
open(out, 'w').write('\n'.join(o))
print("wrote", out)
