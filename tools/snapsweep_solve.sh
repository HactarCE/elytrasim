#!/bin/bash
# One solve. Reads a tab-separated work line: seedfile  outfile  n  lambda  vy  vz
# Resumes by file existence, so a killed array task costs at most one solve per worker.
set -u
IFS=$'\t' read -r SEED OUT N LAM VY VZ <<< "$1"
[ -s "$OUT" ] && grep -q '^# certified' "$OUT" 2>/dev/null && exit 0
mkdir -p "$(dirname "$OUT")"
# --tol 0 is deliberate: the instrument is a fixed 30-pass stopping time, and an early exit on
# a convergence test would silently give a handful of profiles a different stopping time than
# the rest while nothing in the file said so.
exec "$SWEEP" polish --trig mth_lut --flight "${FLIGHT:-algebraic}" \
  --n "$N" --lambda "$LAM" --vy "$VY" --vz "$VZ" \
  --passes 30 --tol 0 --mu 0.0001 --limit 85 \
  --init "$SEED" --out "$OUT" > /dev/null 2>&1
