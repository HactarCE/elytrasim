#!/usr/bin/env python3
"""J against the hand movement it asks for. Each point is a converged optimum at one price."""
import sys, os, glob, re
import numpy as np, matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt

J0 = 0.42750632   # J(s_0) at v0 = (0.167467, 0.200887): the schedules' dJ baseline
def stats(path):
    """dJ from the header when there is one; a bare .pitches file has none, so replay it."""
    txt = open(path).read()
    p = np.array([float(x) for l in txt.splitlines() if not l.startswith('#') for x in l.split()])
    h = dict(re.findall(r'^# (\w+)\s+([-\d.e+]+)', txt, re.M))
    d2 = np.abs(np.diff(p, 2))
    if 'dJ' in h: return float(h['dJ']), d2.sum(), d2.max(), p
    import subprocess
    out = subprocess.run(['./target/release/examples/fragility', path],
                         capture_output=True, text=True).stdout.strip().splitlines()[-1]
    return float(out.split()[1]), d2.sum(), d2.max(), p

pts = []
for f in sorted(glob.glob('runs/antichatter/front/mu*.pitches')):
    mu = float(re.search(r'mu([\d.]+)\.pitches', f).group(1))
    dj, c1, cm, _ = stats(f); pts.append((mu, c1, dj, cm))
pts.sort(key=lambda x: x[1])

fig, ax = plt.subplots(figsize=(8.2, 5.2))
c1 = [p[1] for p in pts]; dj = [p[2] for p in pts]
ax.plot(c1, dj, '-o', ms=5, lw=1.6, color='#58a6ff', label='converged optimum at a price mu', zorder=3)
for mu, x, y, _ in pts:
    ax.annotate(f'{mu:g}', (x, y), fontsize=6, color='#58a6ff',
                textcoords='offset points', xytext=(3, -9))

extra = [('reference cycle (a person flies this)', 'runs/veljit/ref300.pitches', '#3fb950', 'D'),
         ('no price at all (chatters)', 'runs/antichatter/alt/relax1.pitches', '#f85149', 'X')]
for lab, path, col, mk in extra:
    if not os.path.exists(path): continue
    d, a, _, _ = stats(path)
    ax.plot([a], [d], mk, ms=10, color=col, label=lab, zorder=4)
    ax.annotate(lab, (a, d), fontsize=7, color=col, textcoords='offset points', xytext=(-6, 10),
                ha='right' if a > 1000 else 'left')

ax.set_xscale('log')
ax.set_xlabel(r'summed $|$second difference$|$ of pitch, deg/tick$^2$   (what the wrist does)')
ax.set_ylabel('dJ, blocks')
ax.set_title('What the chatter is worth: J against hand movement, every point converged', fontsize=11)
ax.grid(alpha=.25, which='both')
ax.legend(fontsize=8, loc='lower right')
fig.tight_layout(); fig.savefig(sys.argv[1], dpi=115); print(sys.argv[1])
for mu, a, d, cm in pts: print(f"  mu {mu:<9g} curv_l1 {a:8.1f}  curv_max {cm:6.1f}  dJ {d:8.4f}")
