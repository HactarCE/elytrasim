# The control space, and where the weird answers come from

`README-sweep.md` records that polishing the exact objective to convergence produces a schedule
no human can fly, and that the way out was to stop the optimizer early -- a pass budget, or the
`--lag1-floor` guard. That works, and it is unsatisfying for a specific reason: it makes the
answer a property of when the optimizer was interrupted rather than a property of the problem.
It also invites the reading "the optimum really is weird, we just cannot reach it", which would
mean the corpus is a record of an optimizer's stamina.

This document is the follow-up. It says what the degenerate answers actually are, shows that a
stronger optimizer finds them faster rather than avoiding them, and replaces the stopping rule
with two things that go in the header and leave the polish free to converge: a *price* on how
much the schedule asks of a wrist, and a *margin* between the pitch and the ends of its range.

The short version:

* Adam at 20000 passes chatters harder than coordinate ascent does and scores higher. The
  degeneracy is not something a better optimizer avoids.
* There are **two** degeneracies. The chatter is worth 0.7%. The other one -- parking a pitch a
  hundredth of a degree from +-90, where `look_hor_length` underflows and the aerodynamics
  switch off -- is worth five to seventeen blocks of fragility, is invisible to `lag1`, and is in
  91% of the cells of the corpus we have.
* Total variation cannot separate a flick from chatter (a factor of 2.3). The *second* difference
  can (380 at the 95th percentile), in an l1 norm, because a real schedule's curvature is sparse.
* Priced and margined, polished to convergence with no stopping rule at all: at the reference
  cycle's own curvature budget, `dJ` **21.83 against a person's 21.49**, +1.6%, with a *lower*
  peak angular acceleration and no fragility left.

Everything below is at the cell every chatter number in `README-sweep.md` was measured on:
`n = 300`, `lambda = 0`, `v0 = (0.167467, 0.200887)`. `J` is `J(s_n)`; `dJ = J - 0.4275`.

## The optimizer is not too weak

`examples/gd.rs` is finite-difference gradient ascent with Adam, sharing the objective and the
jitter with `polish`. Its passes are about a thousand times cheaper than a coordinate-ascent
global pass -- two tail replays per tick against 720 -- so 20000 of them cost 36 seconds.

| | J | TV | lag1 | max abs d2p | residual |
|---|---|---|---|---|---|
| reference cycle (flown by a person) | 21.929 | 248 | +0.48 | 38 | - |
| coordinate ascent, no price, from the reference | 22.180 | 2121 | -0.73 | 180 | 3.8e-5 |
| coordinate ascent, no price, from a chattering seed | 22.346 | 2667 | -0.80 | 180 | 4.3e-5 |
| **Adam, 20000 passes, from the reference** | **22.198** | **3751** | **-0.75** | **180** | **6.5e-4** |
| Adam, 7000 passes, from a chattering seed | 22.367 | 4326 | -0.89 | 179 | - |

Gradient ascent chatters *harder* than coordinate ascent and scores higher doing it. Plain SGD
diverges at every step size that moves at all (`lr` 30 leaves `dJ` at 17.7 with a gradient norm
of 375; `lr` 300 goes to -17.2). So the degeneracy is not a Gauss-Seidel artifact and it is not
a budget problem. Any optimizer that is actually good at this objective finds it.

## What the degenerate answers are, exactly

Two separate things, and only one of them is the chatter.

### 1. Chatter: tick-rate alternation of the aerodynamics

At pitch 0 the elytra is fully engaged; at 90 `lift_force = cos^2` is zero to fifteen digits, so
the tick is very nearly ballistic. Alternating between them beats holding any fixed pitch. It is
localized: ticks ~160-192, the end of the dive, and nowhere else.

Things that do not detect it, with the numbers:

