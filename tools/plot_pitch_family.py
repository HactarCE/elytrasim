#!/usr/bin/env python3
"""The rebuilt schedules on their own, as families, against the reference cycle.

    python3 tools/plot_pitch_family.py out.png

Left: one n, the lambda axis. Right: one lambda, the n axis, with the reference cycle in black.
Neighbouring cells should lie on top of each other for as long as they share a phase and then
separate cleanly; a family that frays cell to cell is continuation drift, which is the failure
the pass budget used to hide. y is inverted so up on the page is nose up.
"""
import sys, os, re
import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt

out = sys.argv[1]
D, SHARD = 'runs/antichatter/shard0', 'vy+0.0000_vz+0.0000'

def read(n, lam):
    p = os.path.join(D, SHARD, f'n{n:04d}_lam{lam:+.6f}.pitches')
    return np.array([float(x) for l in open(p) if not l.startswith('#') for x in l.split()])

ref = np.array([float(x) for l in open('runs/veljit/ref300.pitches')
                if not l.startswith('#') for x in l.split()])

fig, (a, b) = plt.subplots(1, 2, figsize=(13.5, 4.6))
lams = [-3.0, -1.5, 0.0, 1.5, 3.0]
for i, lam in enumerate(lams):
    c = plt.cm.viridis(i / (len(lams) - 1))
    a.plot(read(300, lam), color=c, lw=1.1, label=f'lambda {lam:+.1f}')
ns = [200, 250, 300, 350, 400]
for i, n in enumerate(ns):
    c = plt.cm.viridis(i / (len(ns) - 1))
    b.plot(read(n, 0.0), color=c, lw=1.1, label=f'n {n}')
b.plot(ref, color='k', lw=1.4, ls='--', label='reference cycle')

for ax, t in ((a, 'n 300, the lambda axis'), (b, 'lambda 0, the n axis')):
    for y in (-90, 90): ax.axhline(y, color='0.55', lw=0.7, ls=':')
    ax.axhline(0, color='0.85', lw=0.6)
    ax.set_ylim(100, -100); ax.set_xlim(0, None)
    ax.set_title(t + '   (rebuilt, --mu 1e-4 --limit 85)', fontsize=10)
    ax.set_xlabel('tick', fontsize=8); ax.set_ylabel('pitch, deg (up = nose up)', fontsize=8)
    ax.legend(fontsize=8, loc='lower right', ncol=2, framealpha=0.9)
    ax.tick_params(labelsize=7)
fig.savefig(out, dpi=120, bbox_inches='tight'); print(out)
