# Flight over a floor

How long, and how far, can you fly from height `y0` and velocity `v0` before you first go under
`y = 0`? The floor is a plane, not Minecraft's collision: nothing happens at contact except that
the flight is over. `src/bin/floor.rs` is the tool; `Floor` in `src/opt.rs` is the price the
fixed-horizon polish pays for it.

Conventions. A replay starts at `y = 0` like every other schedule in this crate, so the floor sits
at `y = -y0`. The *clearance* at tick `t` is `h_t = y_t + y0`. A schedule *survives* `k` ticks when
`h_1 .. h_k >= 0`. Endurance is the most ticks any schedule survives; range is the most `z` at
the moment of exit. Physics is `mth_lut` trig and `reference` flight throughout.

## What the optimum looks like, `v0 = 0`, `y0 = 1..8`

```
            best hold          optimized, endurance     optimized, range      hold 0      hold -13
 y0   ticks  pitch       z   t*       ticks  z(t*)     t*      ticks z(t*)   ticks  z    ticks  z
  1     12   -10.2   0.490   12.661   12    0.529      12.64   12   0.530    12  0.466   11  0.403
  2     20    -8.0   1.622   20.511   20    1.663      20.47   20   1.671    20  1.603   18  1.299
  3     27   -10.6   3.263   28.191   28    3.387      28.08   28   3.414    27  3.232   26  2.962
  4     35   -10.7   5.636   36.448   36    5.787      36.18   36   5.856    34  5.446   34  5.192
  5     44   -10.3   8.737   45.867   45    9.008      45.32   45   9.167    40  7.790   43  8.204
  6     53   -13.7  12.510   56.956   56   13.229      55.87   55  13.575    47 11.015   53 11.971
  7     65   -12.8  17.197   69.813   69   18.499      67.76   67  19.264    54 14.738   64 16.429
  8     77   -14.8  22.609   83.827   83   24.557      80.13   80  26.212    60 18.299   77 21.929
```

The table is the first run (commit 8a19cdd, l1 price in score units); with the price in energy
units the optimized cells move by at most 0.04. "Best hold" is the best constant pitch for each column separately (0.1-degree grid), so its `z` is
not flown at its ticks' pitch. `t*` is the interpolated exit tick; see below. Every optimized cell
was reached from three different starts -- hold -13, hold 0, and a dive-then-pull -- that agree to
0.003 ticks in `t*` and 0.003 blocks in `z`.

What the optimizer buys over the best constant pitch is nothing at `y0 <= 2`, one tick at 3 to 5,
and six ticks (8%) and 3.6 blocks of range (16%) at `y0 = 8`. The gain grows with height.

**Below `y0` about 10-14 there is no dive and no climb:** `v_y` never goes positive.

* **Endurance** is one ramp: pitch 0 at the first tick, then nose-up at a near-constant rate --
  about -0.25 deg/tick at `y0 = 8` -- to about -22 at the exit.
* **Range** is a hold then a pull: pitch -0.0055 for 36 ticks at `y0 = 8`, then nose-up at
  0.6-1 deg/tick to -23, held to the exit. The hold sits just past the corner at 0, where the
  forward-to-up branch switches on.

The "look down" half of a mini pump is **pitch 0, not nose-down**. A steep dive keeps *more*
energy per block fallen -- from rest, 8 blocks down, `TE` is -4.34 at hold 60 against -5.42 at
hold 0 -- but it stores it as `v_y`, and `v_y` only turns into forward speed through the
`v_y < 0` conversion, 10% x `cos^2(pitch)` per tick, while still sinking. Pitch 0 is where that
conversion (and the lift) is largest, so the pull-out from a steep dive costs height a low floor
does not have. With the `minipump` seed (below), the fitted dive angle is 0 for every
endurance cell but two (5 degrees at `y0 = 22, 31`), 0-5 degrees for range up to `y0 = 27`, and
10-15 degrees for range at `y0 = 28..32`. At `y0 = 4` and `8` every dive angle from 0 to 45 lands on the
same optimum.