| statistic | reference cycle | chattering optimum | ratio |
|---|---|---|---|
| total variation, deg | 248 | 2407 | 9.7 |
| max abs first difference, deg/tick | 38.3 | 90.0 | **2.3** |
| max abs second difference, deg/tick^2 | 37.9 | 180.0 | 4.7 |
| 95th pct abs second difference | 0.4 | 153.2 | 380 |
| sum abs second difference | 147.1 | 4211.1 | 29 |

The first difference is why a slew-rate limit cannot work: the reference cycle's snap really does
cover 38 degrees in one tick, so any budget that admits it admits an alternation. The *second*
difference separates them, and the reference's is **sparse** -- a straight line almost everywhere
with a handful of corners. So the norm to charge is l1, which is l1 trend filtering: it buys a
corner wherever one earns its keep and charges nothing for the straight runs between.

Two other things were tried and did not work, and are recorded so they are not tried again:

* **Sub-tick phase.** Read the schedule as samples of a piecewise linear control and let the game
  sample it at `t + phi` instead of `t` (`examples/fragility.rs`). Averaged over phase the
  chattering schedule scores `dJ` 21.85 against the reference's 21.54, and its worst phase is
  21.78. Being unable to phase-lock your mouse to the tick clock does not cost you the exploit.
* **Initial-velocity jitter**, re-measured here: `E[dJ]` at sigma 0.1 is *higher* than the
  unperturbed value for every schedule in the table, chattering or not, and the 5th percentile
  is about 1.9 blocks down for all of them equally. It does not discriminate.

### 2. Boundary parking: sitting a hundredth of a degree from a cliff

This one is larger than the chatter and a curvature price does not touch it.

`look_hor_length` is `|cos(pitch)|`, and the descent-to-forward conversion, the forward-to-up
conversion and the turning term are all behind `if look_hor_length > 0.0`. Both ends of the pitch
range are that gate rather than a limit, and which cliff you get depends on the trig:

* **`mth_lut`, which is vanilla.** `Mth::cos(-90 deg)` indexes `SIN[0]`, exactly `0.0`, so the
  gate *fails*: pitch -90 is a dead tick with no lift, no conversion and no turning. At +90 the
  same table gives +9.6e-5 and the gate passes -- the two ends are not symmetric. The flick wants
  the forward-to-up conversion at its maximum, which is at -90, so the optimizer parks at
  **-89.98902**, one table cell (0.0055 deg) short of the cliff. Overshoot by 0.05 degrees and
  that tick's next `v_y` is 0.331 instead of 0.611 and the schedule loses **5.2 blocks**.
* **`libm`, the current default.** `cos(90 deg)` in f32 is `-4.4e-8`, *negative*, so
  `look_angle.z / look_hor_length` flips to -1: forward reverses and the turning term becomes
  `-0.2 * v_z`, a fifth of your speed every tick. Scanned at tick 169 (`examples/razor.rs`) the
  cliff sits between pitch 89.9999924 (`dJ` 21.918) and 90.0000000 (`dJ` -3.395): 25 blocks
  across 7.6e-6 of a degree. The optimizer parks 6e-5 degrees from it.

Worst loss from nudging one single tick by 0.05 degrees (`examples/whichtick.rs`, vanilla trig):

```
reference cycle (flown by a person)     0.0002 blocks
polished under a curvature price        5.2353 blocks   tick 209, pitch -89.98902
polished with no price at all          16.9060 blocks   tick 188, pitch -90.00000
```

Minecraft delivers rotation in steps of about `0.15 * sensitivity` degrees. Nobody holds a pitch
a hundredth of a degree off a cliff on purpose, so the pitches beyond a margin are not worse than
the ones inside it -- they are unavailable, which makes this a statement about the control space
and not a term in the objective.

**This also means the corpus as it stands has a portability problem one level below the one
`sweep fingerprint` was built for.** It is written under `trig = libm`, and any cell whose
optimum leans on pitch +90 is leaning on the sign of a rounding error that vanilla does not have.

## The fix: say what the control can do, then converge

