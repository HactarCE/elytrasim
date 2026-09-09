#!/usr/bin/env python3
"""dJ against pitch noise. Smooth is not the same as robust, and this is the picture of that."""
import sys, subprocess
import numpy as np, matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt

rows = [('reference cycle (a person)', 'runs/veljit/ref300.pitches', '#3fb950', 'o'),
        ('jitter + --lag1-floor 0.2 (today)', 'runs/antichatter/old/old8.pitches', '#d29922', 's'),
        ('--mu 1e-4 --limit 85, fixed point (this)', 'runs/antichatter/fix/i8.pitches', '#58a6ff', 'D'),
        ('no price, no limit (relaxed)', 'runs/antichatter/alt/relax1.pitches', '#f85149', 'X')]
amps = [1e-4, 1e-3, 1e-2, 0.05, 0.15, 0.5]
out = subprocess.run(['./target/release/examples/sens', '--trig', 'mth_lut'] + [r[1] for r in rows],
                     capture_output=True, text=True).stdout.strip().splitlines()
data = {}
for line in out[1:]:
    f = line.split()
    data[f[0]] = (float(f[1]), [float(f[2 + 3*i]) for i in range(len(amps))],
                               [float(f[3 + 3*i]) for i in range(len(amps))])

fig, ax = plt.subplots(figsize=(8.6, 5.0))
for lab, path, col, mk in rows:
    key = path.rsplit('/', 1)[-1]
    if key not in data: continue
    base, mean, p05 = data[key]
    ax.plot(amps, mean, '-'+mk, color=col, lw=1.6, ms=5, label=lab)
    ax.fill_between(amps, p05, mean, color=col, alpha=.18, lw=0)
ax.set_xscale('log'); ax.set_ylim(-5, 22.5)
ax.axhline(0, color='#888', lw=0.6)
ax.set_xlabel('uniform pitch noise on every tick, amplitude in degrees')
ax.set_ylabel('dJ, blocks')
ax.text(0.012, 0.985, 'line: mean of 200 draws   band: down to the 5th percentile',
        transform=ax.transAxes, va='top', fontsize=7.5, color='#666')
ax.set_title('Minecraft delivers rotation in steps of about 0.15 * sensitivity degrees', fontsize=10)
ax.axvspan(0.05, 0.5, color='#888', alpha=.10)
ax.annotate('roughly where a real hand lives', (0.16, -3.4), fontsize=8, color='#666', ha='center')
ax.grid(alpha=.25, which='both'); ax.legend(fontsize=8, loc='lower left')
fig.tight_layout(); fig.savefig(sys.argv[1], dpi=115); print(sys.argv[1])
