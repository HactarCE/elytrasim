# The optimal-profile corpus

A grid of optimal elytra pitch schedules, one file per cell, over
`(num_ticks, lambda, vy0, vz0)`.

Each file states the utility function and initial conditions it was optimized under, and its
claim is checkable by re-running the optimizer's own stopping test against the file alone:

```
sweep verify runs/corpus/vy+0.0000_vz+0.0000/n0300_lam+0.000000.pitches
```

How a schedule was reached is deliberately not recorded. Given `(v0, pitches)` the trajectory is
fully determined, so every derived quantity is recomputable and the optimality claim is
falsifiable without trusting the pipeline that produced it.

## The objective

```
KE = |v|^2 / (2 * GRAVITY)     PE = y     TE = KE + PE        (blocks)
J  = TE(s_n) + w * z_n                                        (terminal velocity free)
w  = lambda * Y_REF / Z_REF    Y_REF = 21.5   Z_REF = 330
```

`lambda` normalizes the reference cycle's 21.5 blocks of climb against its 330 blocks of
distance, so at `lambda = 1` the two terms are the same scale. Negative `lambda` punishes
distance, which is the elytra-parkour side and matters at least as much as the positive side.

Pitch is `f32` because the sim casts it to `f32`, so `f32` is the exact control alphabet -- the
text in the file is the schedule that will be flown, not a rounding of it.

## Degeneracy, and why stopping time is the regularizer

Polished to convergence, the exact objective genuinely prefers a schedule no human can fly. At
pitch 90 degrees `cos(lean_angle)` is zero, so lift vanishes *and* `look_hor_length` vanishes,
gating off both the descent-to-forward conversion and the turning term; bang-bang between "full
aero" and "no aero" beats any fixed pitch on the singular arc. This is not a convergence
failure: 200 passes reaches residual 9.4e-6, a real coordinate optimum.

**Total variation cannot detect it**, because a real flick is also a large move -- the cycle
drops to 0 and covers ~88 degrees in about six ticks. What separates them is whether
neighboring ticks move *together*. `lag1`, the lag-1 correlation of the per-tick changes, is
the statistic that does: the reference cycle reads +0.48, the same schedule polished 200 passes
reads -0.74, and the sign flip is the diagnosis. It is in every header.

Things that were tried and measured:

| lever | result |
|---|---|
| slew-rate limit | no. Any limit loose enough to keep the real flicks keeps the chatter. |
| per-tick pitch jitter | no. Held fixed it made chatter *worse* (TV 2406 -> 3493); resampled it cut TV 29% for 2.3 blocks of climb. The chattering end is pitch-insensitive -- `lift_force = cos^2(lean_angle)` is flat at 90 degrees. |
| initial-velocity jitter, polished hard | no. At 60 forced passes TV is 925, 871, 838, 826, 884 for sigma 0, 0.05, 0.1, 0.2, 0.4 -- flat across an 8x range, while `dz` falls monotonically 345 -> 337. |
| initial-velocity jitter, stopped early | **yes.** At every small pass count, both horizons, for 0.02-0.03 blocks of `dJ`. |
| stopping time | **yes.** This is what bounds it. |

lag-1 against pass count from the reference cycle at `n = 300`, `sigma = 0`:

```
passes    0     1     2     3     4     8    16    32    60   200
lag-1  +0.48 -0.09 +0.01 +0.05 +0.13 +0.27 +0.29 +0.07 -0.27 -0.74
```

It is *not monotone*. The dip in the first few passes is the entry dive forming, which is
legitimate structure; the decay after ~16 passes is the chatter. Under `sigma = 0.1` the early
dip stays above 0.2 (+0.32, +0.42, +0.57, +0.53), which is what makes an absolute floor usable
as a stopping rule -- it catches the degeneracy without firing on the transient.

Nearly all the honest gain is early: `dJ` runs 21.50, 21.54, 21.58, 21.77 at 0, 4, 32 and 200
passes. The last 0.2 blocks is bought entirely with chatter.

### Continuation compounds

A warm-started cell inherits its neighbor's polish, so a per-cell pass budget accumulates along
the continuation path. Measured on `n = 300 -> 310 -> 320` at 8 passes each:

```
lag-1  +0.73  +0.59  -0.07
TV      293    361    600
```

Degenerate by the second hop. A pass budget cannot bound this; a property of the schedule can,
which is what `--lag1-floor` is for. It stops a polish that crosses the floor and *discards the
crossing pass* -- on that same `n = 320` cell it stops at 2 passes with lag-1 +0.217, TV 505,
and `dy` 19.02 against the unguarded 18.91. The discarded passes were buying chatter, not
height.

## The entry is under-determined