Two additions to `PolishOpts`, both in `Rough` in `src/opt.rs`, both recorded in the profile
header so `sweep verify` re-certifies against the same objective the writer used:

```
--mu <blocks per deg/tick^2>   price on sum |p[t+1] - 2p[t] + p[t-1]|
--limit <deg>                  largest |pitch| the schedule may use
--presmooth <k>                replace the seed by its local mean over k ticks, before polishing
--cap / --slew-cap             hard caps on the second / first difference (available, not used below)
```

`--mu` is a term in the utility function and says so; `--limit` is a restriction of the
admissible set. Neither is a stopping rule, and with either of them on, `--lag1-floor` is off,
`--tol` is 0.002, and the polish runs until it stops moving. Residuals below are 1e-5 or better.

`--presmooth` is the piece that makes it work, and it is not cosmetic. A chattering control is
the discrete stand-in for a *relaxed* control: at each tick the optimizer is really choosing a
distribution over pitches and realizing the mixture by alternating. The ordinary control that
means the same thing is the local mean. Skip it and coordinate ascent can stall, because an l1
penalty on second differences couples three neighbouring coordinates -- it is a fused-l1 term,
and coordinate descent has no guarantee on a nonsmooth objective that couples coordinates.
Measured: seeded from a schedule alternating -90/+90, the same price converged (residual 2.2e-7)
to `curv_l1` 1295, against 79 from the same price seeded from the local mean.

### The recipe

```
# 1. relax: solve the problem with no price on the control at all, to convergence
sweep polish --trig mth_lut --n 300 --vy 0.167467 --vz 0.200887 --lambda 0 \
             --passes 200 --tol 0.005 --init <seed> --out relaxed.pitches

# 2. project and polish: local mean, then converge under a price and a margin
sweep polish --trig mth_lut --n 300 --vy 0.167467 --vz 0.200887 --lambda 0 \
             --passes 200 --tol 0.002 --init relaxed.pitches \
             --presmooth 9 --mu 0.001 --limit 85 --out flyable.pitches
```

Step 1 is not optional and not a mistake to be avoided: without a relaxation step the price
lands 0.29 blocks lower, and graduated smoothing does not substitute for it.

| what was projected at `mu = 1e-3` | its own J | J after projection |
|---|---|---|
| coordinate ascent from a chattering seed | 22.346 | **22.213** |
| Adam from the reference | 22.198 | 22.073 |
| coordinate ascent from the reference | 22.180 | 22.061 |
| nothing -- `mu` annealed 0.3 -> 0.001 from the reference, never chattering | - | 21.924 |
| nothing -- `mu = 1e-3` straight from the reference | - | 21.923 |

Annealing `mu` down from 0.3 lands within 0.001 blocks of where the direct run lands. The chatter
is worth something as *search* even though it is worthless as an *answer*.

**But the projection is not monotone in the relaxed J, and that is the sharp edge in this
recipe.** A second relaxed optimum, `B`, scores 22.402 -- the highest raw J anything found -- and
projects *worse* than the 22.346 one under every filter but one:

| projection of relaxed B (J 22.402) | J after | | projection of relaxed A (J 22.346) | J after |
|---|---|---|---|---|
| none | 14.23 | | median 5 | 22.179 |
| box 3 / 5 / 9 | 15.49 / 15.73 / 15.67 | | median 7 | 22.167 |
| median 3 / 5 / 7 / 9 | 15.32 / 15.69 / 15.92 / 16.01 | | box 3 / 5 / 9 | 22.165 / 22.161 / 22.156 |
| median 13 | **22.136** | | box 15 / 25 | 22.130 / 22.047 |

Every one of those is converged -- residuals 1e-5 or better, several at the 200-pass ceiling. So
`B` is simply a worse *basin* for a flyable schedule despite being a better relaxed point, and a
projection width that is fine for one relaxed optimum is catastrophic for another. There is no
way to tell from the relaxed schedule which you have.

