#!/usr/bin/env python3
"""The cliff at the top of the pitch range, in both trig modes."""
import sys, csv
import numpy as np, matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt
out = sys.argv[1]
panels = [('trig = libm: cos(90) is -4.4e-8, so forward reverses',
           'runs/antichatter/razor_libm.csv', 90.0, 1e6, 'millionths of a degree'),
          ("trig = mth_lut (vanilla): +90 is fine, the table gives +9.6e-5",
           'runs/antichatter/razor_mth.csv', 90.0, 1e6, 'millionths of a degree'),
          ("trig = mth_lut (vanilla): -90 indexes SIN[0] = 0.0, the gate fails",
           'runs/antichatter/razor_mth_neg.csv', -90.0, 1e3, 'thousandths of a degree')]
fig, axes = plt.subplots(1, 3, figsize=(15, 4.0), sharey=True)
for ax, (lab, path, about, scale, unit) in zip(axes, panels):
    r = [x for x in csv.reader(open(path)) if x and x[0] not in ('pitch',) and not x[0].startswith('#')]
    p = np.array([float(a) for a,_,_ in r]); c = np.array([float(b) for _,b,_ in r]); j = np.array([float(x) for _,_,x in r])
    ax.plot((p-about)*scale, j, lw=1.4, color='#58a6ff')
    ax.set_xlabel(f'pitch - ({about:+.0f}) deg, in {unit}')
    ax.set_title(lab, fontsize=9); ax.grid(alpha=.25)
    ax2 = ax.twinx(); ax2.plot((p-about)*scale, c, lw=0.9, color='#f85149', alpha=.75)
    ax2.axhline(0, color='#f85149', lw=0.5, ls=':')
    ax2.set_ylabel('Mth::cos(pitch)', color='#f85149', fontsize=8)
    ax2.tick_params(labelsize=6, colors='#f85149')
axes[0].set_ylabel('dJ of the whole schedule (blocks)')
fig.suptitle('One tick of a schedule, scanned across the end of the pitch range it parks against',
             fontsize=11)
fig.tight_layout(); fig.savefig(out, dpi=115); print(out)
