#!/usr/bin/env python3
"""Quick visual: pitch schedules stacked, plus a zoom and a spectrum. For eyeballing chatter.

usage: look.py out.png [label=]file ...      (label defaults to the basename)
       ZOOM=a,b look.py ...                  zoom window for the middle column
"""
import sys, os, math
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

out = sys.argv[1]
args = sys.argv[2:]
files = []
for a in args:
    lab, _, path = a.rpartition('=')   # rpartition: labels may contain '=' (mu=1e-3)
    if not path: path, lab = lab, os.path.basename(lab).replace('.pitches', '')
    files.append((lab, path))

def load(p):
    return np.array([float(x) for l in open(p) if not l.startswith('#') for x in l.split()])

z0, z1 = (int(x) for x in os.environ.get('ZOOM', '150,200').split(','))

n = len(files)
fig, axes = plt.subplots(n, 3, figsize=(16, 2.0*n + 0.6), squeeze=False,
                         gridspec_kw=dict(width_ratios=[2.4, 1.2, 1.0]))
for i, (lab, path) in enumerate(files):
    p = load(path)
    t = np.arange(len(p))
    ax = axes[i][0]
    ax.plot(t, p, lw=0.7, color='#58a6ff')
    ax.axhline(0, color='#888', lw=0.4)
    ax.set_ylim(95, -95)                      # inverted: up on the page is nose up
    ax.set_ylabel(lab, fontsize=8)
    ax.axvspan(z0, z1, color='#f0a', alpha=0.10)
    tv = np.abs(np.diff(p)).sum()
    d = np.diff(p); l1 = np.corrcoef(d[:-1], d[1:])[0,1] if len(d) > 2 else 0
    ax.text(0.99, 0.06, f'TV {tv:.0f}   lag1 {l1:+.2f}   n {len(p)}',
            ha='right', transform=ax.transAxes, fontsize=7, color='#555')

    ax = axes[i][1]
    s = slice(max(0,z0), min(len(p), z1))
    ax.plot(t[s], p[s], lw=1.0, marker='.', ms=3, color='#f0883e')
    ax.set_ylim(95, -95); ax.axhline(0, color='#888', lw=0.4)

    ax = axes[i][2]
    d = np.diff(p)
    if len(d) > 8:
        f = np.abs(np.fft.rfft(d - d.mean()))**2
        fr = np.fft.rfftfreq(len(d), 1.0)      # cycles per tick; 0.5 is Nyquist = alternation
        ax.semilogy(fr, np.maximum(f, 1e-6), lw=0.7, color='#3fb950')
        ax.set_xlim(0, 0.5)
        hi = f[fr > 0.35].sum() / max(f.sum(), 1e-12)
        ax.text(0.02, 0.06, f'power above 0.35 c/tick: {hi:.0%}',
                transform=ax.transAxes, fontsize=7, color='#555')
    if i == 0:
        axes[i][0].set_title('pitch (deg), nose up = up', fontsize=9)
        axes[i][1].set_title(f'zoom ticks {z0}-{z1}', fontsize=9)
        axes[i][2].set_title('spectrum of per-tick deltas', fontsize=9)
for row in axes:
    for ax in row: ax.tick_params(labelsize=6)
fig.tight_layout()
fig.savefig(out, dpi=110)
print(out)
