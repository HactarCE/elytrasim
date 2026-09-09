#!/usr/bin/env python3
"""One cell up close: what the price actually removed.

    python3 tools/plot_pitch_zoom.py 450 0.0 200 260 out.png

Top row is the whole schedule, old and new. Bottom left zooms the window given on the command
line, which is where the old cell chatters. Bottom right is the per-tick change over the same
window: chatter is the sign alternating every tick, which is what lag-1 measures and what a hand
cannot do. y is inverted on the pitch axes so up on the page is nose up.
"""
import sys, os, re
import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt

n, lam, t0, t1, out = int(sys.argv[1]), float(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
SHARD = 'vy+0.0000_vz+0.0000'
DIRS = [('the corpus we have', 'runs/corpus', '#d62728'),
        ('rebuilt, --mu 1e-4 --limit 85', 'runs/antichatter/shard0', '#2ca02c')]

def read(d):
    p = os.path.join(d, SHARD, f'n{n:04d}_lam{lam:+.6f}.pitches')
    t = open(p).read()
    a = np.array([float(x) for l in t.splitlines() if not l.startswith('#') for x in l.split()])
    h = dict(re.findall(r'^# (\w+)\s+(\S+)', t, re.M))
    return a, float(h.get('dJ', 'nan'))

fig = plt.figure(figsize=(13, 6.6))
gs = fig.add_gridspec(2, 2, height_ratios=[1, 1], hspace=0.35, wspace=0.18)
top = fig.add_subplot(gs[0, :]); zl = fig.add_subplot(gs[1, 0]); zr = fig.add_subplot(gs[1, 1])

for lab, d, col in DIRS:
    a, dJ = read(d)
    top.plot(a, color=col, lw=0.8, label=f'{lab}   dJ {dJ:.3f}   curv {np.abs(np.diff(a,2)).sum():.0f}')
    s = slice(t0, t1)
    zl.plot(range(t0, min(t1, len(a))), a[s], color=col, lw=1.2, marker='.', ms=3)
    dd = np.diff(a)
    zr.plot(range(t0, min(t1, len(dd))), dd[t0:t1], color=col, lw=1.2, marker='.', ms=3)

for ax in (top, zl):
    for y in (-90, 90): ax.axhline(y, color='0.55', lw=0.7, ls=':')
    ax.axhline(0, color='0.8', lw=0.6); ax.set_ylabel('pitch, deg (up = nose up)', fontsize=8)
top.set_ylim(100, -100); top.set_xlim(0, n); top.set_xlabel('tick', fontsize=8)
top.axvspan(t0, t1, color='0.85', zorder=0)
top.legend(fontsize=8, loc='lower right', framealpha=0.9)
top.set_title(f'n {n}, lambda {lam:+.2f}: the whole schedule (shaded = the window below)', fontsize=10)
zl.invert_yaxis(); zl.set_xlabel('tick', fontsize=8)
zl.set_title(f'ticks {t0}-{t1}: the pitch itself', fontsize=10)
zr.axhline(0, color='0.6', lw=0.7)
zr.set_xlabel('tick', fontsize=8); zr.set_ylabel('pitch change per tick, deg', fontsize=8)
zr.set_title('the same window, differenced: chatter is the sign flipping every tick', fontsize=10)
for ax in (top, zl, zr): ax.tick_params(labelsize=7)
fig.savefig(out, dpi=120, bbox_inches='tight'); print(out)
