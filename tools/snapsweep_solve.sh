#!/bin/bash
# One solve. Reads a space-separated work line: seedfile outfile n lambda vy vz
#
# Space-separated, not tab: BSD xargs -I collapses tabs to spaces when it substitutes, so a
# tab-delimited line arrives as one field and `sweep` is handed an empty --n. No path here
# contains a space (cell names and tick names are generated), so spaces are the portable choice.
# Resumes by file existence, so a killed array task costs at most one solve per worker.
set -u
# Work-list paths are relative to the run directory, not to wherever xargs was invoked.
cd "${RUN:?set RUN to the run directory}" || exit 1
read -r SEED OUT N LAM VY VZ <<< "$1"
[ -n "${VZ:-}" ] || { echo "malformed work line: [$1]" >&2; exit 2; }
[ -s "$OUT" ] && grep -q '^# certified' "$OUT" 2>/dev/null && exit 0
mkdir -p "$(dirname "$OUT")" 2>/dev/null
# --tol 0 is deliberate: the instrument is a fixed 30-pass stopping time, and an early exit on
# a convergence test would silently give a handful of profiles a different stopping time than
# the rest while nothing in the file said so.
exec "$SWEEP" polish --trig mth_lut --flight "${FLIGHT:-algebraic}" \
  --n "$N" --lambda "$LAM" --vy "$VY" --vz "$VZ" \
  --passes 30 --tol 0 --mu 0.0001 --limit 85 \
  --init "$SEED" --out "$OUT" > /dev/null 2>&1
