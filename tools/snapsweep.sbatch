#!/bin/bash
#SBATCH --job-name=snapsweep
#SBATCH --partition=camelot
#SBATCH --array=0-10
#SBATCH --nodes=1
#SBATCH --exclusive
#SBATCH --output=logs/%x-%A_%a.out
# One array task per NODE; the node owns all 8 hyperthreads (--exclusive) and fans out locally.
# Each solve is a whole profile on one core -- polish's internal rayon scales to only ~5.7x on
# 10 cores, so one-solve-per-core beats one-solve-using-all-cores by a wide margin.
set -u
export RUN="${RUN:-$HOME/atlas-run}"
cd "$RUN" || exit 1
export SWEEP="$HOME/elytra-atlas/target/release/sweep"
export FLIGHT=algebraic
export RAYON_NUM_THREADS=1
WORK=${WORK:?set WORK to the stage work list}
NSHARD=${SLURM_ARRAY_TASK_COUNT:-11}
ID=${SLURM_ARRAY_TASK_ID:-0}
MINE=$(mktemp)
awk -v n="$NSHARD" -v i="$ID" 'NR % n == i' "$WORK" > "$MINE"
echo "node=$(hostname) shard=$ID/$NSHARD lines=$(wc -l < "$MINE") work=$WORK"
[ -s "$MINE" ] || { echo "nothing to do"; exit 0; }
t0=$(date +%s)
xargs -P 8 -I@ "$HOME/elytra-atlas/tools/snapsweep_solve.sh" "@" < "$MINE"
echo "shard $ID done in $(( $(date +%s) - t0 ))s"
rm -f "$MINE"
