#!/bin/bash
# Build the seed files and the work list for one stage of the snap-timing sweep.
#
#   tools/snapsweep_build.sh 1     coarse: stride 10 across the whole horizon, every cell
#   tools/snapsweep_build.sh 2     fine:   stride 2 over +/-100 around each cell's coarse best
#
# Stage 2 reads each cell's coarse output, writes snap_window.json recording the choice, and
# only then emits seeds -- so a cell whose coarse scan did not finish is skipped loudly rather
# than being given a window derived from a partial scan.
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
RUN=${RUN:-$HOME/atlas-run}
STAGE=${1:?usage: snapsweep_build.sh <1|2>}
cd "$RUN" || exit 1
mkdir -p seeds out work logs

WORK="work/stage$STAGE.tsv"
: > "$WORK"
skipped=0
while IFS=$'\t' read -r NAME N LAM VY VZ FAM; do
  [ -z "${NAME:-}" ] && continue
  if [ "$STAGE" = 1 ]; then
    SD="seeds/$NAME/coarse"; OD="out/$NAME/coarse"; STEP=10; LO=0; HI=$N
  else
    SD="seeds/$NAME/fine";   OD="out/$NAME";        STEP=2
    if ! W=$(python3 "$SELF/snapsweep_pick.py" "out/$NAME" "$N" --half 100 --step 2 2>&1); then
      echo "SKIP $NAME: $W" >&2; skipped=$((skipped+1)); continue
    fi
    LO=${W% *}; HI=${W#* }
  fi
  if [ ! -f "$SD/seeds.json" ]; then
    python3 "$SELF/atlas_seeds.py" --n "$N" --flick-step "$STEP" \
      --flick-lo "$LO" --flick-hi "$HI" --out "$SD" > /dev/null || exit 1
  fi
  for f in "$SD"/tight_t*.pitches; do
    printf '%s\t%s/%s\t%s\t%s\t%s\t%s\n' "$f" "$OD" "$(basename "$f")" "$N" "$LAM" "$VY" "$VZ" >> "$WORK"
  done
done < cells.tsv

echo "stage $STAGE: $(wc -l < "$WORK" | tr -d ' ') solves queued in $WORK${skipped:+  ($skipped cells skipped)}"
