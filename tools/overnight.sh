#!/bin/bash
# The overnight corpus. Phases run in the order the axes are wanted, so a kill at any point
# leaves a complete earlier phase. Every phase resumes by file existence, so re-running is safe.
cd /Users/josie/programming_local/elytrasim-luna
B=./target/release/sweep
OUT=runs/corpus
REF=runs/veljit/ref300.pitches
# Stopping time is the regularizer: at most 8 passes, and stop earlier if a pass pushes the
# lag-1 correlation below 0.2. Under jitter the pass-1 transient stays above 0.2, so the floor
# catches real degeneracy rather than the entry forming. Without it continuation compounds:
# n 300 -> 310 -> 320 ran lag-1 +0.73, +0.59, -0.07 and TV 293, 361, 600.
OPTS="--passes 8 --tol 0 --lag1-floor -2 --jitter 0.10 --draws 8"
mkdir -p $OUT
say() { echo "[$(date +%H:%M:%S)] $*" ; }

NS10=$(python3 -c "print(','.join(str(n) for n in range(100,601,10)))")
NS50=$(python3 -c "print(','.join(str(n) for n in range(100,601,50)))")
LAMS=$(python3 -c "print(','.join('%g'%(i*0.25) for i in range(-16,17)))")

say "PHASE A  n axis, v0 = 0, lambda = 0, stride 10"
$B run --out $OUT $OPTS --anchor $REF --ns "$NS10" --lams 0 --vys 0 --vzs 0
cp $OUT/manifest.json $OUT/manifest_a_n.json 2>/dev/null

say "PHASE B  lambda sweep, v0 = 0, n stride 50, lambda -4..4 stride 0.25"
$B run --out $OUT $OPTS --anchor $REF --ns "$NS50" --lams "$LAMS" --vys 0 --vzs 0
cp $OUT/manifest.json $OUT/manifest_b_lambda.json 2>/dev/null

# Anchor the velocity shards on the solved v0 = 0 cell rather than the raw reference: it is the
# nearest solved point, so continuation starts closer and is less likely to pick a bad branch.
A0=$OUT/vy+0.0000_vz+0.0000/n0300_lam+0.000000.pitches
[ -f "$A0" ] || A0=$REF
say "PHASE C  velocity, corners first, n stride 50, lambda = 0   (anchor $A0)"
for vv in "-0.2 -0.2" "-0.2 0.4" "0.4 -0.2" "0.4 0.4" \
          "-0.2 0.2" "0.2 -0.2" "0.2 0.2" "0.2 0.4" "0.4 0.2"; do
  set -- $vv
  say "  v0 = ($1, $2)"
  $B run --out $OUT $OPTS --anchor "$A0" --ns "$NS50" --lams 0 --vys "$1" --vzs "$2"
done
cp $OUT/manifest.json $OUT/manifest_c_vel.json 2>/dev/null

say "PHASE D  lambda at the velocity corners, if there is still night left"
for vv in "-0.2 -0.2" "-0.2 0.4" "0.4 -0.2" "0.4 0.4"; do
  set -- $vv
  say "  v0 = ($1, $2), lambda sweep"
  $B run --out $OUT $OPTS --anchor "$A0" --ns "$NS50" --lams "$LAMS" --vys "$1" --vzs "$2"
done
say "OVERNIGHT DONE"
