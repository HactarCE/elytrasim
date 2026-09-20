#!/bin/bash
# One solve. Reads a space-separated work line: seedfile outfile n lambda vy vz
#
# STEADY=1 adds --steady, which re-solves v0 to the schedule's own fixed point after every pass.
# The work line's vy/vz then only *seed* that iteration and the header states the fixed point
# actually reached, so v0 stops being a coordinate of the grid -- pair this with
# snapsweep_grid.py --steady, which drops the velocity tag from the cell name to match. Steady
# is free on time: measured 5.5s either way for one n=250 cell at 30 passes on one core.
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
# ${STEADY:+--steady} is safe under `set -u`: the :+ form does not trip nounset when unset.
exec "$SWEEP" polish --trig mth_lut --flight "${FLIGHT:-algebraic}" ${STEADY:+--steady} \
  --n "$N" --lambda "$LAM" --vy "$VY" --vz "$VZ" \
  --passes 30 --tol 0 --mu 0.0001 --limit 85 \
  --init "$SEED" --out "$OUT" > /dev/null 2>&1
