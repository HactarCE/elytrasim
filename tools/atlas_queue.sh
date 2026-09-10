#!/bin/bash
# The overnight atlas. One cell at a time -- each cell already runs 5 solves in parallel, so
# stacking cells would only make them contend. Every cell resumes by file existence, so this
# script is safe to re-run and a kill costs at most one solve.
#
# Cells are chosen from the (n, lambda) grid of runs/antichatter/shard0 so that each one has a
# free continuation baseline to be validated against: shard0 is the same objective (mu 1e-4,
# limit 85, mth_lut) solved by breadth-first warm starts instead of independent multi-start.
# Agreement between the two is evidence that continuity in the corpus is a property of the
# problem rather than an artifact of warm starting; disagreement localizes where continuation
# carried a branch it should have dropped.
#
#   tools/atlas_queue.sh [--wait]      --wait blocks until any running solves have drained first
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
cd "$SELF/.." || exit 1

if [ "${1:-}" = "--wait" ]; then
  while pgrep -f 'target/release/sweep' > /dev/null; do sleep 20; done
fi

say() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"; }

# Ordered by value, because the machine is the binding constraint: sustained throughput is about
# 3.6 solves/minute on all ten cores, so a night is five or six cells and the list below will not
# finish. Whatever has run when morning comes is the atlas.
#
# The reference cycle's own v0 = (0.167467, 0.200887) is deliberately NOT here. It is the velocity
# REPLAY_PITCHES_300 happens to close on, which gives it unrepresentatively nice steady-state
# behavior and is a property of that one profile rather than of the problem; it is also off the
# sweep's own velocity grid. The v0 axis is sampled at {0, 0.2} x {0, 0.2} instead.
#
# name                n    lambda  vy         vz          seeds
CELLS="
v00_n300_lam0       300   0        0          0           n300
flick_v00_n300      300   0        0          0           flick300
v00_n150_lam0       150   0        0          0           n150
v00_n450_lam0       450   0        0          0           n450
vy02vz02_n300_lam0  300   0        0.2        0.2         n300
vy00vz02_n300_lam0  300   0        0          0.2         n300
vy02vz00_n300_lam0  300   0        0.2        0          n300
v00_n300_lamP2      300   2        0          0           n300
v00_n300_lamM2      300  -2        0          0           n300
v00_n600_lam0       600   0        0          0           n600
"

echo "$CELLS" | while read -r name n lam vy vz seeds; do
  [ -z "${name:-}" ] && continue
  say "CELL $name  n=$n lambda=$lam v0=($vy, $vz)"
  N=$n LAM=$lam VY=$vy VZ=$vz MU=${MU:-0.0001} LIMIT=${LIMIT:-85} \
    PASSES=${PASSES:-400} JOBS=${JOBS:-5} \
    tools/atlas_run.sh "runs/atlas/seeds/$seeds" "runs/atlas/cells/$name"
done
say "ATLAS QUEUE DONE"
