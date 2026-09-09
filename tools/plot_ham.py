#!/usr/bin/env python3
"""Per-tick objective curves J(p): is this a normal arc (one peak) or a singular arc (two)?"""
import sys, csv
import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt

out = sys.argv[1]; files = sys.argv[2:]
fig, axes = plt.subplots(1, len(files), figsize=(5.2*len(files), 4.2), squeeze=False)
for k, spec in enumerate(files):
    lab, _, path = spec.partition('=')
    rows = list(csv.reader(open(path)))
    hdr = rows[0]; d = np.array([[float(x) for x in r] for r in rows[1:]])
    p = d[:,0]
    ax = axes[0][k]
    for j in range(1, d.shape[1]):
        y = d[:,j]
        ax.plot(p, y - y.max(), lw=0.9, label=hdr[j])
    ax.set_ylim(-0.35, 0.02); ax.set_xlim(-90, 90)
    ax.axvline(0, color='#888', lw=0.5); ax.axvline(90, color='#888', lw=0.5)
    ax.set_xlabel('pitch at tick t (deg, + = nose down)')
    ax.set_ylabel('J - max J  (blocks)')
    ax.set_title(lab, fontsize=10)
    ax.legend(fontsize=6, ncol=2)
    ax.grid(alpha=.2)
fig.tight_layout(); fig.savefig(out, dpi=115); print(out)
