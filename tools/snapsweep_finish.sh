#!/bin/bash
# Stage 2, cleanup, and the per-cell figures. Run after stage 1 has drained.
#
#   tools/snapsweep_finish.sh            on the cluster: build stage 2, submit, wait, clean
#
# The coarse profiles are deleted only after stage 2 has written its own output, so an
# interrupted run never loses the scaffolding it still needs.
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
RUN=${RUN:-$HOME/atlas-run}
cd "$RUN" || exit 1

echo "== building stage 2 (HALF=${HALF:-100}) =="
RUN="$RUN" HALF="${HALF:-100}" python3 "$SELF/snapsweep_build.py" 2 || exit 1

echo "== submitting =="
JID=$(sbatch --parsable --export=ALL,WORK=work/stage2.tsv "$SELF/snapsweep.sbatch")
echo "job $JID  $(date '+%Y-%m-%d %H:%M:%S %Z')"
while squeue -j "$JID" -h -o '%T' 2>/dev/null | grep -q .; do sleep 15; done
echo "== stage 2 done $(date '+%Y-%m-%d %H:%M:%S %Z') =="

want=$(wc -l < work/stage2.tsv | tr -d ' ')
got=$(find out -name 'tight_t*.pitches' -not -path '*/coarse/*' | wc -l | tr -d ' ')
echo "fine profiles: $got / $want"
if [ "$got" != "$want" ]; then
  echo "INCOMPLETE -- not deleting coarse. Re-run to resume." >&2
  exit 1
fi

echo "== deleting coarse scaffolding =="
n=$(find out -type d -name coarse | wc -l | tr -d ' ')
find out -type d -name coarse -exec rm -rf {} + 
echo "removed $n coarse directories; snap_window.json keeps the choice"
echo "cells with a window record: $(find out -name snap_window.json | wc -l | tr -d ' ')"
