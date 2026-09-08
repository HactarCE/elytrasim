#!/usr/bin/env python3
"""Steady glide against pitch, on three metrics, with the critical points marked.

Panels share the pitch axis. Horizontal velocity is `sim`'s z component -- yaw is pinned to
zero -- but is labelled vx, which is the convention everywhere outside the sim.

Velocities are blocks/second, as the CSVs carry them. The glide ratio is a ratio of two
velocities, so it is dimensionless and unaffected.

Nothing here is unbounded on the full [-90, 90] domain: the worst sink is 78.4 b/s at either
pole. The problem is dynamic range, not bounds. vy spans 55x between the min-sink glide and a
vertical dive, so it gets two panels -- the full range, and a zoom on the shallow end where the
critical points live. vx and the glide ratio are readable at full range as they are.

The last panel is the one that matters: the glide ratio within a degree of pitch 0. For p >= 0
the tick map depends on pitch only through `lift_force = cos^2(p)`, which is even, so the curve
is flat to second order on the right. For p < 0 the `lean_angle < 0` branch switches on with a
term linear in `sin(p)`. Pitch 0 is therefore a corner, not a jump -- and a deeply asymmetric
one. That panel draws both trig modes, because Minecraft's sine table quantizes the left branch
into a staircase and kills it outright inside |p| < 0.0027 deg, while the right branch is
bit-identical between the two.

Usage: plot_glide.py <libm.csv> <mth.csv> <zoom_libm.csv> <zoom_mth.csv> [out.svg]
"""
import sys, os

BG, FG, DIM, GRID = '#0d1117', '#e6edf3', '#8b949e', '#21262d'
LINE, ALT, CRIT = '#388bfd', '#bb8009', '#8b949e'   # validated vs #0d1117, dark categorical
PW, PH, AXW, GAPY, TOP, RIGHT = 1000, 150, 66, 58, 108, 20

libm_p = sys.argv[1] if len(sys.argv) > 1 else 'runs/glide/glide_libm.csv'
mth_p  = sys.argv[2] if len(sys.argv) > 2 else 'runs/glide/glide_mth.csv'
zl_p   = sys.argv[3] if len(sys.argv) > 3 else 'runs/glide/zoom_libm.csv'
zm_p   = sys.argv[4] if len(sys.argv) > 4 else 'runs/glide/zoom_mth.csv'
out    = sys.argv[5] if len(sys.argv) > 5 else 'runs/glide/glide.svg'

def load(p):
    rows = []
    for line in open(p):
        if line.startswith('#') or line.startswith('pitch'):
            continue
        f = line.split(',')
        rows.append((float(f[0]), float(f[1]), float(f[2]), float(f[4])))   # pitch, vy, vx, glide
    return rows

L, M, ZL, ZM = load(libm_p), load(mth_p), load(zl_p), load(zm_p)

# The critical points, from `myopic crit` (libm). Each is an argmax of the metric named.
CRITS = [(-13.058, '-13.058', 'end'), (0.0, '0', 'start'), (53.366, '53.366', 'start')]

def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')

# panel: (title, subtitle, column index, ylo, yhi, yticks, xlo, xhi, series)
P = [
    ('vy  -- sink rate, blocks/second', 'full range; both poles settle at -78.400',
     1, -80.0, 0.0, [-80, -60, -40, -20, 0], -90, 90, [(L, LINE, None)]),
    ('vy  -- sink rate, zoomed 12x', 'the shallow end, where the critical points live; the dive tails run off-panel',
     1, -7.0, 0.0, [-6, -4, -2, 0], -90, 90, [(L, LINE, None)]),
    ('vx  -- forward speed, blocks/second', 'peaks at pitch 53.366, and is 0 at both poles',
     2, 0.0, 72.0, [0, 20, 40, 60], -90, 90, [(L, LINE, None)]),
    ('glide ratio  -- blocks forward per block fallen', 'dimensionless; peaks at pitch 0, at 10.102',
     3, 0.0, 10.6, [0, 2, 4, 6, 8, 10], -90, 90, [(L, LINE, None)]),
    ('glide ratio, within 0.06 deg of pitch 0', 'the corner: flat to second order on the right, linear on the left, and no jump',
     3, 10.0775, 10.1032, [10.08, 10.09, 10.10], -0.06, 0.06,
     [(ZL, LINE, 'libm'), (ZM, ALT, "mth_lut (Minecraft's sine table)")]),
]

