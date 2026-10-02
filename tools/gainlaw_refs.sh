#!/bin/bash
# Horizon-free closed cycles for `myopic gainlaw`, built from `runs/steady/nlamsweep` cells.
#
#   tools/gainlaw_refs.sh <outdir> <cell>...      e.g. tools/gainlaw_refs.sh runs/gainlaw n0256_lamP0
#   tools/gainlaw_refs.sh <outdir> table          run `myopic gainlaw` over everything built there
#
# A steady cell is an open-horizon optimum: its climb ends at the cut, where the terminal price is
# `dJ/dv_n` rather than the periodic costate, and that bends the last twenty-odd ticks of the climb
# -- exactly the ticks the law is tested on. So each cell is tiled three times, re-polished under
# its own header's physics, and the middle apex-to-apex cycle is cut out and closed with `wobble`.
# The periodic costate then fits the cut cycle's climb to 0.1-1.2 deg RMS (`myopic gain`).
#
# About 15-60 s of one core per cell, n = 200..450.
set -eu
SELF=$(cd "$(dirname "$0")" && pwd)
cd "$SELF/.." || exit 1
for b in sweep myopic examples/wobble; do
    [ -x ./target/release/$b ] || { echo "no ./target/release/$b under $PWD" >&2; exit 1; }
done
OUT=${1:?usage: gainlaw_refs.sh <outdir> <cell>... | table}; shift
R=runs/steady/nlamsweep
mkdir -p "$OUT"
# `w` goes on the command line because a profile header's `w` is recomputed from its lambda, and
# wobble's header states lambda 0; see `wobble`.
w_of() { awk '/^# w /{print $3}' "$1"; }
if [ "${1:-}" = table ]; then
    for f in "$OUT"/*.cyc; do
        ./target/release/myopic --trig mth_lut --flight algebraic gainlaw "$f" "$(w_of "$f")" --limit 85
    done
    exit 0
fi
for c in "$@"; do
    f=$(awk -F, -v c="$c" '$1==c{print $14}' $R/best.csv | tr -d '\r')
    [ -n "$f" ] || { echo "$c: not in $R/best.csv" >&2; exit 1; }
    p=$R/out/$c/$f
    n=$(awk '/^# n /{print $3}' "$p"); lam=$(awk '/^# lambda/{print $3}' "$p")
    vy=$(awk '/^# v0/{print $3}' "$p"); vz=$(awk '/^# v0/{print $4}' "$p")
    w=$(awk '/^# w /{print $3}' "$p")
    grep -v '^#' "$p" > "$OUT/$c.x3"; cat "$OUT/$c.x3" "$OUT/$c.x3" "$OUT/$c.x3" > "$OUT/$c.x3.tmp"
    mv "$OUT/$c.x3.tmp" "$OUT/$c.x3"
    RAYON_NUM_THREADS=${THREADS:-1} ./target/release/sweep polish --n $((3 * n)) --lambda "$lam" \
        --vy "$vy" --vz "$vz" --init "$OUT/$c.x3" --trig mth_lut --flight algebraic \
        --mu 1e-4 --limit 85 --passes 200 --out "$OUT/$c.x3pol" 2> "$OUT/$c.log"
    grep -v '^#' "$OUT/$c.x3pol" | ./target/release/myopic --trig mth_lut --flight algebraic \
        cyclecut /dev/stdin > "$OUT/$c.cut" 2>> "$OUT/$c.log"
    ./target/release/examples/wobble "$OUT/$c.cut" 0 "$w" \
        | sed 's/^# trig libm/# trig mth_lut/; s/^# flight reference/# flight algebraic/' > "$OUT/$c.cyc"
    rm -f "$OUT/$c.x3" "$OUT/$c.cut"
    echo "$c: $(tail -1 "$OUT/$c.log")"
done
