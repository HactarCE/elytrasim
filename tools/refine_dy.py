#!/usr/bin/env python3
"""Print a stride-2 n window bracketing the dy = 0 crossing, for a second pass.

dy = 0 is where a horizon stops being long enough to climb at all, so it is the one place on
the n axis where the answer changes character rather than just scale. Worth more samples.
"""
import sys, os, re, glob
d = sys.argv[1] if len(sys.argv) > 1 else 'runs/corpus'
rows = []
for f in glob.glob(os.path.join(d, 'vy+0.0000_vz+0.0000', 'n*_lam+0.000000.pitches')):
    n = int(re.search(r'n(\d+)_', os.path.basename(f)).group(1))
    dy = None
    for line in open(f):
        if line.startswith('# dy'): dy = float(line.split()[2]); break
        if not line.startswith('#'): break
    if dy is not None: rows.append((n, dy))
rows.sort()
cross = None
for (n0, y0), (n1, y1) in zip(rows, rows[1:]):
    if y0 <= 0.0 < y1 or y0 >= 0.0 > y1:
        cross = (n0, n1); break
if not cross:
    sys.stderr.write("no dy=0 crossing in %d cells\n" % len(rows)); sys.exit(1)
lo, hi = max(60, cross[0]-20), cross[1]+20
sys.stderr.write("dy crosses zero between n=%d and n=%d\n" % cross)
print(','.join(str(n) for n in range(lo, hi+1, 2)))