**Above that the mini pump appears.** The first climb (`v_y > 0`) shows up at `y0 = 14` for
endurance and `y0 = 10` for range: hold 0 while sinking, pull hard (about -44) and relax, then glide
or climb again. Endurance does exactly one climb from 14 to 32; range does two from 18 and three
from 27. From `v0 = 0`, cluster corpus of 2026-09-24 15:20 EDT (`runs/floor/v2-cluster`):

```
 y0   endurance t*  first climb   range z*   climbs
 10      112.47          -          43.10       1
 14      169.11         100         83.97       1
 16      211.19         114        111.45       1
 20      301.23         141        176.86       2
 24      386.12         168        249.65       2
 28      462.92         194        347.44       3
 32      538.23         208        476.92       3
```

A cold start from a hold never finds it. At `y0 = 19` the best of hold -13 and a tiled steady
cycle exits at 239.6 ticks; the `minipump` seed finds 279.0.

Range and endurance separate. At `y0 = 8` the range optimum exits 3.7 ticks earlier than the
endurance optimum and goes 1.66 blocks further.

## How it is solved, and what worked

The polish only knows a fixed horizon and a terminal objective, and the question is about a
*first exit* -- a discontinuous, path-dependent quantity. What follows is every trick that was
tried, what it measured, and whether it stayed. Timings are wall-clock on an Apple M5, all
cores. Measured at the commit that added this file.

### 1. The bubble: a soft floor in the fixed-horizon polish (`Floor`, kept)

`J = TE(s_n) + w*z_n`, less a price on every post-tick state:

```
pen = weight * (max(0, margin - h) / margin)^2  +  wall * max(0, -h)
```

A bubble of thickness `margin` that costs `weight` at contact, and a wall that is linear in depth
under the floor. The wall is not a barrier on purpose: the coordinate search samples the whole
pitch range at every tick and most samples crash, and a log barrier scores them all `-inf`, so
the sweep cannot tell a crash at tick 80 from one at tick 3. A finite wall keeps them ranked.

For **endurance** it does not matter what the bubble is. Four shapes, from `margin 0.02,
weight 0.1` to `margin 1, weight 10`, all put the edge at `y0 = 4` between `n = 36` (feasible,
clearance +0.02 to +0.05) and `n = 37` (clearance -0.0630 in every case). The floor only binds at
the last tick, so the bubble only decides how far off it the answer stands.

For **range** the bubble *is* the answer. A range optimum skims the floor for many ticks and pays
the bubble on each of them, so `z` is set by the ratio `weight / lambda`, not by `lambda`. At
`y0 = 8` and `n = 83`:

```
lambda   weight   wall     z
  20        1      100    25.44
  20       10     1000    24.21
 100       10     1000    24.78
 100      100    10000    24.14
1000      100    10000    24.78
1000     1000   100000    24.14
```

Equal ratios, equal `z`, to 0.01. **A bubble is a bias wherever the optimum lives near the floor.**

### 2. Feasibility is read off the replay, never off the price (kept)

`Floor::clearance` is the minimum `h` over the replay. That, not a small penalty, is what decides
feasibility, since a finite bubble stands the optimum off the floor by an amount that depends on
the price.

### 3. The safety value as a feasibility oracle (`floor safety`, kept as a check)

Coordinate ascent on the soft minimum clearance, `-tau ln sum exp(-h_t/tau)` with `tau = 0.01`:
the discrete HJ safety value `max_u min_t h_t`, which is `>= 0` exactly when the horizon is
feasible. It is what a feasibility question should be optimized on -- energy only picks among
feasible schedules. At `y0 = 4`, `n = 37`, it reaches -0.06303 from hold -13, hold 0 and a
dive-then-pull alike, the same number the energy polish reached. That is the evidence that
`n = 37` really is infeasible, not merely that the energy objective declined it.

### 4. Walking the horizon (`floor endure`, superseded by 6)

Endurance is the largest feasible `n`. Walk `n` up from the best hold's survival; polish each
from the previous answer (last pitch repeated) *and* from every cold start; keep the best feasible
one; send a horizon nobody made feasible to the safety oracle before calling it a miss; stop
after three misses. It finds the right endurance -- the same ticks as 6 at every `y0` -- but
costs 17 s at `y0 = 8`, and for range it is path-dependent: at `y0 = 8` with the bubble annealed
(5), `lambda = 20` found 26.14 blocks and `lambda = 100` found 25.75, where a larger price on
distance can only have helped.

