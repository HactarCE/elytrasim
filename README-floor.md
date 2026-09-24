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

"Best hold" is the best constant pitch for each column separately (0.1-degree grid), so its `z` is
not flown at its ticks' pitch. `t*` is the interpolated exit tick; see below. Every optimized cell
was reached from three different starts -- hold -13, hold 0, and a dive-then-pull -- that agree to
0.003 ticks in `t*` and 0.003 blocks in `z`.

What the optimizer buys over the best constant pitch is nothing at `y0 <= 2`, one tick at 3 to 5,
and six ticks (8%) and 3.6 blocks of range (16%) at `y0 = 8`. The gain grows with height.

**There is no dive.** At these heights no optimum ever pitches nose-down:

* **Endurance** is one ramp: pitch 0 at the first tick, then nose-up at a near-constant rate --
  about -0.25 deg/tick at `y0 = 8` -- to about -22 at the exit. You build forward speed level
  and spend it on lift progressively, and the pitch the ramp ends on is well past min sink (-13).
* **Range** is a hold then a pull: pitch -0.0055 for 36 ticks at `y0 = 8`, then nose-up at
  0.6-1 deg/tick to -23, held to the exit. The hold sits just past the corner at 0, where the
  forward-to-up branch switches on.

So the expected "mini pump" is only its second half, look level then look up. The first half,
looking *down* to gain speed, does not pay from rest at these heights: from `v0 = 0` the dive's
speed is bought with height the flight does not have.

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
ratio. Crude, and it only has to rank schedules that all outlive the cap.

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
to be seeded with a cycle.**

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

### 8. A raised floor (`--raise`, kept as an option)

Optimize against a floor `delta` higher, report against the real one: a margin the schedule keeps
everywhere, so that a skimming optimum is one a hand can fly. At `y0 = 8`, where the floor is
touched only at the exit, it is cheap:

```
delta      endurance t*     range z(t*)
0          83.83            26.21
0.1        83.80            26.18
0.5        83.32            25.66
1          82.45            24.87
```

Where the floor is skimmed mid-flight -- the pumps above `y0 = 30` -- a raise of `delta` is exactly
a floor `delta` higher, so it moves the infinite-flight threshold up by `delta` and nothing subtler.

## Tricks not tried

* **Lexicographic ranking** (Deb's rules: feasible beats infeasible, then objective, then least
  violation). A drop-in for the per-tick comparison with no weight to tune; not needed once the
  exit objective replaced the bubble.
* **Smoothing a touch-and-go** by scoring a soft minimum over the pre-exit clearance alongside
  `t*`, so that a near-touch is ranked before it becomes a crossing.
* **Velocity jitter** (`Jitter`), which averages the knife edges over starting states. It is the
  honest version of a raised floor when the uncertainty is in `v0`.

## Running it

```
floor probe
floor exit   --y0 8 --mode time|dist [--n 150] [--init hold:-13] [--raise 0.5] [--out <file>]
floor endure --y0 8 [--lambda 20 --anneal 3] [--out <file>]
floor safety --y0 4 --n 37 [--init <spec>]
floor solve  --y0 4 --n 36 [--init <spec>]
floor depth  --file <pitches> [--vy --vz] [--every 50]
```

Init specs: `hold:<p>`, `pump:<p_down>,<k>,<p_up>`, `tile:<file>` (a cycle repeated), or a file.
`--mu` and `--limit` are the usual curvature price and pitch limit, defaulting to `1e-4` and `85`
as in `runs/atlas`. `runs/floor/` holds the table's schedules (`exit_{time,dist}_y<y0>.pitches`, from hold -13)
and `runs/floor/inf/` the infinite-flight scan.
