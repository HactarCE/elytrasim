#!/bin/bash
# Stage 2, cleanup, and the per-cell figures. Run after stage 1 has drained.
#
#   RUN=... HALF=20 tools/snapsweep_finish.sh   on the cluster: build stage 2, submit, wait, clean
#
# HALF is the stride-2 window half-width around the coarse winner, and it is required rather
# than defaulted. why? it is the one parameter that changes what the corpus *is* -- 100 keeps
# the breadth of the population, which is the product for an atlas cell; 20 keeps only the
# neighbourhood of the optimum, and costs 2.4x fewer solves. A default would pick one silently,
# and the two are not comparable at the level of the profile population. The coarse stride-10
# scan localizes a real manoeuvre to within six ticks, so 20 is ample where only the optimum
# matters.
#
# The coarse profiles are deleted only after stage 2 has written its own output, so an
# interrupted run never loses the scaffolding it still needs.
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
RUN=${RUN:-$HOME/atlas-run}
cd "$RUN" || exit 1

HALF=${HALF:?set HALF to the stride-2 window half-width (20 or 100 -- see the header)}
echo "== building stage 2 (HALF=$HALF) =="
RUN="$RUN" HALF="$HALF" python3 "$SELF/snapsweep_build.py" 2 || exit 1

echo "== submitting =="
RUN="$RUN" "$SELF/snapsweep_submit.sh" work/stage2.tsv --wait > /dev/null
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
