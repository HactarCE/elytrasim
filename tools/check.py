#!/usr/bin/env python3
"""Morning check: is the corpus degenerate anywhere?

Reads only the headers, so it is fast over thousands of files. lag1 below 0.2 is chatter;
`collapsed` means the schedule left the cyclic branch entirely, which is a different and
expected outcome at short horizons, not a defect.
"""
import sys, os, glob, collections

d = sys.argv[1] if len(sys.argv) > 1 else 'runs/corpus'
rows = []
for f in glob.glob(os.path.join(d, '**', '*.pitches'), recursive=True):
    h = {}
    for line in open(f):
        if not line.startswith('#'): break
        p = line[1:].split('#')[0].split()
        if len(p) >= 2: h[p[0]] = p[1]
    if 'lag1' in h: rows.append((f, h))

if not rows:
    print(f"no profiles under {d}"); sys.exit(0)

def fl(h, k, d=float('nan')):
    try: return float(h[k])
    except Exception: return d

bad  = [r for r in rows if fl(r[1], 'lag1') < 0.2]
coll = [r for r in rows if r[1].get('structure') == 'collapsed']
l1   = sorted(fl(r[1], 'lag1') for r in rows)
q    = lambda p: l1[min(len(l1)-1, int(p*len(l1)))]

print(f"{len(rows)} profiles under {d}")
print(f"  lag1   min {l1[0]:+.3f}   p10 {q(.10):+.3f}   median {q(.50):+.3f}   "
      f"p90 {q(.90):+.3f}   max {l1[-1]:+.3f}")
print(f"  degenerate (lag1 < 0.2): {len(bad)}  ({100*len(bad)/len(rows):.1f}%)")
print(f"  collapsed (left the cyclic branch): {len(coll)}")

by = collections.Counter(os.path.basename(os.path.dirname(r[0])) for r in bad)
if by:
    print("  degenerate by shard:")
    for k, v in sorted(by.items()): print(f"    {k}: {v}")
for f, h in sorted(bad, key=lambda r: fl(r[1], 'lag1'))[:15]:
    print(f"    {fl(h,'lag1'):+.3f}  n {h.get('n'):>4}  lam {h.get('lambda'):>7}  "
          f"dy {h.get('dy','-'):>10}  {os.path.relpath(f, d)}")
