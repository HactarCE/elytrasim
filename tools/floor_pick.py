#!/usr/bin/env python3
"""Pick the best `floor exit` solve per (mode, y0) from a pulled cluster run.

    tools/floor_pick.py <run>/out [--install runs/floor]

Scored on the mode's own quantity, t* or z(t*), read from each file's header. A schedule that
survived its cap has only a value-to-go guess there, so it is flagged, not trusted. With
--install the winners overwrite runs/floor/exit_<mode>_y<y0>.pitches, the files the vis reads.
"""
import re, shutil, sys
from collections import defaultdict
from pathlib import Path

HEAD = re.compile(r"t\* ([\d.+-]+)\s+z\(t\*\) ([\d.+-]+)\s+survived (\d+)")
CAP = re.compile(r"cap (\d+)")

def read(path):
    lines = path.read_text().splitlines()[:2]
    t, z, k = HEAD.search(lines[1]).groups()
    return float(t), float(z), int(k), int(CAP.search(lines[0]).group(1))

def main():
    out = Path(sys.argv[1])
    install = Path(sys.argv[sys.argv.index("--install") + 1]) if "--install" in sys.argv else None
    cells = defaultdict(dict)
    for f in out.glob("*.pitches"):
        mode, y, seed = re.match(r"(time|dist)_y(\d+)_(.+)\.pitches", f.name).groups()
        cells[(mode, int(y))][seed] = (f, *read(f))
    for mode in ("time", "dist"):
        seeds = sorted({s for (m, _), d in cells.items() if m == mode for s in d})
        col = 1 if mode == "time" else 2
        print(f"\n{mode}: best per y0 (* = survived its cap, value-to-go guess)")
        print(f"{'y0':>3} " + " ".join(f"{s:>10}" for s in seeds) + f"   {'winner':>10}  {'old':>9}")
        for y in sorted(y for (m, y) in cells if m == mode):
            d = cells[(mode, y)]
            row, best = [], max(d.items(), key=lambda kv: kv[1][col])
            for s in seeds:
                if s not in d: row.append(f"{'-':>10}"); continue
                v = d[s]
                row.append(f"{v[col]:9.3f}{'*' if v[3] >= v[4] else ' '}")
            old = Path(f"runs/floor/exit_{mode}_y{y}.pitches")
            ov = f"{read(old)[col - 1]:9.3f}" if old.exists() else f"{'-':>9}"
            print(f"{y:>3} " + " ".join(row) + f"   {best[0]:>10}  {ov}")
            if install:
                shutil.copyfile(best[1][0], install / f"exit_{mode}_y{y}.pitches")

if __name__ == "__main__":
    main()
