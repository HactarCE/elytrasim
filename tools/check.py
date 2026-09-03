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

def chatter(path):
    """Count sign-alternating pitch steps that are both large.

    Whole-schedule statistics do not find this. The chatter the optimizer produces is a burst of
    a few dozen ticks in the snap, at 62-68% of the horizon; a mean over the other few hundred
    ticks dilutes it away. Measured at n = 600: TV/tick is 32 inside the burst and 0.58 outside,
    so the average reads 2.5 and looks clean. lag-1 does not find it either -- it was calibrated
    at n = 300 and reads negative on schedules that are visibly smooth elsewhere.
    """
    v = [float(x) for l in open(path) if not l.startswith('#') for x in l.split()]
    d = [v[i+1] - v[i] for i in range(len(v)-1)]
    alt = [i for i in range(len(d)-1)
           if d[i]*d[i+1] < 0 and abs(d[i]) > 5 and abs(d[i+1]) > 5]
    if not alt: return 0, None
    return len(alt), (min(alt)/len(v), (max(alt)+2)/len(v))

chat = {r[0]: chatter(r[0]) for r in rows}
bad  = [r for r in rows if chat[r[0]][0] > 0]
coll = [r for r in rows if r[1].get('structure') == 'COLLAPSED']
multi = [r for r in rows if r[1].get('structure') == 'MULTICYCLE']
l1   = sorted(chat[r[0]][0] for r in rows)
q    = lambda p: l1[min(len(l1)-1, int(p*len(l1)))]

print(f"{len(rows)} profiles under {d}")
print(f"  chatter steps   median {q(.50)}   p90 {q(.90)}   max {l1[-1]}")
print(f"  degenerate (any alternating step pair > 5 deg): {len(bad)}  "
      f"({100*len(bad)/len(rows):.1f}%)")
print(f"  collapsed (left the cyclic branch): {len(coll)}")
print(f"  multi-cycle (more than one cycle in the horizon): {len(multi)}")
if multi:
    print("    " + ", ".join(f"n{m[1].get('n')}/lam{m[1].get('lambda')}" for m in multi[:12])
          + (" ..." if len(multi) > 12 else ""))

by = collections.Counter(os.path.basename(os.path.dirname(r[0])) for r in bad)
if by:
    print("  degenerate by shard:")
    for k, v in sorted(by.items()): print(f"    {k}: {v}")
for f, h in sorted(bad, key=lambda r: -chat[r[0]][0])[:15]:
    c, span = chat[f]
    where = f"{span[0]:.2f}-{span[1]:.2f}" if span else "-"
    print(f"    {c:>4} steps at {where:>9} of the horizon   n {h.get('n'):>4}  "
          f"lam {h.get('lambda'):>7}   {os.path.relpath(f, d)}")