### 5. Annealing the bubble (`--anneal`, `--shrink`; superseded by 6)

The interior-point schedule: polish, then shrink `margin` and `weight` by `shrink` (0.3) and
polish again, keeping the last stage that is still feasible. It is what makes range work under
the bubble: `y0 = 8` range rose from 25.44 to 26.14 blocks, and showed for the first time that
the range optimum exits *earlier* than the endurance one (79 ticks against 83). Three stages and
six gave the same answer; each costs one more polish per start per horizon, 17 s -> 48 s.

### 6. The interpolated first exit (`floor exit`, **kept -- this is the solver**)

Drop the horizon search. Fly a long fixed cap and score the schedule on where it first goes
under, interpolated along the secant between the two states that straddle the floor:

```
k   = first tick with h_k < 0
t*  = (k - 1) + h_{k-1} / (h_{k-1} - h_k)          endurance
z*  = z_{k-1} + (t* - (k-1)) (z_k - z_{k-1})        range
```

The integer exit tick is a step function of every pitch; `t*` is continuous as the crossing
slides from one tick to the next (at `h_k = 0` both readings give `t* = k`). Ticks after the exit
are dead: the search skips them and the replay stops at the crossing, which is most of the speed.
Against the walk at `y0 = 8`:

```
                     walk (4, 5)             exit (6)
endurance            83 ticks, 17 s          83 ticks (t* 83.83), 0.5 s
range                26.14 blocks, 48 s      26.21 blocks, 0.7 s
```

Same endurance at every `y0` in the table, better range, 30-70x faster, and no bubble, so no
bias. What it does not smooth is a **touch-and-go**: a dip under the floor that comes back up makes
an earlier pair the first crossing, and `t*` drops discontinuously. Below `y0 = 8` nothing dips
twice, so this never bites. Above it, it does -- see the next section.

A replay that survives the whole cap scores the cap plus a value-to-go for the energy it still
holds -- `(TE + y0) / 0.0708` ticks at min sink, or `10.10 (TE + y0)` blocks at the best glide
ratio. Crude, so the cap is not allowed to bind. It starts at `--n 1200`, far past any exit up
to `y0 = 32`, which is nearly free: a dead tick costs one replay step, not a search. If the answer
still survives, `floor exit` doubles the cap (up to `--nmax 4800`) and re-ascends, and a file whose header
says `SURVIVES the cap` holds a guess, not an exit. why? on the first `y0 <= 32` corpus, ten
cells at `y0 >= 27` (range) and `y0 >= 29` (endurance) survived a fixed 450-tick cap, and their
scores were the guess.

The l1 curvature price is quoted in blocks of energy, as in `polish`, and converted into the
score's units: one block of energy is worth `1 / 0.0708 = 14.1` ticks or `10.10` blocks of `z`.
why? charged unconverted, `mu = 1e-4` was 14x (endurance) or 10x (range) weaker than the same
number in `polish`. Dead ticks are reset to the last live pitch after every pass, so the price
never sees the seed's leftover tail either.

The conversion did *not* remove the one-tick nose-down spikes above `y0 = 20`, but they are not
"paid for" either, as this section first claimed. Flattening a spike to its neighbors' mean costs
0.1-3.9 ticks, but that test removes the nose-down itself. Spreading the same total lift dump
(`sum sin^2 p`) over nine ticks scores *better* (`y0 = 16` endurance: score 212.016 -> 212.082;
spreading both lumps and re-ascending, 212.217 with roughness 202 -> 86). The proposed reason: at
pitch >= 0 the up-conversion branch is off and a tick depends on pitch only through `cos^2 p`, so
near 0 the physics is flat (quadratic in `p`) while the l1 price is linear, and a small one-tick
step from 0 always loses. Measured 2026-09-24 17:45 EDT by an independent agent on single cells at
`y0 = 15, 16`. But rounding the price's corner (Huber, 6b) leaves the spikes, while a quadratic
price removes them, so the corner is at best not the whole story.

