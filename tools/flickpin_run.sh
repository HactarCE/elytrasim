#!/bin/bash
# J*(t) for a flick the optimizer would not choose: pin the flick tick and optimize everything
# else around it.
#
#   tools/flickpin_run.sh <seeddir> <outdir> [lo] [hi]
#
# The free sweep (runs/atlas/cells/flick_v00_n300) cannot answer this. Seeded past tick 216 it
# slides the flick back to 215-216 and piles up there -- 35 of its 50 post-peak profiles sit on
# those two ticks -- so the whole region beyond the wall is unsampled, and "unsampled" has been
# read off the figure as "bad". --flick-at makes the late flick a *constraint* instead of a
# starting guess, which is the only way to price it.
#
# The pin is the tick where the schedule first reaches --flick-pitch, which is NOT the seed's
# nominal flick_tick: the seeds ramp into the snap over about five ticks, so seed tight_t0240
# first crosses -80 at 245. Pin to what the seed actually does, or the seed starts infeasible
# and the first thing the optimizer does is move the flick.
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
cd "$SELF/.." || exit 1
[ -x ./target/release/sweep ] || { echo "no ./target/release/sweep under $PWD" >&2; exit 1; }

SEEDS=${1:?usage: flickpin_run.sh <seeddir> <outdir> [lo] [hi]}
OUT=${2:?}
LO=${3:-160}; HI=${4:-290}
N=${N:-300}; LAM=${LAM:-0}; VY=${VY:-0}; VZ=${VZ:-0}
MU=${MU:-0.0001}; LIMIT=${LIMIT:-85}; PASSES=${PASSES:-400}; TOL=${TOL:-0.002}
TRIG=${TRIG:-mth_lut}; JOBS=${JOBS:-8}; FPITCH=${FPITCH:--80}

trap 'trap - INT TERM EXIT; kill -- -$$ 2>/dev/null; exit 130' INT TERM

mkdir -p "$OUT" || exit 1
LOG="$OUT/flickpin.log"
CELL="--trig $TRIG --n $N --lambda $LAM --vy $VY --vz $VZ"
OPTS="--passes $PASSES --tol $TOL --mu $MU --limit $LIMIT"
{
  echo "# flickpin $(date '+%Y-%m-%d %H:%M:%S %Z')"
  echo "# cell   $CELL"
  echo "# opts   $OPTS  --flick-pitch $FPITCH"
  echo "# seeds  $SEEDS  pinned ticks $LO..$HI  jobs $JOBS"
} >> "$LOG"

export CELL OPTS OUT LOG FPITCH
solve() {
  local f=$1 t=$2 b="t$2"
  if [ -s "$OUT/$b.pitches" ] && grep -q '^# certified' "$OUT/$b.pitches" 2>/dev/null; then return 0; fi
  local t0 line
  t0=$(date +%s)
  line=$(./target/release/sweep polish $CELL $OPTS --flick-at "$t" --flick-pitch "$FPITCH" \
           --init "$f" --out "$OUT/$b.pitches" 2>&1 | tail -1)
  printf '%-8s %s  [%ss]\n' "$b" "$line" "$(( $(date +%s) - t0 ))" >> "$LOG"
}
export -f solve

# Emit "<seedfile> <pin tick>", one per distinct pin tick in range, nearest seed wins.
python3 - "$SEEDS" "$LO" "$HI" "$FPITCH" <<'PY' | xargs -P "$JOBS" -n 2 bash -c 'solve "$0" "$1"'
import glob, os, sys
seeds, lo, hi, fp = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4])
seen = {}
for f in sorted(glob.glob(f"{seeds}/*.pitches")):
    p = [float(x) for l in open(f) for x in l.split("#")[0].split()]
    t = next((i for i, v in enumerate(p) if v <= fp), None)
    if t is None or not lo <= t <= hi or t in seen:
        continue
    seen[t] = f
for t in sorted(seen):
    print(seen[t], t)
PY
echo "flickpin done: $(ls "$OUT"/*.pitches 2>/dev/null | wc -l | tr -d ' ') profiles" >&2
