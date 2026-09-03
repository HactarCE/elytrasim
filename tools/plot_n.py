#!/usr/bin/env python3
"""Pitch through the cycle across num_ticks, one panel per n.

x is the fraction of the horizon, not the tick, so cycles of different length line up and the
question "is this still one cycle, stretched?" is the one the picture answers. The reference
cycle is drawn behind each panel on the same fractional axis.

y is inverted: negative pitch is nose up, so up on the page is up in the world.

Usage: plot_n.py <dir> [out.svg]
"""
import sys, os, glob, re, statistics as st

BG, FG, DIM, GRID = '#0d1117', '#e6edf3', '#8b949e', '#21262d'
REF, LINE = '#58a6ff', '#d29922'
PW, PH, AXW, GAPY, TOP, LEFT = 940, 132, 52, 22, 66, 0

d = sys.argv[1] if len(sys.argv) > 1 else '/tmp/nsweep'
out = sys.argv[2] if len(sys.argv) > 2 else 'runs/veljit/by_n.svg'

def load(p):
    h, v = {}, []
    for line in open(p):
        if line.startswith('#'):
            q = line[1:].split('#')[0].split()
            if len(q) >= 2: h[q[0]] = q[1]
        else:
            v += [float(x) for x in line.split()]
    return h, v

want = None
if len(sys.argv) > 3:
    want = {int(x) for x in sys.argv[3].split(',')}
files = sorted(glob.glob(os.path.join(d, '**', '*.pitches'), recursive=True),
               key=lambda f: int(re.search(r'n(\d+)_', os.path.basename(f)).group(1)))
files = [f for f in files if 'lam+0.000000' in os.path.basename(f)]
if want:
    files = [f for f in files if int(re.search(r'n(\d+)_', os.path.basename(f)).group(1)) in want]
ref = load('runs/veljit/ref300.pitches')[1]
rows = [load(f) for f in files]
lo, hi = -95, 95

W = LEFT + PW + 8
H = TOP + len(rows)*(PH+GAPY) + 26
o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
     f'font-family="ui-monospace,SFMono-Regular,Menlo,monospace" font-size="10">',
     f'<rect width="{W}" height="{H}" fill="{BG}"/>',
     f'<text x="{AXW}" y="22" fill="{FG}" font-size="14" font-weight="600">'
     f'one cycle, swept across num_ticks</text>',
     f'<text x="{AXW}" y="40" fill="{DIM}">lambda 0, v0 (0,0), 8 passes, jitter 0.1. '
     f'x is the fraction of the horizon so cycles of different length line up.</text>',
     f'<text x="{AXW}" y="55" fill="{DIM}">y inverted: up is nose up. '
     f'<tspan fill="{REF}">blue</tspan> is the 300-tick reference on the same fractional axis.</text>']

for i, (h, v) in enumerate(rows):
    top = TOP + i*(PH+GAPY)
    py = lambda y: top + PH*(y-lo)/(hi-lo)
    n = len(v)
    for y in (-90, 0, 90):
        o.append(f'<line x1="{AXW}" y1="{py(y):.1f}" x2="{PW}" y2="{py(y):.1f}" stroke="{GRID}"/>')
        o.append(f'<text x="{AXW-5}" y="{py(y)+3:.1f}" text-anchor="end" fill="{DIM}" font-size="9">{y}</text>')
    px = lambda t, m: AXW + (PW-AXW)*t/max(m-1, 1)
    o.append('<polyline points="{}" fill="none" stroke="{}" stroke-width="1" opacity="0.4"/>'.format(
        ' '.join(f'{px(t,len(ref)):.1f},{py(y):.1f}' for t, y in enumerate(ref)), REF))
    o.append('<polyline points="{}" fill="none" stroke="{}" stroke-width="1.3"/>'.format(
        ' '.join(f'{px(t,n):.1f},{py(y):.1f}' for t, y in enumerate(v)), LINE))
    o.append(f'<text x="{AXW+6}" y="{top+12}" fill="{FG}" font-weight="600">n {n}</text>')
    dd = [v[k+1]-v[k] for k in range(len(v)-1)]
    m = st.mean(dd); den = sum((x-m)**2 for x in dd)
    l1 = (sum((dd[k]-m)*(dd[k+1]-m) for k in range(len(dd)-1))/den) if den else 1.0
    tv = sum(abs(x) for x in dd)
    nc, armed = 0, False
    for q in v:
        if q > 20.0: armed = True
        elif armed and q < -30.0: nc += 1; armed = False
    o.append(f'<text x="{PW-4}" y="{top+12}" text-anchor="end" fill="{DIM}">'
             f'dTE {float(h.get("dte", h.get("dJ", 0))):+.2f}   '
             f'cycles {nc}   lag-1 {l1:+.2f}   TV {tv:.0f}</text>')

o.append(f'<text x="{(AXW+PW)/2:.0f}" y="{H-8}" text-anchor="middle" fill="{DIM}">'
         f'fraction of the horizon</text>')
o.append('</svg>')
os.makedirs(os.path.dirname(out) or '.', exist_ok=True)
open(out, 'w').write('\n'.join(o))
print("wrote", out)