### 6a. Terminal energy, tail shifts, and a growing cap (kept; measured 2026-09-24 17:00 EDT)

All at `mu = 1e-4`, the price every earlier run used; sums are over the 32 cells `y0 = 1..32`.

* **`--ke c`** adds `c` times the energy left at the crossing (all kinetic there), in score units.
  `c = +1` wrecks both utilities -- it hoards speed instead of flying (range -24%). `c < 0`,
  charging for reaching the floor fast, does nothing for endurance (which already lands slow, 0.85
  blocks of KE) and helps range: `c = -0.3` gave +79 blocks and 6 spikes instead of 15.
* **Tail shifts (`--shift`).** A per-pitch search crawls under the curvature price: moving one pitch
  off a straight line pays for three kinks, so a ramp that should start a tick later advances by
  slivers. At `mu = 1e-3`, `y0 = 16` took 263 passes. A move that shifts every pitch from `t` on
  changes only two second differences; with it the same cell converges in 48 passes. At
  `mu = 1e-4`, `y0 = 16`, it found t* 213.1 against 211.0 with a third of the roughness.
* **A growing cap (`--n 150`).** The ascent returns after the first pass whose flight outlasts the
  cap, and the doubled cap continues from it -- a continuation in horizon, optimizing the early
  flight first. With `c = -0.3`: endurance +272 ticks over the fixed 1200 cap.

Together (`--n 150 --shift --ke -0.3`): endurance +327 ticks (+4.2%) with 6 spikes instead of 18,
range +229 blocks (+4.5%). Range comes out rougher (summed |second difference| 8939 against 6214),
partly because it finds more pumps -- one more climb at `y0 = 24` and `32`. The cost is 1.7x the
core time of growing alone; the slowest cell took 840 s on one core. Data: `runs/floor/v3-ke`
(c sweep), `v4-mu`, `v5-grow`, `v6-shift`.

### 6b. The curvature price's shape, and multi-pitch moves (measured 2026-09-24 19:00 EDT)

`--penalty l1|huber:<d>|l2:<d>` (`l2:2` by default since 2026-09-24 23:45 EDT; `l1`
before, including every run in this section) sets the shape of the price on each second difference `x`: `|x|`,
`x^2/2d` inside `d` degrees and `|x| - d/2` outside, or `x^2/2d` everywhere, all times `mu`.
`--moves tick,box:<k>:..,ramp,shift` sets which sweeps a pass makes: `box:k` adds one change to `k`
consecutive pitches, and `ramp` adds `d*(s-t)` from `t` on, which is coordinate search in the second
differences (see `Move` in `src/bin/floor.rs`). All runs below: `mu = 1e-4`, 30 passes, the
minipump seed, `--n 150`, no bubble; sums over `y0 = 1..32`; chatter counts sign flips of the first
difference with both steps over 2 degrees. `l1` with `tick` reproduces `v7-30pass` exactly.

| variant | endurance Σt* | Σ\|d2\| | chatter | range Σz | Σ\|d2\| | chatter | core-h |
|---|---|---|---|---|---|---|---|
| l1 | 8104 | 4829 | 55 | 5263 | 8759 | 57 | 0.60 |
| huber:0.5 | +3 | 4955 | 47 | +12 | 9460 | 65 | 0.59 |
| huber:2 | +4 | 4677 | 35 | +5 | 9396 | 61 | 0.59 |
| l2:0.125 | -13 | 900 | 0 | 0 | 1426 | 0 | 0.62 |
| l2:0.5 | +22 | 1122 | 1 | -102 | 2259 | 0 | 0.61 |
| l2:2 | +58 | 1839 | 8 | +258 | 3893 | 31 | 0.64 |
| l1, tick+box:3:9:17:33 | -31 | 3487 | 65 | +170 | 4990 | 50 | 1.36 |
| l2:0.5, tick+box | -36 | 1405 | 0 | +311 | 3625 | 4 | 1.40 |
| l1, ramp only | -1050 | 5059 | 18 | -1145 | 4894 | 11 | 0.27 |

