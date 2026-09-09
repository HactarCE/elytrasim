#!/usr/bin/env python3
"""What shape is a corpus actually in? Standard library plus numpy; runs on any sweep output.

    python3 tools/audit.py runs/corpus

Reports, per shard and overall: the curvature statistics a roughness price is levied on, the
lag-1 degeneracy statistic, and the fraction of cells that park a pitch against the +-90 gate --
which `lag1` cannot see and which costs more than the chatter does. See README-control.md.
"""
import sys, glob, os, statistics as st
import numpy as np

REF = dict(curv_l1=147.1, curv_max=37.9, lag1=0.479, tv=247.9)   # the reference cycle, for scale

def stats(path):
    p = np.array([float(x) for l in open(path) if not l.startswith('#') for x in l.split()])
    d, d2 = np.diff(p), np.abs(np.diff(p, 2))
    return dict(curv_l1=d2.sum(), curv_max=d2.max(), tv=np.abs(d).sum(),
                lag1=float(np.corrcoef(d[:-1], d[1:])[0, 1]),
                at90=int((np.abs(p) > 89.9).sum()))

root = sys.argv[1] if len(sys.argv) > 1 else 'runs/corpus'
# Both layouts at once: a sweep writes one directory per shard, but a hand-built axis is
# usually flat, and taking only the first non-empty glob silently hid the flat half.
files = sorted(set(glob.glob(os.path.join(root, '*', '*.pitches'))) |
               set(glob.glob(os.path.join(root, '*.pitches'))))
if not files: sys.exit(f'no .pitches under {root}')

rows = {}
for f in files:
    d = os.path.dirname(f)
    rows.setdefault('.' if os.path.abspath(d) == os.path.abspath(root)
                    else os.path.basename(d), []).append(stats(f))

hdr = f"{'shard':<24} {'cells':>5} {'curv_l1':>9} {'curv_max':>9} {'lag1':>7} {'at +-90':>8}"
print(hdr); print('-' * len(hdr))
for k in sorted(rows) + ['ALL']:
    v = [x for vs in rows.values() for x in vs] if k == 'ALL' else rows[k]
    print(f"{k:<24} {len(v):>5} {st.median(x['curv_l1'] for x in v):>9.0f} "
          f"{st.median(x['curv_max'] for x in v):>9.0f} {st.median(x['lag1'] for x in v):>+7.2f} "
          f"{100 * sum(1 for x in v if x['at90']) / len(v):>7.0f}%")
print('-' * len(hdr))
print(f"{'reference cycle':<24} {1:>5} {REF['curv_l1']:>9.0f} {REF['curv_max']:>9.0f} "
      f"{REF['lag1']:>+7.2f} {0:>7}%")
print("\nmedians. `at +-90` is the fraction of cells holding at least one pitch within 0.1 deg of\n"
      "the gate, where look_hor_length underflows and the aerodynamics switch off; a cell can\n"
      "read a healthy lag1 and still be there. See README-control.md.")
