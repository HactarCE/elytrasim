#!/usr/bin/env python3
"""Extend the first-exit optimum corpus through y0=32 with multi-start searches."""

import argparse
import re
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FLOOR = ROOT / "target/release/floor"
TILE = ROOT.parent / "myopic-metrics/runs/steady/nlamsweep/out/n0150_lamP0/tight_t0100.pitches"
HEADER = re.compile(r"t\* ([\d.+-]+)\s+z\(t\*\) ([\d.+-]+)")


def solve(y0, mode, seed, passes, out):
    cmd = [str(FLOOR), "exit", "--y0", str(y0), "--mode", mode, "--n", "450",
           "--init", seed, "--passes", str(passes), "--out", str(out)]
    result = subprocess.run(cmd, cwd=ROOT, check=True, capture_output=True, text=True)
    match = HEADER.search(result.stdout)
    if not match:
        raise RuntimeError(f"could not read result from: {result.stdout}")
    t_exit, z_exit = map(float, match.groups())
    return t_exit if mode == "time" else z_exit, result.stdout.strip()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--from-y", type=int, default=9)
    parser.add_argument("--to-y", type=int, default=32)
    parser.add_argument("--passes", type=int, default=30)
    args = parser.parse_args()
    if not FLOOR.exists():
        subprocess.run(["cargo", "build", "--release", "--bin", "floor"], cwd=ROOT, check=True)
    if not TILE.exists():
        raise FileNotFoundError(TILE)

    with tempfile.TemporaryDirectory(prefix="floor-profiles-") as temporary:
        temp = Path(temporary)
        for mode, stem in [("time", "exit_time"), ("dist", "exit_dist")]:
            previous = ROOT / f"runs/floor/{stem}_y{args.from_y - 1}.pitches"
            for y0 in range(args.from_y, args.to_y + 1):
                candidates = [("hold:-13.052", 100), (f"tile:{TILE}", args.passes)]
                if previous.exists():
                    candidates.append((str(previous), 100))
                scored = []
                for index, (seed, passes) in enumerate(candidates):
                    path = temp / f"{mode}-y{y0}-{index}.pitches"
                    score, summary = solve(y0, mode, seed, passes, path)
                    scored.append((score, path, summary))
                score, path, summary = max(scored, key=lambda row: row[0])
                destination = ROOT / f"runs/floor/{stem}_y{y0}.pitches"
                shutil.copyfile(path, destination)
                previous = destination
                print(f"{mode:4} y0={y0:2}: {score:9.4f}  {summary.splitlines()[-1]}", flush=True)


if __name__ == "__main__":
    main()