* **The quadratic price removes the chatter and, on the sums, costs nothing.** Rounding only the
  corner (Huber) barely changes anything, which argues against the corner mechanism in 6 (a small
  first step losing to a linear price) and points at the large second differences instead: a
  spike's price grows linearly in its height under l1 and quadratically under l2. Not tested
  further than this. Under l1 the minipump seed's 2.5-degree dive plateau at `y0 = 13`
  range stays at exactly 2.5 through every pass, box moves included; under `l2:0.5` it moves to
  3.1-3.7 and `z` improves (72.19 -> 72.31). The plateau was only coordinatewise optimal.
* **Range at `y0 >= 22` is basin luck.** Cells swing by 20% between variants (`y0 = 32`: 485 under
  l1, 608 under `l2:0.5` with box moves), and no variant wins every cell, so the range sums above
  measure which basins each variant fell into as much as the variant itself. Endurance at every `y0`,
  and range below 22, agree across variants within about 2.5%.
* **Ramp-only search is badly conditioned.** One ramp moves every later pitch, so 30 passes get
  nowhere near the per-tick optimum, and it cannot make a sharp pull-up. Its low chatter is that of
  a schedule that barely moved.
* Box moves cost 2.3x the core time.
* About 20 of the 32 optima per mode rest a mid-flight state within 1e-9 blocks of the floor, and
  two of them (`dist_y27_hub0.5`, `dist_y27_l2_2`) go under by 1e-14 when replayed on the laptop
  rather than on the cluster that solved them, exiting 100-200 ticks early. The vis skips them.

Data: `runs/floor/v10-pen` (`buggy-moves/` holds the box and ramp runs from before a fix to how a move
prices a candidate that exits earlier; they differ by at most 0.07).

### 6c. Gradient ascent (`--method grad`; measured 2026-09-25 00:06 EDT, commit 9136906)

`floor exit --method grad` replaces the coordinate sweep with L-BFGS ascent on the exact gradient
of the same score, every pitch moving at once. The default is still `--method tick` (`ascend`);
`grad+tick` runs the coordinate sweep from the gradient's answer at every cap. Seed, growing cap,
dead-tail flattening, the `f32` round and the header's score are the same as `ascend`'s.

**The gradient** (`src/adjoint.rs`) is one replay and one backward sweep, O(n). The tick's
partials (`fall_flying_partials` in `src/sim/entity.rs`) differentiate trig as if it were smooth
-- under `mth_lut` it is a 65536-cell staircase whose derivative is 0 almost everywhere -- and
count each branch of the flight kernel only where the replay took it, with no smoothing. At the
exit the score is differentiated through the secant, `dt*/dh[k-1] = -b/(a-b)^2` and
`dt*/dh[k] = a/(a-b)^2` (plus `1 - f` and `f` on the two `z`s for range), with the crossing's
index held; a survivor is differentiated through its value-to-go. The l2 and Huber prices are
differentiated exactly and l1 by its subgradient (0 at the corner). Checked against central
differences under `libm` in `cargo test`: worst relative error 4e-5 through the crossing (both
modes, with and without `--ke`) and 5e-6 on the whole priced objective.

**The optimizer.** L-BFGS, 10 pairs, a first step of at most 1 degree and after that at most 5
per pitch per step (`--max-step`), and an Armijo line search on the true score: the replay, not
the gradient, decides whether a step is kept. why? the score has a corner at pitch 0, a
staircase at 0.0055 degrees and whole-tick cliffs at a touch-and-go; a fixed-rate method (Adam)
would step across all three blind. `--iters` (2000 per cap) never binds: every run stops at a
stall, after at most 1209 iterations at one cap.

**What the plain gradient cannot see, and what it cost before it was handled.** Both measured on
the laptop, 2026-09-24.

