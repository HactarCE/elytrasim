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
# Resolve the repo from this file's location, following a symlink, and refuse to run from a
# copy that has been moved elsewhere -- a copy in a scratch directory silently cd's to the wrong
# tree and every seed then fails its existence check, leaving an empty run and no error.
SELF=$(cd "$(dirname "$0")" && pwd)
cd "$SELF/.." || exit 1
[ -x ./target/release/sweep ] || { echo "no ./target/release/sweep under $PWD" >&2; exit 1; }
OUT=${1:?usage: flyable.sh <outdir> <mu> [extra sweep args...]}; MU=${2:?}; shift 2
# The cell comes from the environment, not from repeated flags: `sweep` resolves an option to its
# *first* occurrence, so appending `--lambda -1` after a default `--lambda 0` silently keeps the
# 0. Defaults are the cell every number in README-control.md was measured on.
#
#   N=150 LAM=-1 VY=0 VZ=0 tools/flyable.sh out 1e-4
CELL="--trig ${TRIG:-mth_lut} --n ${N:-300} --lambda ${LAM:-0}"
CELL="$CELL --vy ${VY:-0.167467} --vz ${VZ:-0.200887} $*"
S=./target/release/sweep
mkdir -p "$OUT/relax" "$OUT/cand"

echo "== stage 1: relax (no price) =="
i=0
for seed in ${SEEDS:-runs/veljit/ref300.pitches runs/jitter/sigma0.pitches runs/antichatter/mth/raw_chat.pitches}; do
  [ -f "$seed" ] || { echo "  missing seed $seed" >&2; continue; }
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