The recipe therefore has to be multi-start, which is cheap: relax from two or three seeds,
project each at `k` in {3, 5, 9, 15} with both filters, polish all of them, and keep the best on
`J - mu * sum|d2p|`. Twenty-four runs of half a minute. What is *not* legitimate is picking one
projection, getting 15.66, and reporting it as the answer.

`tools/flyable.sh <outdir> <mu>` is that, end to end:

```
N=300 tools/flyable.sh runs/flyable 1e-4         # cell from N / LAM / VY / VZ, defaults as above
```

and the spread across its 24 candidates on the standard cell is the argument for doing it:

```
score      dJ    curv_l1  curv_max  lag1   candidate
21.851  21.868     169.3      34.3  +0.65  r2_box15      <- best
21.771  21.792     209.5      47.1  +0.56  r3_med9
21.767  21.782     150.5      25.7  +0.70  r3_med5
21.629  21.643     147.1      25.0  +0.71  r1_box9
20.516  20.547     305.1      47.7  +0.52  r2_box9
19.799  19.838     383.6      49.4  +0.51  r1_box5
17.499  17.519     204.1      42.5  +0.63  r1_box3
15.064  15.084     199.3      39.6  +0.66  r3_box3       <- worst
```

Every one of those is converged. Six and three quarters blocks separate the best from the worst,
and nothing about the schedule -- not `lag1`, not `curv_l1`, not looking at it -- distinguishes
them. Only `J` does.

## What it costs: J against hand movement

Every point below is a converged coordinate optimum (`--tol 0.002`, residuals 1e-5 or better) of
`J - mu * sum|d2p|` at `--limit 85`, projected from the same relaxed solution with
`--presmooth 9`. `curv_l1` is `sum |d2p|` in deg/tick^2 -- what the wrist actually does.

| mu | curv_l1 | max abs d2p | TV | lag1 | dJ |
|---|---|---|---|---|---|
| none (relaxed) | 6819 | 281 | 4169 | -0.66 | **21.975** |
| 3e-5 | 424 | 81 | 462 | +0.18 | 21.859 |
| **1e-4** | **147** | **27** | **336** | **+0.71** | **21.829** |
| 2e-4 | 131 | 28 | 328 | +0.75 | 21.817 |
| 5e-4 | 82 | 24 | 300 | +0.81 | 21.745 |
| 1e-3 | 79 | 16 | 291 | +0.84 | 21.728 |
| 2e-3 | 68 | 16 | 292 | +0.84 | 21.630 |
| 5e-3 | 65 | 10 | 294 | +0.88 | 21.494 |
| 1e-2 | 50 | 9 | 272 | +0.90 | 20.707 |
| 2e-2 and up | 34 | 8 | 246 | +0.90 | 13.23 (collapsed to a glide) |
| *reference cycle, flown by a person* | *147* | *38* | *248* | *+0.48* | *21.494* |

Read the bolded row against the italic one: **at the reference cycle's own curvature budget the
optimizer finds `dJ` 21.829 against a person's 21.494 -- 0.335 blocks, +1.6% -- while asking for
a lower peak angular acceleration, 27 deg/tick^2 against 38.** All the chatter above that is
worth 0.146 blocks, **0.7%**.

The frontier is very flat over the range anyone cares about. Dropping `curv_l1` from 424 to 79, a
factor of five less hand movement, costs 0.13 blocks. It only turns down below about 65, and by
`mu = 2e-2` the schedule has stopped pumping and is holding a glide -- the collapse `opt::shape`
already knows how to name.

## Is the answer actually robust, or only smooth?

Smooth is not the same as not-overfit, so the schedules were re-scored under noise they will
actually see. `dJ` under uniform pitch noise of amplitude `a` on *every* tick, 200 draws, vanilla
trig, 5th percentile in brackets (`examples/sens.rs`):

