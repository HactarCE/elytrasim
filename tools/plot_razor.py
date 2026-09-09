#!/usr/bin/env python3
"""The cliff at the top of the pitch range, in both trig modes."""
import sys, csv
import numpy as np, matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt
out = sys.argv[1]
fig, axes = plt.subplots(1, 2, figsize=(11, 3.8), sharey=True)
for ax, (lab, path) in zip(axes, [('trig = libm (f32 cos)', 'runs/antichatter/razor_libm.csv'),
                                  ("trig = mth_lut (vanilla's table)", 'runs/antichatter/razor_mth.csv')]):
    r = [x for x in csv.reader(open(path)) if x and x[0] not in ('pitch',) and not x[0].startswith('#')]
    p = np.array([float(a) for a,_,_ in r]); c = np.array([float(b) for _,b,_ in r]); j = np.array([float(x) for _,_,x in r])
    ax.plot((p-90)*1e6, j, lw=1.2, color='#58a6ff')
    ax.set_xlabel('pitch - 90 deg, in millionths of a degree')
    ax.set_title(lab, fontsize=10); ax.grid(alpha=.25)
    ax2 = ax.twinx(); ax2.plot((p-90)*1e6, c, lw=0.8, color='#f85149', alpha=.7)
    ax2.axhline(0, color='#f85149', lw=0.5, ls=':'); ax2.set_ylabel('cos(pitch)', color='#f85149', fontsize=8)
    ax2.tick_params(labelsize=6, colors='#f85149')
axes[0].set_ylabel('dJ of the whole schedule (blocks)')
fig.suptitle('One tick of a chattering schedule, scanned across the top of the pitch range', fontsize=10)
fig.tight_layout(); fig.savefig(out, dpi=115); print(out)
