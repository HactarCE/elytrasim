#!/bin/bash
#SBATCH --job-name=booster
#SBATCH --partition=camelot
#SBATCH --nodes=1
#SBATCH --exclusive
#SBATCH --output=logs/%x-%A_%a.out
# The booster explorer: one array task per node, one cell per hyperthread. Submit with
# `sbatch --array=0-<nodes-1> --export=ALL,RUN=<dir>`; the work list is $RUN/work.txt, one
# "<speed> <pitch>" per line, and the shard count is the array's size.
set -u
cd "${RUN:?set RUN}" || exit 1
export TRIG=mth_lut LIM=89 FLIGHT=algebraic RAYON_NUM_THREADS=1
NSHARD=${SLURM_ARRAY_TASK_COUNT:?submit as an array}
ID=${SLURM_ARRAY_TASK_ID:?submit as an array}
MINE=$(mktemp)
awk -v n="$NSHARD" -v i="$ID" 'NR % n == i' work.txt > "$MINE"
echo "node=$(hostname) shard=$ID/$NSHARD cells=$(wc -l < "$MINE")"
t0=$(date +%s)
# SOLVER picks the per-cell script: booster_solve.sh (full explorer profiles, the default) or
# booster_cell.sh (apex only).
xargs -P 8 -I@ "$HOME/booster-gain/tools/${SOLVER:-booster_solve.sh}" "@" < "$MINE"
echo "shard $ID done in $(( $(date +%s) - t0 ))s"
rm -f "$MINE"