| | a = 0.001 | a = 0.01 | a = 0.05 | a = 0.5 |
|---|---|---|---|---|
| reference cycle | 21.494 (21.494) | 21.495 (21.494) | 21.495 (21.494) | 21.426 (21.343) |
| **mu = 1e-4, limit 85** | 21.823 (21.822) | 21.820 (21.817) | 21.796 (21.777) | 21.521 (21.317) |
| mu = 1e-3, limit 85 | 21.723 (21.722) | 21.720 (21.717) | 21.704 (21.686) | 21.519 (21.338) |
| relaxed, no price or limit | 21.969 (21.968) | **-19.98 (-44.43)** | -45.88 (-58.36) | -49.86 (-60.20) |

Worst loss from nudging one single tick by 0.05 degrees (`examples/whichtick.rs`):

```
reference cycle         0.0002 blocks   (0.002 summed over all 300 ticks)
mu = 1e-4, limit 85     0.0145 blocks   (0.126 summed)   at tick 192, pitch 0.00013
relaxed, no limit       5.2 to 16.9     (102 summed)     at pitch -90.00000
```

The remaining 0.0145 is the *other* corner in the physics -- the forward-to-up conversion is
gated on `lean_angle < 0`, so pitch 0 is a kink and the snap sits on it at 1e-4 of a degree. It
costs a hundredth of a block, which is the right size for something to be left alone.

At a tenth of a degree of pitch noise -- far past any real input precision -- the priced schedule
is still 0.3 blocks ahead of the reference cycle. At half a degree they meet. That is the honest
statement of how much of the +1.6% is real: all of it, until your hand is worse than half a degree.

## Head to head against the stopping rule

The same cell, under vanilla trig, comparing what `README-sweep.md` recommends today against
this. The old recipe is `--jitter 0.10 --draws 8 --lag1-floor 0.2` seeded from the reference
cycle; it stops itself at 12 passes and the pass budget above that changes nothing (8, 16 and 60
give bit-identical output).

| | dJ | curv_l1 | max abs d2p | lag1 | residual | worst 1-tick nudge | dJ at +-0.01 deg noise |
|---|---|---|---|---|---|---|---|
| reference cycle (a person) | 21.494 | 147 | 38 | +0.48 | - | 0.0002 | 21.495 (21.494) |
| jitter + `--lag1-floor 0.2` | 21.564 | 308 | 73 | +0.26 | 3.4e-4 | **5.369** | **18.464 (10.961)** |
| `--mu 1e-4 --limit 85` | **21.829** | **147** | **27** | +0.71 | **2.5e-6** | **0.015** | **21.820 (21.817)** |

Better on every axis at once: 0.265 blocks more `J`, half the summed curvature, a third of the
peak angular acceleration, and a residual 140 times smaller -- meaning it is actually converged
rather than interrupted.

The interesting column is the last two. **The stopping rule catches the chatter and does nothing
at all about the boundary parking**, which is the larger fragility: its answer holds three
consecutive ticks at pitch -89.98902, one table cell from the gate, and each of them costs more
than five blocks if the hand is 0.05 degrees out. Under a hundredth of a degree of pitch noise it
loses 3.1 blocks on average and 10.6 at the fifth percentile. That is what "weird and overfit"
actually looks like once the chatter has been suppressed, and `lag1` cannot see it -- it reads a
perfectly respectable +0.26.

## The corpus as it stands

`python3 tools/audit.py runs/corpus`, medians per shard:

```
shard                    cells   curv_l1  curv_max    lag1  at +-90
vy+0.0000_vz+0.0000        403      4167       180   -0.77      91%
vy+0.2000_vz+0.2000         11       542        90   +0.33      91%
vy+0.2000_vz-0.2000         11       534        90   +0.14      91%
vy+0.4000_vz+0.2000         11       667        94   +0.15      91%
vy-0.2000_vz+0.2000         11       466        63   +0.42      82%
vy-0.2000_vz-0.2000         18       437        96   +0.15      94%
ALL                        509      2384       180   -0.70      91%
reference cycle              1       147        38   +0.48       0%
```

