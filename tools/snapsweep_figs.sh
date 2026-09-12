#!/bin/bash
# One field figure per cell, the same instrument as flicksoft30_v00_n300_field.png.
#
#   tools/snapsweep_figs.sh <celldir> <outdir> [jobs]
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
SRC=${1:?usage: snapsweep_figs.sh <celldir> <outdir> [jobs]}
OUT=${2:?usage: snapsweep_figs.sh <celldir> <outdir> [jobs]}
JOBS=${3:-8}
mkdir -p "$OUT"
ls -d "$SRC"/*/ | sed 's:/$::' > /tmp/snapfigs.$$
echo "$(wc -l < /tmp/snapfigs.$$ | tr -d ' ') cells -> $OUT"
xargs -P "$JOBS" -I@ python3 "$SELF/plot_field_replay.py" @ "$OUT" < /tmp/snapfigs.$$
rm -f /tmp/snapfigs.$$
echo "figures: $(ls "$OUT"/*_field.png 2>/dev/null | wc -l | tr -d ' ')"
