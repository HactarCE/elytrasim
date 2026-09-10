#!/bin/bash
# Evaluate the substitutions written by tools/atlas_ablate.py, two ways each.
#
#   N=300 LAM=0 VY=0 VZ=0 tools/atlas_ablate.sh <seeddir> <outdir>
#
#   hold     --passes 0: J of the substituted schedule with nothing else allowed to move. The
#            feature's raw contribution.
#   recover  a short unrestricted re-polish. NOT the same as "let the rest adapt while the
#            substitution stands" -- the polish is free to undo the substitution too, so this
#            measures whether the perturbation is recoverable, and where it lands if it is not.
#            Reporting the recovered flick tick alongside J is what makes the difference visible:
#            same flick tick means it fell back into the same optimum, a different one means the
#            feature was selecting the basin.
#
# The budget is deliberately small. These are differences between schedules optimized the same
# way, not certificates, so convergence is not required and is not claimed.
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
cd "$SELF/.." || exit 1
trap 'trap - INT TERM; kill -- -$$ 2>/dev/null; exit 130' INT TERM

SEEDS=${1:?usage: atlas_ablate.sh <seeddir> <outdir>}
OUT=${2:?}
N=${N:-300}; LAM=${LAM:-0}; VY=${VY:-0}; VZ=${VZ:-0}
MU=${MU:-0.0001}; LIMIT=${LIMIT:-85}; TRIG=${TRIG:-mth_lut}
RECOVER_PASSES=${RECOVER_PASSES:-40}; JOBS=${JOBS:-4}
S=./target/release/sweep
CELL="--trig $TRIG --n $N --lambda $LAM --vy $VY --vz $VZ --mu $MU --limit $LIMIT"
mkdir -p "$OUT/recover" || exit 1

jval() { grep -m1 '^n ' | sed 's/.*J \([-0-9.]*\).*/\1/'; }
export -f jval
export S CELL OUT RECOVER_PASSES

one() {
  local f=$1 b hold rec
  b=$(basename "$f" .pitches)
  hold=$($S polish $CELL --passes 0 --tol 0 --init "$f" 2>&1 | jval)
  $S polish $CELL --passes "$RECOVER_PASSES" --tol 0.002 --init "$f" \
     --out "$OUT/recover/$b.pitches" > /dev/null 2>&1
  rec=$(grep -m1 '^# dJ' "$OUT/recover/$b.pitches" 2>/dev/null | awk '{print $3}')
  printf '%s\t%s\t%s\n' "$b" "${hold:-NA}" "${rec:-NA}"
}
export -f one

ls "$SEEDS"/*.pitches | xargs -P "$JOBS" -I{} bash -c 'one "$@"' _ {} | sort > "$OUT/raw.tsv"
echo "wrote $OUT/raw.tsv ($(wc -l < "$OUT/raw.tsv" | tr -d ' ') rows)"
