#!/usr/bin/env python3
"""One continuation hop: n=300 polished k passes, then n=310 seeded from it and polished k more.

Rows are k. Columns pair each sigma's anchor with the cell one hop out. If polishing compounds
along a continuation path, the n=310 column degrades faster than the n=300 column it came from
-- that is the thing a per-cell pass budget cannot bound.

y is inverted so up on the page is nose up. Pass a tick range to zoom: cont.py 0 40
"""
import sys, os, statistics as st

BG, FG, DIM, GRID = '#0d1117', '#e6edf3', '#8b949e', '#21262d'
GOOD, BAD, REF = '#3fb950', '#f85149', '#58a6ff'
PW, PH, AXW, LAB, GAPX, GAPY, TOP = 340, 116, 34, 92, 22, 20, 62

t0 = int(sys.argv[1]) if len(sys.argv) > 2 else 0
t1 = int(sys.argv[2]) if len(sys.argv) > 2 else 10**9
zoom = len(sys.argv) > 2

def load(p):
    return [float(x) for l in open(p) if not l.startswith('#') for x in l.split()]

def stats(f):
    d = [f[i+1]-f[i] for i in range(len(f)-1)]
    m = st.mean(d); den = sum((x-m)**2 for x in d)
    l1 = sum((d[i]-m)*(d[i+1]-m) for i in range(len(d)-1))/den if den else 1.0
    return l1, sum(abs(x) for x in d)

cols = [('s000', 300, 'sigma 0   n 300'), ('s000', 310, 'sigma 0   n 310'),
        ('s010', 300, 'sigma 0.1  n 300'), ('s010', 310, 'sigma 0.1  n 310')]
rows = [0, 1, 2, 3, 4]
ref = load('runs/veljit/ref300.pitches')
lo, hi = -95, 95

W = LAB + len(cols)*(PW+GAPX)
H = TOP + len(rows)*(PH+GAPY) + 24
o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
     f'font-family="ui-monospace,SFMono-Regular,Menlo,monospace" font-size="10">',
     f'<rect width="{W}" height="{H}" fill="{BG}"/>',
     f'<text x="14" y="20" fill="{FG}" font-size="14" font-weight="600">'
     f'one continuation hop: n 300 -&gt; n 310, same pass count each'
     f'{"  [entry zoom, ticks %d-%d]" % (t0, t1) if zoom else ""}</text>',
     f'<text x="14" y="36" fill="{DIM}">lambda 0, v0 (0.1675, 0.2009). n 310 is seeded from the '
     f'n 300 cell in its own row, so row k is k passes then k more. '
     f'y inverted: up is nose up. blue = the reference cycle.</text>']

for ci, (_, _, title) in enumerate(cols):
    x0 = LAB + ci*(PW+GAPX)
    o.append(f'<text x="{x0+AXW}" y="{TOP-10}" fill="{FG}" font-weight="600">{title}</text>')

for ri, k in enumerate(rows):
    top = TOP + ri*(PH+GAPY)
    o.append(f'<text x="{LAB-10}" y="{top+PH/2+4:.0f}" text-anchor="end" fill="{FG}">'
             f'{k} pass{"" if k == 1 else "es"}</text>')
    for ci, (tag, n, _) in enumerate(cols):
        x0 = LAB + ci*(PW+GAPX)
        f = f'runs/veljit/cont/n{n}_{tag}_p{k}.pitches'
        py = lambda y: top + PH*(y-lo)/(hi-lo)
        for y in (-90, 0, 90):
            o.append(f'<line x1="{x0+AXW}" y1="{py(y):.1f}" x2="{x0+PW}" y2="{py(y):.1f}" stroke="{GRID}"/>')
            if ci == 0:
                o.append(f'<text x="{x0+AXW-4}" y="{py(y)+3:.1f}" text-anchor="end" fill="{DIM}" '
                         f'font-size="9">{y}</text>')
        if not os.path.exists(f):
            o.append(f'<text x="{x0+AXW+8}" y="{top+PH/2}" fill="{DIM}">missing</text>'); continue
        v = load(f)
        a, b = t0, min(t1, len(v)-1)
        span = max(b - a, 1)
        px = lambda t: x0 + AXW + (PW-AXW)*(t-a)/span
        seg = lambda w: ' '.join(f'{px(t):.1f},{py(y):.1f}' for t, y in enumerate(w) if a <= t <= b)
        o.append(f'<polyline points="{seg(ref)}" fill="none" stroke="{REF}" stroke-width="1" opacity="0.35"/>')
        l1, tv = stats(v)
        col = GOOD if l1 > 0.2 else BAD
        o.append(f'<polyline points="{seg(v)}" fill="none" stroke="{col}" stroke-width="1.3"/>')
        o.append(f'<text x="{x0+PW-2}" y="{top+11}" text-anchor="end" fill="{col}" font-size="9">'
                 f'lag-1 {l1:+.2f}  TV {tv:.0f}</text>')
        if ri == len(rows)-1:
            for t in (a, (a+b)//2, b):
                o.append(f'<text x="{px(t):.0f}" y="{top+PH+14}" text-anchor="middle" fill="{DIM}" '
                         f'font-size="9">{t}</text>')
o.append('</svg>')
out = 'runs/veljit/cont_zoom.svg' if zoom else 'runs/veljit/cont.svg'
open(out,'w').write('\n'.join(o))
print("wrote", out)
