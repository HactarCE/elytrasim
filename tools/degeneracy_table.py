#!/usr/bin/env python3
"""Did a non-degenerate profile degenerate under polishing?

Degeneracy is high-frequency structure, not amplitude: a real flick is one big step with its
neighbours agreeing, chatter is many big steps alternating sign. Total variation alone cannot
tell them apart, so the lag-1 correlation of the per-tick deltas carries the verdict.
"""
import sys, os, statistics as st

def load(path):
    hdr, vals = {}, []
    for line in open(path):
        if line.startswith('#'):
            parts = line[1:].split('#')[0].split()
            if len(parts) >= 2: hdr[parts[0]] = ' '.join(parts[1:])
        else:
            vals += [float(x) for x in line.split()]
    return hdr, vals

def stats(f):
    d = [f[i+1] - f[i] for i in range(len(f)-1)]
    m = st.mean(d)
    den = sum((x-m)**2 for x in d)
    lag1 = sum((d[i]-m)*(d[i+1]-m) for i in range(len(d)-1)) / den if den else float('nan')
    alt = sum(1 for i in range(len(d)-1) if d[i]*d[i+1] < 0 and abs(d[i]) > 5 and abs(d[i+1]) > 5)
    return dict(tv=sum(abs(x) for x in d), lag1=lag1, alt=alt,
                big=sum(1 for x in d if abs(x) > 20), lo=min(f), hi=max(f))

print(f"{'profile':<10} {'sigma':>6} {'TV':>8} {'TV/tick':>8} {'lag-1':>7} {'alt>5':>6} "
      f"{'|d|>20':>7} {'dy':>8} {'dz':>8} {'dJ':>8}")
print('-' * 84)
for path in sys.argv[1:]:
    if not os.path.exists(path): continue
    hdr, f = load(path)
    s = stats(f)
    sig = hdr.get('jitter', '-').split()[0]
    name = os.path.basename(path).replace('.pitches', '')
    def g(k):
        try: return f"{float(hdr[k]):8.2f}"
        except Exception: return f"{'-':>8}"
    print(f"{name:<10} {sig:>6} {s['tv']:8.1f} {s['tv']/(len(f)-1):8.3f} {s['lag1']:+7.3f} "
          f"{s['alt']:6d} {s['big']:7d} {g('dy')} {g('dz')} {g('dJ')}")