Splicing three very different entries (ticks 0-24) onto one shared tail moves `J` by 0.047
blocks total -- reference 21.942, an extreme dip 21.989, a gentler one 21.947 -- and the choice
of tail changes nothing (3.5e-5). The entry is a flat plateau, consistent with the 5.8e-7
curvature and 2.0-degree half-width measured in `README-myopic.md`. So an entry that looks
alarming is not necessarily wrong, and imposing smoothness there costs almost nothing.

More generally: the dive's score is 15x flatter than the gain's and the entry's flatter still,
while the snap is ~3000x more pitch-sensitive. **Any analysis reading pitch differences between
cells as signal has to check the `J` difference first.** Convergence is tested on `J`, never on
pitch.

## Running it

```
sweep polish --n 300 --lambda 0 --vy 0 --vz 0 --jitter 0.10 --draws 8 \
             --passes 8 --tol 0 --lag1-floor 0.2 --flight algebraic \
             --anchor <file> --out <file>
sweep run    --out <dir> --ns <list> --lams <list> --vys <list> --vzs <list> [--shard i,j]
sweep verify <file>...
sweep fingerprint
```

A shard is one `(vy0, vz0)` cell, so it is a directory and an independent job. Inside a shard,
continuation runs breadth-first over the `(n, lambda)` plane from the cell nearest `n = 300`,
`lambda = 0`; `--anchor` seeds that one cell from a file. This matters: at `v0 = 0` the built-in
policy seed lands degenerate (lag-1 -0.19 at 8 passes) where the reference cycle gives +0.73,
and the anchor propagates to the whole shard. Once the `v0 = 0` shard exists, anchor the
nonzero-velocity shards on its *solved* `n = 300, lambda = 0` cell rather than on the raw
reference file: it is the nearest solved point, so continuation starts closer and is less
likely to pick a bad branch.

Resume is by file existence -- a cell whose file matches `(n, lambda, v0, trig, flight)` is not redone
but still seeds its neighbors, so a killed job costs at most one cell.

`--flight reference|algebraic` selects the movement kernel. `reference` remains the default;
`algebraic` routes yaw-zero movement through the collapsed equations and falls back to the
reference path for nonzero yaw. Profiles and manifests record the route, and resume also
requires it to match. On an Apple arm64 test machine, a representative `dp` transition-table
build fell from 0.121 seconds to 0.057 seconds (2.12x); the generated value table and schedule
were byte-identical.

That 2.12x was measured before pitch trigonometry was cached, and caching has since taken most
of it back -- `PitchTrig` hoists the sin/cos out of *both* routes, and eliminating that trig was
most of what the algebraic route was winning. What is left is three divisions and a sqrt per
tick, and whether removing them pays depends on the machine. Measured on `sweep polish`, one
n=300 cell at 30 passes, single core:

```
                          reference   algebraic
  Apple M5 (arm64)           7.15 s      7.98 s     algebraic 1.12x SLOWER
  Intel i7-6700 (Skylake)   14.11 s      6.93 s     algebraic 2.04x faster
```

On Skylake f64 division is long-latency and poorly pipelined, so dropping three of them per
tick is worth 2x; on the M5 division is cheap and the extra branches the collapsed form needs
(`vel.x == 0.0`, `is_sign_negative`) cost more than they save. So pick the route for the
machine: `reference` on arm64, `algebraic` on the cluster. Both are recorded in the header, and
`verify` replays under the route the file claims, so a corpus may mix them without ambiguity --
though a single corpus is easier to reason about if it does not.

Check a corpus with `python3 tools/check.py <dir>`; read profiles from Python with
`tools/load.py`.

**What `runs/corpus` actually is.** It was built in four phases -- the `n` axis at stride 10,
then `lambda` at `n` stride 50, then the velocity corners, then `lambda` at those corners --
over `n = 100..600` and `lambda = -4..4` stride 0.25, with `--trig libm` and
`--passes 8 --tol 0 --jitter 0.10 --draws 8`. **The lag-1 floor was off** (`--lag1-floor -2`,
below the -1 a correlation can reach), so every cell ran all 8 passes; all 509 profiles read
`(8 passes)`, which is how you can confirm it from the corpus alone. That is worth stating
because the synopsis above shows `--lag1-floor 0.2` and the section on degeneracy argues for
it: the guard is the recommendation, it is not what produced this corpus, and a re-run with it
on would not be comparable. The per-phase manifests record the cells of each phase:
`manifest_a_n.json`, `manifest_b_lambda.json`, `manifest_c_vel.json`.

## Portability

Vanilla's `Mth` sin LUT is ported and selectable with `--trig`, which removes the f32 trig
divergence between platforms (LLVM fuses standalone `sinf`/`cosf` into `___sincosf_stret` on
arm64, differing by 1 ULP on ~2.6% of inputs). `lift_force` still goes through f64 `cos`, so
bit-identity across platforms is **not** guaranteed; `sweep fingerprint` makes any drift a
measured number rather than a silent one. Compare it between machines before trusting output
from a new one.