* **Dips.** A pre-exit minimum of the clearance is a constraint the score feels only by falling
  off a cliff. Unhandled, the ascent walks a dip onto the floor and every trial step then crosses
  it: the first version, with neither fix in this list, stalled on `y0 = 16` endurance at
  t* 186.2 (coordinate ascent: 214.9) with a dip 1e-5 blocks off the floor. Each dip under
  one block now adds its own gradient (one more backward sweep) and the step is projected onto
  `dh_j . d >= margin - h_j`. A margin of 1e-4 was not enough: under `mth_lut` a dip parked there
  is crossed by the table's noise at every step size (`y0 = 24` endurance stalled at 298 ticks),
  so the line search also rejects a step that sinks a dip under half the margin, and the margin
  steps down `0.01, 0.003, 0.001` blocks (`--graze`), one stage per stall. The optima therefore
  stand 5e-4 to 5e-3 blocks off the floor at their dips, where coordinate ascent's rest within
  1e-9 -- and every gradient file replays on the laptop to within 1e-3 of the header the cluster
  wrote, where `dist_y27_l2_2` still goes under by 1e-14 (6b).
* **Pitch 0.** The forward-to-up branch is off at exactly 0, and the off branch's pitch-derivative
  there is 0 (lift goes as `cos^2`), so the seed's zeros never move. At a pitch on that switch
  the ascent takes both one-sided derivatives (`climb_switch_partials`) and moves the way that
  climbs. This is the other branch's derivative, used only at pitches sitting on the switch, and
  no smoothing. Without it, over `y0 = 4, 8, .., 32`: endurance 1824 against 2319 ticks, range
  1161 against 1769 blocks.

**Results.** Cluster, `y0 = 1..32`, minipump seed, `--n 150`, `--penalty l2:2`, no bubble;
coordinate ascent is `--moves tick --passes 30 --tol -1e9`, rerun on the same build as the
gradient runs (it reproduces v10's `l2:2` sums). Roughness is Σ|second difference| and chatter
the sign flips of the first difference with both steps over 2 degrees, over live pitches.
Core time is the gradient runs' own `wall` (which leaves out the seed fit, a fraction of a
second) and `times.tsv` for coordinate ascent, which lost 2 of its 64 lines to interleaved
appends (both `y0 <= 8`, a few seconds each).

| variant | endurance Σt* | Σ\|d2\| | chatter | range Σz | Σ\|d2\| | chatter | core time |
|---|---|---|---|---|---|---|---|
| coordinate, 30 passes | 8162 | 1839 | 8 | 5521 | 3893 | 31 | 2130 s |
| grad | +203 | 1156 | 3 | +675 | 4263 | 40 | 18 s |
| grad, `--max-step 2` | +193 | 1118 | 3 | +1337 | 3676 | 20 | 20 s |
| grad+tick, 30 passes | +183 | 1104 | 5 | +538 | 3449 | 31 | 2346 s |

```
 y0   endurance t*: coord    grad  step 2     range z(t*): coord    grad  step 2
  1                  12.66   12.66   12.66                    0.53    0.53    0.53
  4                  36.45   36.45   36.45                    5.86    5.86    5.86
  8                  83.83   83.82   83.82                   26.21   26.21   26.21
 10                 112.48  112.48  112.48                   43.14   43.13   43.13
 12                 140.80  140.79  140.79                   61.51   62.27   62.31
 13                 153.40  154.40  154.43                   72.26   72.99   72.96
 14                 173.35  173.28  173.40                   84.12   84.61   84.50
 16                 214.86  215.32  215.37                  111.24  111.69  111.82
 18                 263.90  262.24  260.43                  142.88  145.50  145.28
 20                 306.40  318.10  317.77                  177.27  186.58  183.44
 22                 349.05  361.72  363.79                  219.28  239.41  239.65
 24                 397.47  413.22  413.31                  275.96  303.74  305.33
 26                 456.36  462.47  462.43                  324.42  419.54  415.00
 28                 500.12  510.93  509.14                  408.07  473.91  447.24
 30                 537.00  562.33  560.25                  481.50  527.77  648.90
 32                 570.43  605.23  598.28                  537.76  577.44 1055.25
```

* **It is about 120 times cheaper and, above `y0` about 12, better.** The slowest single solve
  took 4.9 s, against up to 128 s for coordinate ascent. Endurance gains grow with height, to
  +35 ticks (6%) at `y0 = 32`, flying the same single climb better (one dip each, 606 against
  571 ticks). It loses at `y0 = 14` and `18` (by 1.7 ticks at 18), and with step 2 also at 15
  and 17.
