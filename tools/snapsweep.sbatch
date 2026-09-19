#!/bin/bash
#SBATCH --job-name=snapsweep
#SBATCH --partition=camelot
#SBATCH --nodes=1
#SBATCH --exclusive
#SBATCH --output=logs/%x-%A_%a.out
# One array task per NODE; the node owns all 8 hyperthreads (--exclusive) and fans out locally.
# There is deliberately no `#SBATCH --array` here: the shard count must match the number of
# nodes that will run, so it is computed at submit time by tools/snapsweep_submit.sh. Submit
# through that, not with a bare sbatch.
# Each solve is a whole profile on one core -- polish's internal rayon scales to only ~5.7x on
# 10 cores, so one-solve-per-core beats one-solve-using-all-cores by a wide margin.
set -u
export RUN="${RUN:-$HOME/atlas-run}"
cd "$RUN" || exit 1
export SWEEP="$HOME/elytra-atlas/target/release/sweep"
export FLIGHT=algebraic
export RAYON_NUM_THREADS=1
WORK=${WORK:?set WORK to the stage work list}
# No default: without an array this used to fall back to "shard 0 of 11", quietly running a
# fraction of the work and exiting 0. A sweep that is 10/11 missing looks like a sweep that was
# interrupted, and you go looking in the wrong place.
NSHARD=${SLURM_ARRAY_TASK_COUNT:?no array -- submit with tools/snapsweep_submit.sh}
ID=${SLURM_ARRAY_TASK_ID:?no array -- submit with tools/snapsweep_submit.sh}
MINE=$(mktemp)
awk -v n="$NSHARD" -v i="$ID" 'NR % n == i' "$WORK" > "$MINE"
echo "node=$(hostname) shard=$ID/$NSHARD lines=$(wc -l < "$MINE") work=$WORK"
[ -s "$MINE" ] || { echo "nothing to do"; exit 0; }
t0=$(date +%s)
xargs -P 8 -I@ "$HOME/elytra-atlas/tools/snapsweep_solve.sh" "@" < "$MINE"
echo "shard $ID done in $(( $(date +%s) - t0 ))s"
rm -f "$MINE"
