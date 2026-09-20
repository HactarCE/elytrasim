#!/bin/bash
# Submit one stage of the snap-timing sweep, one array task per usable node.
#
#   RUN=... tools/snapsweep_submit.sh work/stage1.tsv          submit, print the job id
#   RUN=... tools/snapsweep_submit.sh work/stage2.tsv --wait   submit, block until it drains
#
# The shard count is computed here instead of being written into snapsweep.sbatch. It has to
# equal the number of nodes that will actually run, and a number sitting in the batch script
# goes stale silently the moment the rack changes size -- on 2026-09-19 the file still said
# `--array=0-10` from an eleven-node rack, which would have left 5 of 16 nodes idle for the
# whole sweep. Nothing warns you: the job succeeds, it just takes 45% longer.
#
# NODES=n overrides the count, PART=name the partition, ALLOW_BAD=1 proceeds anyway.
# STEADY=1 is forwarded to the solver (see snapsweep_solve.sh). It is named in --export rather
# than left to ALL because it changes what the corpus *is*, and a variable that reaches the
# nodes only by inheritance is one `sbatch --export=NONE` away from a sweep that silently
# solved the other problem.
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
WORK=${1:?usage: snapsweep_submit.sh <work list> [--wait]}
WAIT=${2:-}
PART=${PART:-camelot}
[ -s "$WORK" ] || { echo "$WORK: empty or missing work list" >&2; exit 1; }

# A node that is down, drained or failing is refused rather than sized around. Sizing around it
# silently turns a broken rack into a slower sweep, and a rack that is quietly one node short is
# a thing to go look at -- the last one was a compute node that had locked up under load.
# ALLOW_BAD=1 proceeds anyway, for when you already know and want the sweep to run regardless.
# A whitelist on the state field, not a blacklist over the whole line: a blacklist matches
# node names too (these are Arthurian, but "drain" inside a hostname is a bug waiting), and it
# cannot know every state Slurm might invent. A trailing `*` means the node is not responding,
# so `idle*` is not `idle` and correctly falls outside the whitelist.
STATES=$(sinfo -h -p "$PART" -N -o '%N %t')
BAD=$(echo "$STATES" | awk '$2 !~ /^(idle|mix|alloc|comp)$/')
if [ -n "$BAD" ] && [ "${ALLOW_BAD:-0}" != 1 ]; then
  echo "partition $PART has nodes that cannot take work:" >&2
  echo "$BAD" | sed 's/^/  /' >&2
  echo "fix them, or set ALLOW_BAD=1 to run on the rest." >&2
  exit 1
fi
N=${NODES:-$(echo "$STATES" | awk '$2 ~ /^(idle|mix|alloc|comp)$/' | wc -l | tr -d ' ')}
[ "${N:-0}" -ge 1 ] || { echo "no usable nodes in partition $PART" >&2; exit 1; }

JID=$(sbatch --parsable --partition="$PART" --array="0-$((N-1))" \
        --export=ALL,WORK="$WORK",STEADY="${STEADY:-}" "$SELF/snapsweep.sbatch") || exit 1
echo "job $JID  $N shards  $(wc -l < "$WORK" | tr -d ' ') solves  $(date '+%Y-%m-%d %H:%M:%S %Z')" >&2

if [ "$WAIT" = "--wait" ]; then
  while squeue -j "$JID" -h -o '%T' 2>/dev/null | grep -q .; do sleep 15; done
  echo "job $JID drained $(date '+%Y-%m-%d %H:%M:%S %Z')" >&2
fi
echo "$JID"
