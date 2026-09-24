#!/bin/bash
#SBATCH --job-name=floor
#SBATCH --partition=camelot
#SBATCH --nodes=1
#SBATCH --exclusive
#SBATCH --output=logs/%x-%A_%a.out
# One array task per node, 8 single-threaded solves at a time (-P 8 saturates a NUC; rayon inside
# one solve scales worse than solves side by side). Shard count comes from the submitter:
#   sbatch --array=0-$((N-1)) --export=ALL,WORK=<list>,RUN=<dir> tools/floor.sbatch
set -u
cd "$RUN" || exit 1
export FLOOR="$HOME/elytra-floor/target/release/floor" RAYON_NUM_THREADS=1
NSHARD=${SLURM_ARRAY_TASK_COUNT:?submit as an array}
ID=${SLURM_ARRAY_TASK_ID:?submit as an array}
MINE=$(mktemp)
awk -v n="$NSHARD" -v i="$ID" 'NR % n == i' "$WORK" > "$MINE"
echo "node=$(hostname) shard=$ID/$NSHARD lines=$(wc -l < "$MINE")"
xargs -P 8 -d '\n' -I@ "$HOME/elytra-floor/tools/floor_job.sh" "@" < "$MINE"
rm -f "$MINE"