* **Below `y0` about 12 it stops short by up to 0.011 ticks.** There are no dips there; the line
  search stalls when its predicted rise falls under the table's noise, where the coordinate
  scan, which prices every candidate on the real table, still resolves. `grad+tick` closes it.
* **Range is basin luck above `y0 = 22`, more so than in 6b.** The gradient runs find more and
  longer pump laps (`y0 = 26`: 4 dips and 614 ticks, against 3 and 460), but the step size alone
  moves `y0 = 32` from 577 to 1055 blocks, and a laptop run of the same binary found 1308 with
  step 2 and 611 with 5 (the likeliest reason: vanilla's lift `cos` is the platform's libm,
  see `src/sim/mth.rs`).
  Only the endurance sums and range below 22 compare optimizers.
* **Chatter.** Endurance: fewer flips (3 against 8) and 37% less roughness. Range: not
  uniformly. Most counted flips in every variant are the tops of pull-ups (-52 then -49) and the
  peaks of nose-down lumps; the gradient's own are 3-8 degree zigzags at the glide-to-dive entry
  of a late pump at `y0 = 30, 32` (`dist_y30_grad`, ticks 477-483), pitches at or above 0 where
  the physics sees pitch only through `cos^2`. Proposed, not tested: the score is nearly flat in
  how such a lump is spread, and the ascent stalls on table noise before the price's small
  gradient smooths it.
* `grad+tick` is worse than `grad` alone on both sums: its coordinate passes at the smaller caps
  move the schedule the next cap's gradient ascent starts from, and it lands in other basins.

Data: `runs/floor/v11-grad` (`l2_2`, `grad`, `grad_s2`, `gradtick`), drawn by
`tools/plot_floor_profiles.py`.

### 7. Exact `f32` output (a correctness fix, kept)

An exit optimum skims the floor at zero margin. Written to four decimals, one `y0 = 30` schedule
replayed 0.004 blocks through the floor it had cleared. Pitches are written as the shortest
decimal that round-trips their `f32`.

### Multi-start, and the collapse onto a hold

At `y0 <= 8` every start lands on the same optimum, so the hold -13 collapse was not seen there.
It is real higher up. At `y0 = 30`, cap 450: from hold -13 the exit ascent finds a plain glide and
exits at 395 ticks. Seeded with a steady 150-tick cycle tiled end to end, it survives the cap,
doing two full pumps that each bottom out at exactly the floor (clearance 0.000). No
per-tick move gets from a glide to a pump, so **past the heights where pumping pays, the search has
to be seeded with a pump.** The tiled cycle is a poor one: it pumps from the first tick. The
`minipump` seed -- `d` nose-down for `k` ticks, 0 for 10, -40 while `v_y` rises, then 0, with `k`
fitted to the best exit (and `d` on a 2.5-degree grid unless given) -- wins every endurance cell
from `y0 = 14` and most range cells, by up to 16% (`y0 = 19`, 279.0 against 239.7 ticks).
Range above `y0 = 22` still has several local optima: warming each cell from its neighbors' winners
beat every cold seed at `y0 = 29, 31, 32`, and the curve is ragged there.

