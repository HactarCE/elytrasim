#!/usr/bin/env python3
"""Two corpora over the same (n, lambda) plane, side by side.

    python3 tools/compare.py runs/antichatter/shard0 runs/corpus out.png

Left column is the old grid, right is the new one, rows are the three things that say whether a
cell is flyable: summed |second difference| (what a wrist does), the lag-1 degeneracy statistic,
and whether the cell parks a pitch against the +-90 gate. The fourth row is what it cost in J.
Cells present in only one corpus are left blank rather than filled in.
"""
import sys, os, re, glob
import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LogNorm, TwoSlopeNorm

new_dir, old_dir, out = sys.argv[1], sys.argv[2], sys.argv[3]

def cells(d):
    r = {}
    for f in glob.glob(os.path.join(d, '*', '*.pitches')):
        m = re.search(r'n(\d+)_lam([-+][\d.]+)\.pitches$', f)
        if not m: continue
        n, lam = int(m.group(1)), float(m.group(2))
        t = open(f).read()
        p = np.array([float(x) for l in t.splitlines() if not l.startswith('#') for x in l.split()])
        if len(p) < 4: continue
        d1, d2 = np.diff(p), np.abs(np.diff(p, 2))
        h = dict(re.findall(r'^# (\w+)\s+(\S+)', t, re.M))
        r[(n, lam)] = dict(curv=d2.sum(), lag1=float(np.corrcoef(d1[:-1], d1[1:])[0, 1]),
                           at90=int((np.abs(p) > 89.9).sum()),
                           dJ=float(h.get('dJ', 'nan')))
    return r

A, B = cells(old_dir), cells(new_dir)
ns = sorted({n for n, _ in set(A) | set(B)})
lams = sorted({l for _, l in set(A) | set(B)})

def grid(src, key):
    g = np.full((len(ns), len(lams)), np.nan)
    for i, n in enumerate(ns):
        for j, l in enumerate(lams):
            if (n, l) in src: g[i, j] = src[(n, l)][key]
    return g

rows = [('summed |2nd difference|, deg/tick^2', 'curv', LogNorm(vmin=30, vmax=12000), 'magma_r'),
        ('lag-1 of the per-tick changes',       'lag1', TwoSlopeNorm(0.2, -1, 1), 'RdYlGn'),
        ('ticks parked past |pitch| 89.9',      'at90', LogNorm(vmin=0.5, vmax=40), 'magma_r')]
fig, axes = plt.subplots(len(rows) + 1, 2, figsize=(13, 3.0 * (len(rows) + 1)), squeeze=False)
ext = [lams[0], lams[-1], ns[0], ns[-1]]
for r, (title, key, norm, cmap) in enumerate(rows):
    for c, (lab, src) in enumerate([('the corpus we have', A), ('rebuilt with --mu 1e-4 --limit 85', B)]):
        ax = axes[r][c]
        im = ax.imshow(grid(src, key), aspect='auto', origin='lower', extent=ext,
                       norm=norm, cmap=cmap, interpolation='nearest')
        ax.set_title(f'{lab}: {title}', fontsize=8)
        ax.set_ylabel('n'); ax.set_xlabel('lambda')
        fig.colorbar(im, ax=ax, fraction=0.03)
# the cost, as a difference
ax = axes[-1][0]
d = grid(B, 'dJ') - grid(A, 'dJ')
lim = np.nanpercentile(np.abs(d), 98) or 1.0
im = ax.imshow(d, aspect='auto', origin='lower', extent=ext, cmap='RdBu',
               norm=TwoSlopeNorm(0, -lim, lim), interpolation='nearest')
ax.set_title('dJ(new) - dJ(old), blocks. blue = the rebuilt cell scores higher', fontsize=8)
ax.set_ylabel('n'); ax.set_xlabel('lambda'); fig.colorbar(im, ax=ax, fraction=0.03)
axes[-1][1].axis('off')
both = [k for k in A if k in B]
if both:
    dd = np.array([B[k]['dJ'] - A[k]['dJ'] for k in both])
    txt = (f"{len(both)} cells in both\n\n"
           f"dJ:      new higher in {int((dd > 0).sum())}, lower in {int((dd < 0).sum())}\n"
           f"         median change {np.median(dd):+.3f} blocks\n\n"
           f"curv_l1: median {np.median([A[k]['curv'] for k in both]):.0f}"
           f"  ->  {np.median([B[k]['curv'] for k in both]):.0f}\n"
           f"lag1:    median {np.median([A[k]['lag1'] for k in both]):+.2f}"
           f"  ->  {np.median([B[k]['lag1'] for k in both]):+.2f}\n"
           f"at +-90: {100*np.mean([A[k]['at90'] > 0 for k in both]):.0f}%"
           f"  ->  {100*np.mean([B[k]['at90'] > 0 for k in both]):.0f}% of cells")
    axes[-1][1].text(0.02, 0.95, txt, va='top', family='monospace', fontsize=9)
fig.tight_layout(); fig.savefig(out, dpi=110); print(out)