Two things, and the second is the one that was not known.

**The compounding under continuation is real and it is most of the grid.** The `(0, 0)` shard
carries the whole `n` x `lambda` plane, so it has the longest continuation paths, and its median
cell sits at `curv_l1` 4167 with `lag1` -0.77 -- fully degenerate, 28 times the reference cycle's
curvature. The nine small shards are 11 to 18 cells each, so their paths are short, and they land
at 437 to 667 with a positive `lag1`. `README-sweep.md` predicted exactly this ("a per-cell pass
budget accumulates along the continuation path") and the guard for it defaults off; this is the
measurement.

**Ninety-one percent of cells park a pitch against the gate, in every shard including the healthy
ones.** The anchor cell itself -- `n = 300, lambda = 0, v0 = 0`, the least-polished cell in the
corpus, `curv_l1` 187 and `lag1` +0.65, by every existing statistic a good profile -- holds three
ticks beyond 89.9 degrees. And the corpus is written under `trig = libm`, where that is the
25-blocks-across-7.6e-6-degrees cliff rather than the merely expensive one. So it is not only
that the degenerate cells are degenerate: the *good* cells are sitting on a discontinuity too,
and no statistic in the header says so.

Rebuilding wants `--trig mth_lut --limit 85 --mu 1e-4` and the relax/project/polish recipe per
cell. That has not been done.

## What is still open

* **The corpus is written under `trig = libm`.** Any cell leaning on pitch +90 is leaning on the
  sign of an f32 rounding error that vanilla does not have. The whole grid should be rebuilt
  under `--trig mth_lut`, or at minimum `--limit` should be on so it cannot matter. This is the
  same class of problem `sweep fingerprint` exists for, one level down.
* **The projection width is not predictable, only searchable.** `tools/flyable.sh` handles it by
  brute force, and that is honest but it is not understanding. Nothing seen so far says which
  relaxed optimum will project well.
* **Relax/project does not iterate.** Feeding the projection back in as a fresh relaxation seed
  went 22.402 -> 15.66 -> 18.68 and kept falling. One round, then stop.
* **The corpus has not been rebuilt.** `--mu` and `--limit` exist and are certified, but every
  cell in `runs/corpus` still predates them, and continuation along `n` and `lambda` under a
  price is untested -- a warm start now arrives with a *feasible* neighbour's schedule, which
  ought to help, but that is a guess.
* `--cap` and `--slew-cap`, the hard-constraint versions of the same idea, are implemented and
  were not swept. A cap is arguably the more honest instrument than a price -- it says a
  pitch profile is unavailable rather than expensive -- and `cap = 45` would admit every move
  the reference cycle makes with 20% to spare.


## Figures

Under `runs/antichatter/fig` (regenerate with the `tools/plot_*.py` and `tools/look.py` scripts;
`runs/` is gitignored, so they are not in the tree):

```
00_baseline.png    the schedules as they were: reference, hard-polished, jittered
01_hamiltonian.png the per-tick objective curve J(p) -- one peak, not two, so not a singular arc
03_razor.png       one tick scanned across the top of the pitch range, in both trig modes
04_frontier.png    J against summed |second difference|, every point converged
05_final.png       the priced schedules at mu = 1e-4, 1e-3, 5e-3
06_robust.png      dJ against pitch noise: the picture of what overfitting costs
07_best.png        reference / today's recipe / this recipe / the relaxed optimum
```

`07_best.png` is worth a specific look. Today's recipe (row 2) is visually clean -- it has no
chatter, and nothing in a pitch plot tells you it holds three ticks a hundredth of a degree from
a cliff. **Boundary parking is invisible to the eye and to `lag1`.** It shows up only in
`examples/whichtick.rs`, which is why that exists.
