#!/bin/bash
# Polish every seed in a directory at one cell, under the unchanged priced objective, and keep
# every converged result.
#
#   tools/atlas_run.sh <seeddir> <outdir>
#
# The cell comes from the environment, not from flags: `sweep` resolves an option to its *first*
# occurrence, so a flag appended after a default is silently ignored.
#
#   N=300 LAM=0 VY=0 VZ=0 MU=0.0001 LIMIT=85 PASSES=400 JOBS=5 tools/atlas_run.sh seeds out
#
# This is deliberately not `flyable.sh`: that ranks its candidates and links one winner. Here
# nothing is discarded -- the whole point is the distribution of optima, and the ones that lose
# are the ones that say what the landscape looks like.
#
# Resumes by file existence, so a killed run costs one solve.
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
cd "$SELF/.." || exit 1
[ -x ./target/release/sweep ] || { echo "no ./target/release/sweep under $PWD" >&2; exit 1; }

SEEDS=${1:?usage: atlas_run.sh <seeddir> <outdir>}
OUT=${2:?}
N=${N:-300}; LAM=${LAM:-0}; VY=${VY:-0}; VZ=${VZ:-0}
MU=${MU:-0.0001}; LIMIT=${LIMIT:-85}; PASSES=${PASSES:-400}; TOL=${TOL:-0.002}
TRIG=${TRIG:-mth_lut}; JOBS=${JOBS:-5}

# `xargs -P` keeps respawning as its children die, and if this script is killed the xargs is
# reparented to init and carries on solving -- so killing the job becomes a hunt through ps.
# Take the whole process group down instead.
trap 'trap - INT TERM EXIT; kill -- -$$ 2>/dev/null; exit 130' INT TERM

ls "$SEEDS"/*.pitches >/dev/null 2>&1 || { echo "no seeds in $SEEDS" >&2; exit 1; }
mkdir -p "$OUT" || exit 1
LOG="$OUT/atlas.log"

CELL="--trig $TRIG --n $N --lambda $LAM --vy $VY --vz $VZ"
OPTS="--passes $PASSES --tol $TOL --mu $MU --limit $LIMIT"
{
  echo "# atlas $(date '+%Y-%m-%d %H:%M:%S %Z')"
  echo "# cell   $CELL"
  echo "# opts   $OPTS"
  echo "# seeds  $SEEDS  ($(ls "$SEEDS"/*.pitches | wc -l | tr -d ' ') files)  jobs $JOBS"
} >> "$LOG"

export CELL OPTS OUT LOG
solve() {
  local f=$1 b
  b=$(basename "$f" .pitches)
  # Resume: a finished cell already has a header with a residual in it.
  if [ -s "$OUT/$b.pitches" ] && grep -q '^# certified' "$OUT/$b.pitches" 2>/dev/null; then
    return 0
  fi
  local t0 line
  t0=$(date +%s)
  line=$(./target/release/sweep polish $CELL $OPTS --init "$f" --out "$OUT/$b.pitches" 2>&1 | tail -1)
  printf '%-28s %s  [%ss]\n' "$b" "$line" "$(( $(date +%s) - t0 ))" >> "$LOG"
}
export -f solve

# The policy seed has no file: `sweep` flies it itself when --init is absent.
if ! { [ -s "$OUT/policy_leak.pitches" ] && grep -q '^# certified' "$OUT/policy_leak.pitches"; }; then
  ./target/release/sweep polish $CELL $OPTS --out "$OUT/policy_leak.pitches" 2>&1 | tail -1 \
    | sed 's/^/policy_leak                   /' >> "$LOG"
fi

ls "$SEEDS"/*.pitches | xargs -P "$JOBS" -I{} bash -c 'solve "$@"' _ {}

done_n=$(ls "$OUT"/*.pitches 2>/dev/null | wc -l | tr -d ' ')
echo "# done $done_n profiles $(date '+%H:%M:%S')" >> "$LOG"
echo "atlas: $done_n profiles in $OUT"