**Running the corpus.** `tools/floor.sbatch` + `tools/floor_job.sh` run a work list of
`<y0> <time|dist> <name> <init spec>` lines, one array task per node, eight single-threaded solves
at a time, against the tree rsync'd to `~/elytra-atlas` on `cif-cpu` (the same one `sweep` uses); `tools/floor_pick.py <out>
--install runs/floor` keeps the best per cell. The v2 corpus was three cold seeds per cell, then
two rounds of warm starts from the neighbors' winners: 341 solves, 7.8 core-hours, about 25 minutes of wall clock on the rack.

## Infinite flight

From rest, flight becomes infinite at a height between **35 and 36 blocks** -- as found, not
proved. Measured with `floor exit --mode time --n 600`, seeded with the steady 150-tick `lambda = 0`
cycle tiled four times (`runs/steady/nlamsweep/out/n0150_lamP0/tight_t0100.pitches`). A 600-tick
cap cannot tell a sustained flight from a slow death on its own -- surviving it is not the test --
so the test is whether total energy rises lap over lap once the first dive is done:

```
 y0    TE at t = 150    300      450      600     first dive bottoms at
 32         -6.01     -10.83   -19.92   -30.40    the floor
 33         -5.54      -8.90   -15.68   -25.52    the floor
 34         -5.17      -7.33   -11.94   -19.58    the floor
 35         -4.82      -5.56    -7.13   -10.29    the floor
 36         -4.48      -4.19    -3.59    -2.16    the floor      sustained
 40         -4.23      -2.51     0.14     3.93    the floor      sustained
 44         -3.55      -0.56     2.35     6.00    -42.75, free   sustained
 50         -3.55      -0.56     2.35     6.00    -42.75, free   sustained
```

Below 36 every lap loses; from 36 every lap gains. From 44 up the floor never binds and the answer
stops depending on `y0` -- the free first dive from rest goes 42.75 blocks deep -- so between 36
and 42.75 the floor is clipping the first dive, and the clipped dive still leaves enough speed to
climb. For comparison, the shallowest break-even steady cycle in `runs/steady/nlamsweep` (1233
cyclic cells, `dy >= 0`) dives 33.4 blocks below its own start; that sweep starts at `n = 150`, so
a shorter cycle may dive less.

Why this is an upper bound on the threshold and not the threshold. The search is seeded with one
cycle, and a per-tick search cannot reach a structurally different one (see the collapse above).
A shallower cycle, or a different first dive, could lower it. The honest measurement is a
steady-state cycle *under* a floor -- maximize per-lap gain subject to `min y >= -D` and find the
least `D` at which it is still `>= 0` -- which `polish --steady` with a `Floor` could do, and which
`polish` refuses today only because nothing has needed it yet.

## Tricks not tried

* **Lexicographic ranking** (Deb's rules: feasible beats infeasible, then objective, then least
  violation). A drop-in for the per-tick comparison with no weight to tune; not needed once the
  exit objective replaced the bubble.
* **Smoothing a touch-and-go** by scoring a soft minimum over the pre-exit clearance alongside
  `t*`, so that a near-touch is ranked before it becomes a crossing.
* **Velocity jitter** (`Jitter`), which averages the knife edges over starting states, for when
  the uncertainty is in `v0`. A raised floor is not a separate trick: a floor `delta` higher is
  the same problem as `y0 - delta`.

## Running it

```
floor probe
floor exit   --y0 8 --mode time|dist [--init hold:-13|minipump] [--ke <c>] [--shift] [--n 150] [--tol 1e-3] [--out <file>]
             [--method tick|grad|grad+tick] [--iters 2000] [--max-step 5] [--graze 1e-2,3e-3,1e-3]
floor endure --y0 8 [--lambda 20 --anneal 3] [--out <file>]
floor safety --y0 4 --n 37 [--init <spec>]
floor solve  --y0 4 --n 36 [--init <spec>]
floor depth  --file <pitches> [--vy --vz] [--every 50]
```

Init specs: `hold:<p>`, `pump:<p_down>,<k>,<p_up>`, `tile:<file>` (a cycle repeated), a file, or
(`exit` only) `minipump[:<d>[,<k>]]`.
`--mu` and `--limit` are the usual curvature price and pitch limit, defaulting to `1e-4` and `85`
as in `runs/atlas`. `runs/floor/` holds the best schedule per cell (`exit_{time,dist}_y<y0>.pitches`, `y0 = 1..32`),
`runs/floor/v7-30pass/` the with/without-tail-shift ascents, `v8-bubble/` and `v9-bubble-cont/` the floor-bubble ones and
`v10-pen/` the curvature-price shapes and search moves, `v11-grad/` gradient against coordinate ascent; `tools/plot_floor_profiles.py` draws the no-shift v7 run and all of v8-v11,
`runs/floor/v2-cluster/` every solve behind the best schedules,
and `runs/floor/inf/` the infinite-flight scan.
