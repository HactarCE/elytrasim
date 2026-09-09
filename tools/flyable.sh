#!/bin/bash
# Relax, project, polish -- the recipe from README-control.md, with the multi-start the
# projection needs.
#
#   tools/flyable.sh <outdir> <mu> [extra sweep args...]
#
# Stage 1 relaxes from three seeds with no price on the control at all. Stage 2 projects each
# relaxed optimum at several widths with both filters and polishes each under the price and the
# pitch margin. Stage 3 prints every candidate ranked by J - mu*curv_l1, which is the objective
# actually being optimized, and links the winner to <outdir>/best.pitches.
#
# The multi-start is not belt and braces: a projection width that is right for one relaxed
# optimum lands 6.5 blocks low on another, and the relaxed schedule does not tell you which.
set -u
cd "$(dirname "$0")/.." || exit 1
OUT=${1:?usage: flyable.sh <outdir> <mu> [sweep cell args...]}; MU=${2:?}; shift 2
# Everything after <mu> is passed through, so the cell is the caller's: pass --n/--lambda/--vy/
# --vz to move it. The defaults here are the cell every number in README-control.md was measured
# on, and they lose to anything the caller repeats later on the command line.
CELL="--trig mth_lut --n 300 --vy 0.167467 --vz 0.200887 --lambda 0 $*"
S=./target/release/sweep
mkdir -p "$OUT/relax" "$OUT/cand"

echo "== stage 1: relax (no price) =="
i=0
for seed in ${SEEDS:-runs/veljit/ref300.pitches runs/jitter/sigma0.pitches runs/antichatter/mth/raw_chat.pitches}; do
  [ -f "$seed" ] || continue
  i=$((i+1))
  $S polish $CELL --passes 200 --tol 0.005 --init "$seed" --out "$OUT/relax/r$i.pitches" \
     2>&1 | sed "s/^/  r$i /"
done

echo "== stage 2: project and polish =="
for r in "$OUT"/relax/r*.pitches; do
  b=$(basename "$r" .pitches)
  for k in 3 5 9 15; do
    $S polish $CELL --passes 200 --tol 0.002 --init "$r" --presmooth $k --mu "$MU" --limit 85 \
       --out "$OUT/cand/${b}_box$k.pitches" 2>/dev/null &
    $S polish $CELL --passes 200 --tol 0.002 --init "$r" --premedian $k --mu "$MU" --limit 85 \
       --out "$OUT/cand/${b}_med$k.pitches" 2>/dev/null &
  done
  wait
done

echo "== stage 3: rank on J - mu*curv_l1 =="
python3 - "$OUT" "$MU" <<'PY'
import sys, glob, os, re
out, mu = sys.argv[1], float(sys.argv[2])
rows = []
for f in sorted(glob.glob(f'{out}/cand/*.pitches')):
    h = dict(re.findall(r'^# (\w+)\s+(\S+)', open(f).read(), re.M))
    dj, c1 = float(h['dJ']), float(h['curv_l1'])
    rows.append((dj - mu * c1, dj, c1, float(h['curv_max']), float(h['lag1']), f))
rows.sort(reverse=True)
print(f"  {'score':>9} {'dJ':>9} {'curv_l1':>8} {'curv_max':>9} {'lag1':>6}  candidate")
for s, dj, c1, cm, l1, f in rows:
    print(f"  {s:>9.4f} {dj:>9.4f} {c1:>8.1f} {cm:>9.1f} {l1:>+6.2f}  {os.path.basename(f)}")
if rows:
    best = rows[0][5]
    dst = f'{out}/best.pitches'
    if os.path.exists(dst): os.remove(dst)
    os.link(best, dst)
    print(f"\n  best: {os.path.basename(best)} -> {dst}")
PY
