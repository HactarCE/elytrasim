#!/bin/bash
# One `floor exit` solve from a work-list line: `<y0> <time|dist> <name> <init spec> [flags...]`,
# the flags passed through (e.g. `--ke -0.3`). Writes $RUN/out/<mode>_y<y0>_<name>.pitches and
# appends a timing line to $RUN/times.tsv.
# A file, not an exported function: exported functions do not survive xargs.
set -u
read -r Y MODE NAME SPEC REST <<< "$1"
OUT="$RUN/out/${MODE}_y${Y}_${NAME}.pitches"
[ -s "$OUT" ] && exit 0
t0=$(date +%s)
# REST is split on purpose: it is a list of flags.
"$FLOOR" exit --y0 "$Y" --mode "$MODE" --init "$SPEC" $REST --out "$OUT.tmp" > /dev/null \
  && mv "$OUT.tmp" "$OUT"
echo -e "$Y\t$MODE\t$NAME\t$(( $(date +%s) - t0 ))\t$(hostname)" >> "$RUN/times.tsv"