W = AXW + PW + RIGHT
H = TOP + len(P)*(PH+GAPY) + 30
o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
     f'font-family="ui-monospace,SFMono-Regular,Menlo,monospace" font-size="10">',
     f'<rect width="{W}" height="{H}" fill="{BG}"/>',
     f'<text x="{AXW}" y="26" fill="{FG}" font-size="15" font-weight="600">'
     f'steady glide against pitch</text>',
     f'<text x="{AXW}" y="45" fill="{DIM}">terminal velocity of the constant-pitch tick map, '
     f'yaw 0. nose-down is positive pitch. dotted verticals are the three critical points.</text>',
     f'<text x="{AXW}" y="60" fill="{DIM}">'
     f'<tspan fill="{CRIT}">|</tspan> pitch 0 max glide ratio &#183; '
     f'-13.058 min sink &#183; 53.366 max forward speed. '
     f'trig is libm except where marked.</text>']

for i, (title, sub, col, ylo, yhi, yt, xlo, xhi, series) in enumerate(P):
    top = TOP + i*(PH+GAPY)
    py = lambda y: top + PH*(1 - (y-ylo)/(yhi-ylo))
    px = lambda x: AXW + PW*(x-xlo)/(xhi-xlo)
    o.append(f'<text x="{AXW}" y="{top-20}" fill="{FG}" font-weight="600">{esc(title)}</text>')
    o.append(f'<text x="{AXW}" y="{top-7}" fill="{DIM}">{esc(sub)}</text>')
    # gridlines
    for y in yt:
        o.append(f'<line x1="{AXW}" y1="{py(y):.1f}" x2="{AXW+PW}" y2="{py(y):.1f}" stroke="{GRID}"/>')
        lab = f'{y:g}' if abs(y) >= 1 or y == 0 else f'{y:.2f}'
        o.append(f'<text x="{AXW-6}" y="{py(y)+3:.1f}" text-anchor="end" fill="{DIM}" font-size="9">{lab}</text>')
    xt = [x for x in ([-90,-60,-30,0,30,60,90] if xhi > 10 else [-0.06,-0.03,0,0.03,0.06]) if xlo <= x <= xhi]
    for x in xt:
        o.append(f'<line x1="{px(x):.1f}" y1="{top}" x2="{px(x):.1f}" y2="{top+PH}" stroke="{GRID}"/>')
        o.append(f'<text x="{px(x):.1f}" y="{top+PH+13}" text-anchor="middle" fill="{DIM}" font-size="9">{x:g}</text>')
    # critical points; labelled once, in the top panel, so the labels cannot collide
    for cp, lab, anc in CRITS:
        if not (xlo <= cp <= xhi):
            continue
        o.append(f'<line x1="{px(cp):.1f}" y1="{top}" x2="{px(cp):.1f}" y2="{top+PH}" '
                 f'stroke="{CRIT}" stroke-width="1" stroke-dasharray="2 3" opacity="0.85"/>')
        if i == 0:
            dx = 4 if anc == 'start' else -4
            o.append(f'<text x="{px(cp)+dx:.1f}" y="{top-6}" text-anchor="{anc}" fill="{FG}" '
                     f'font-size="9">{esc(lab)}</text>')
    # series, clipped to the panel box
    cid = f'clip{i}'
    o.append(f'<clipPath id="{cid}"><rect x="{AXW}" y="{top}" width="{PW}" height="{PH}"/></clipPath>')
    for rows, color, name in series:
        pts = ' '.join(f'{px(r[0]):.2f},{py(r[col]):.2f}' for r in rows if xlo <= r[0] <= xhi)
        o.append(f'<polyline points="{pts}" fill="none" stroke="{color}" stroke-width="2" '
                 f'stroke-linejoin="round" clip-path="url(#{cid})"/>')
    # legend, only where there is more than one series
    if len(series) > 1:
        # right-aligned, so it cannot run into the panel subtitle on the left
        ly, lx = top - 6, AXW + PW
        for _, color, name in reversed(series):
            o.append(f'<text x="{lx}" y="{ly}" text-anchor="end" fill="{FG}" font-size="9">{esc(name)}</text>')
            lx -= 6.1*len(name) + 22
            o.append(f'<line x1="{lx}" y1="{ly-3}" x2="{lx+16}" y2="{ly-3}" stroke="{color}" stroke-width="2"/>')
            lx -= 34

o.append(f'<text x="{AXW+PW/2:.0f}" y="{H-8}" text-anchor="middle" fill="{DIM}">'
         f'pitch, degrees (positive is nose down)</text>')
o.append('</svg>')
os.makedirs(os.path.dirname(out) or '.', exist_ok=True)
open(out, 'w').write('\n'.join(o))
print("wrote", out)
