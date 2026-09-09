#!/usr/bin/env python3
"""Representative pitch schedules, the corpus we have against the rebuilt one.

    python3 tools/plot_pitch_grid.py runs/antichatter/shard0 runs/corpus out.png

One panel per (n, lambda) cell, the old schedule under the new one. y is inverted so up on the
page is nose up. The point of the figure is that you should be able to fly the green line with a
wrist and not the red one; the numbers under each panel say the same thing arithmetically.
"""
import sys, os, re
import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt

new_dir, old_dir, out = sys.argv[1], sys.argv[2], sys.argv[3]
SHARD = 'vy+0.0000_vz+0.0000'
NS   = [150, 300, 450]
LAMS = [-2.0, 0.0, 1.0, 3.0]

def read(d, n, lam):
    p = os.path.join(d, SHARD, f'n{n:04d}_lam{lam:+.6f}.pitches')
    if not os.path.exists(p): return None
    t = open(p).read()
    a = np.array([float(x) for l in t.splitlines() if not l.startswith('#') for x in l.split()])
    h = dict(re.findall(r'^# (\w+)\s+(\S+)', t, re.M))
    return dict(p=a, dJ=float(h.get('dJ', 'nan')), curv=float(np.abs(np.diff(a, 2)).sum()),
                at90=int((np.abs(a) > 89.9).sum()))

OLD, NEW = '#d62728', '#2ca02c'
fig, axes = plt.subplots(len(LAMS), len(NS), figsize=(4.4 * len(NS), 2.5 * len(LAMS)),
                         squeeze=False)
for r, lam in enumerate(LAMS):
    for c, n in enumerate(NS):
        ax = axes[r][c]
        a, b = read(old_dir, n, lam), read(new_dir, n, lam)
        if a: ax.plot(a['p'], color=OLD, lw=0.7, alpha=0.85, label='the corpus we have')
        if b: ax.plot(b['p'], color=NEW, lw=1.1, label='rebuilt, --mu 1e-4 --limit 85')
        for y in (-90, 90):
            ax.axhline(y, color='0.55', lw=0.7, ls=':')
        ax.axhline(0, color='0.8', lw=0.6)
        ax.set_ylim(100, -100)          # inverted: up on the page is nose up
        ax.set_xlim(0, n)
        ax.set_title(f'n {n}   lambda {lam:+.2f}', fontsize=9)
        if c == 0: ax.set_ylabel('pitch, deg\n(up = nose up)', fontsize=8)
        if r == len(LAMS) - 1: ax.set_xlabel('tick', fontsize=8)
        ax.tick_params(labelsize=7)
        txt = []
        if a: txt.append(f"old  dJ {a['dJ']:7.3f}  curv {a['curv']:6.0f}  @90 {a['at90']:2d}")
        if b: txt.append(f"new  dJ {b['dJ']:7.3f}  curv {b['curv']:6.0f}  @90 {b['at90']:2d}")
        ax.text(0.015, 0.03, '\n'.join(txt), transform=ax.transAxes, va='bottom',
                family='monospace', fontsize=6.5,
                bbox=dict(fc='white', ec='0.8', alpha=0.85, pad=1.6))
fig.suptitle('pitch schedules across the (0,0) shard: dotted lines are the +-90 gate',
             y=0.995, fontsize=12)
h, l = axes[0][0].get_legend_handles_labels()
fig.legend(h, l, loc='upper center', bbox_to_anchor=(0.5, 0.972), ncol=2, fontsize=9,
           frameon=False)
fig.tight_layout(rect=[0, 0, 1, 0.945])
fig.savefig(out, dpi=120); print(out)
