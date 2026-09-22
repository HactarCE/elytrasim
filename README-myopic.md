# Myopic metrics of the optimal climb cycle

Which one-tick-ish rules does the global optimum agree with, phase by phase?

Run things with `cargo run --release --bin myopic -- <subcommand>`; the subcommands are
documented at the top of `src/bin/myopic.rs`. Everything is measured against `sim`'s physics
with yaw pinned to zero, so the state is just `(v_y, v_z)` plus height.

> **Units note.** Energies are now in **blocks** — `KE = |v|^2/(2g)`, `PE = y` exactly — so the
> objective is `TE + w*z` with both terms in blocks. Every `w` in this document predates that and
> is in the old convention, where `PE = g*y`; multiply by `1/g = 12.5` to read it in current
> units, so the family below spans `w = -0.125 .. 0.250` today. Only the labels move: every
> angle, rate and pitch here is unchanged, because rescaling the objective cannot move an argmax.

## The reference

`REPLAY_PITCHES_300` is a genuine closed cycle — replaying it returns the velocity to its start
within `6.5e-7` b/tick — so it is a periodic orbit, not a 300-tick trajectory that happens to end
well. It climbs **21.50 blocks per cycle = 1.4335 b/s**. `_200` and `_400` do not close and only
manage ~0.95 b/s.

    myopic profiles

Because greedy is optimal on the last tick by construction, agreement measured near the end of a
fixed horizon is suspect. The **horizon-free** check is: tile the cycle three times, re-polish the
900-tick schedule, and read the middle cycle, where the tail is 300+ ticks long.

    myopic polish tiled900.txt 30 > polished900.txt
    myopic score polished900.txt 300 214 300

That re-optimization moves the cycle by 2.0° mean pitch and improves it 0.6% — the error bar on
everything below.

## Phase 1, dive (~190 ticks): hold the flight-path angle

The optimal pitch is **the pitch at which γ stops changing**: at the optimum's own states, γ after
the tick equals γ before it to within 0.02–0.4°. It is parameter-free and fits to **0.27° median /
0.73° RMS** over ticks 40–193, while pitch itself sweeps 13°→47°.

Note this does *not* mean the nose points along the velocity vector — by the end of the dive the
nose is ~30° below it. What is held is the velocity vector's direction.

The hold is not exact: γ decays first-order from ~25° toward a floor near 16.8°, and flying an
exact hold from the moment the dive starts keeps whatever γ you entered with and loses height.
That was originally written up as "the leak is the whole rule", with a fitted rate `k ≈ 0.04–0.055`.
**That was wrong, and the leak is now gone** — see "The leak was an entry correction" below. The
dive after its first ~30–60 ticks needs no constant at all.

**The floor is derivable.** The steady glide that maximizes forward speed is at pitch `53.35°`,
`v_z = 3.389`, and its flight-path angle is `16.58°` (`myopic eq`). The dive is a slow approach to
the fastest steady glide.

Snapping straight to that angle instead of holding is a much weaker rule — it saturates at −90°
early and scores 34° RMS. The two rules bracket the optimum, but loosely (mean bracket 29°), and
the optimum sits ~99% of the way toward the hold end.

**Trap:** at low speed two pitch branches reach a given γ, and only the nose-down one accelerates.
A naive `argmin |γ' − γ*|` oscillates between them and never builds speed (that scored −3.0 b/s).
`bug_gamma_to` scans for the last upward crossing and then bisects.

The pitch it sweeps has two separate stories — see "The dive has two pitches" below.

## Phase 2, snap (~14 ticks): pitch 0, held until the forward speed peaks

Literally zero, for about fourteen ticks. One-tick greedy independently says 0 here too.

The pitch was never the open part of this phase — the *stopping time* was. What ends the hold:

**Leave on the first tick at which holding pitch 0 would no longer raise `v_z`.** One tick of
lookahead, no fitted constant. `opt::vz_peaked` asks the tick map; in exact rationals the hold
moves `v_z` by `-0.01 v_z - 0.0891 v_y + 0.001782`, so the rule fires on the straight line

    v_z >= 0.1782 - 8.91 v_y

which passes through the pitch-0 steady glide at `(-0.14949, 1.51017)`. Ask the map rather than
the line: the margin at the crossing is about `5e-4` b/tick and the sim's drags are `f32`.

It reads as *no pitch* can raise `v_z` further, not merely pitch 0: for `p >= 0` the map depends
on pitch only through `lift = cos^2 p` and `d(v_z')/d(lift) > 0` whenever `v_y < -0.04`, so 0 is
the argmax there; for `p < 0` the forward-to-up branch switches on and takes `v_z` away outright.

On `REPLAY_PITCHES_300` it is exact: tick 207 is the last tick that still buys forward speed
(`+5.0e-4`) and the optimum holds it; tick 208 would lose it (`-7.6e-4`) and the optimum flicks.

`tools/glide_phase.py <corpus>...` scores it against the optimum's own departure. At `lambda = 0`
on the 151 periodic cells of `runs/steady/nlamsweep`: median miss **0 ticks, 98% inside one
tick**. Competing stopping rules on the same cells, by the same measure:

| rule | median | exact | inside 1 |
|---|---|---|---|
| **`v_z` has peaked** | **0** | 28.5% | **98.0%** |
| glide ratio at its steady max (`gamma <= 5.653`) | +1 | 33.8% | 98.0% |
| `v_y >= -0.260` (the policy's fitted constant) | 0 | 48.3% | 92.7% |
| speed has peaked | -1 | 10.6% | 58.9% |
| 1-tick dTE argmax leaves 0 | +5 | 0% | 0% |
| 20-tick dTE argmax leaves 0 | -12 | 0% | 0% |

The two dTE rules bracket it, one late and one early, which is the same story as the gain phase's
lookahead but with a different answer: the lookahead that lands exactly on the departure is
**10** (median; 10-14 over the 21 of 51 sampled `lambda = 0` cells where some `n` lands exactly at
all, the rest stepping over it), against ~20 in the gain phase thirty ticks later. One `n` does
not serve both.

Flying it costs nothing. `myopic policy opt leak vzpeak` swaps the tuned `vy_flick = -0.260` for
the rule and retunes everything else: **1.38572 b/s, 96.7% of the optimal cycle**, against
**1.37824 b/s, 96.2%** for the tuned threshold — better, on one fewer tuned scalar.

### The miss is monotone in the price on distance

`v_z has peaked` is exact for a pure climb and biased either way once distance is priced, with
the sign you would want (`runs/steady/nlamsweep`, 1233 cells). `runs/atlas/mapsweep` shows the
same monotone walk across its own 5282 cells at quarter-lambda spacing, offset about a tick late
throughout — it is a free-endpoint corpus at a single `v0`, and its `lambda = 0` median is +1:

| lambda | -2 | -1 | **0** | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|---|---|
| median miss, ticks | +1 | +1 | **0** | -1 | -1 | -2 | -2 | -3 | -4 | -4 |
| inside one tick | 95% | 97% | **98%** | 79% | 64% | 46% | 35% | 11% | 0% | 0% |

Paid for ground covered, the optimum holds the level glide past the peak of `v_z` and keeps
collecting distance; charged for it, it leaves early. The threshold in `dv_z` is close to linear
in `w` — the midpoint of the held/left bracket runs `+1.9e-3` at `lambda = -2` through `0` at
`lambda = 0` to `-6.1e-3` at `lambda = 7`, a slope of about `-0.0146` per unit `w`. That slope is
measured, not derived, and nothing here explains it.

**What is not the rule: a threshold on the sink rate.** `v_y` at the departure is the tightest
single invariant in the corpus (7-15% spread against 22-50% for `v_z`, speed and `gamma`), which
makes `v_y >= -0.251` look like the answer and is where the policy's fitted `-0.260` came from.
It is not flat, though: it runs -0.242 at `n = 150` to -0.260 at `n = 350` inside `lambda = 0`
alone, and moves with the departure `v_z` in the same corpus. The spread is structure, not noise.

## The hold ends at a corner of the physics, and the switch is derivable

The rule above is myopic but only exact at `lambda = 0`. The exact condition, for every price, is
a corner switch, and the constant in it comes out of the tick map with nothing fitted.

Pitch 0 is a corner (see the glide section below). Approach it from the nose-up side and the only
term that is linear in pitch is the forward-to-up branch, so the only direction the control can
move the state in is

    d(v_y')/d(lean) = -0.128 * v_z * DRAG_Y        d(v_z')/d(lean) = +0.036 * v_z * DRAG_Z

`0.128 = 3.2 * 0.04` is the forward-to-up gain; `0.036 = 0.9 * 0.04` is what the turning term
leaves of the matching `v_z` loss. Both are proportional to `v_z`, so the rate at which a nose-up
tick trades forward speed for upward speed is a pure constant:

    0.128 * DRAG_Y / (0.036 * DRAG_Z) = (3.2 / 0.9) * (DRAG_Y / DRAG_Z) = 3.519641

Pontryagin's condition at a corner is that the one-sided derivative of `mu . f` does not point out
of the admissible side. So the optimum holds pitch 0 exactly while

    0.036 * DRAG_Z * mu_z  >=  0.128 * DRAG_Y * mu_y

— **while forward velocity is priced at least 3.5196 times upward velocity**, which is the
exchange rate the elytra itself offers. The moment the prices cross that rate, taking the trade
beats banking more forward speed, and the flick starts.

### The other end is a corner too, on the other side of it

Entering the hold is the same argument run on the nose-down side. For `p >= 0` the tick map
depends on pitch *only* through `L = cos^2 p`, which is maximal at `p = 0`, so there is no linear
term and the question is not a derivative in pitch but the sign of `d(mu.f)/dL` at `L = 1`:

    d(v_y')/dL = DRAG_Y * (0.056 - 0.1 v_y)      d(v_z')/dL = -0.09 * DRAG_Z * (v_y + 0.04)

(both exact against the tick map). So the optimum holds 0 rather than pitching down while

    DRAG_Y * (0.056 - 0.1 v_y) * mu_y  >=  0.09 * DRAG_Z * (v_y + 0.04) * mu_z

Unlike the exit, this one keeps a `v_y` in its coefficients — `v_z` cancels out of the exit
condition because both of its components carry a factor of `v_z`, and nothing cancels here. That
is why no single state quantity is constant at the entry: the threshold itself moves with the
state. Looking for an invariant there was the wrong search.

### Both ends, on the whole steady corpus

`myopic adjoint <profile> dump` solves `mu` with no free parameters (periodicity closes it) and
prints it per tick. Across **all 1233 periodic cycles** of `runs/steady/nlamsweep`, `lambda -2..7`,
`n 150..450`, against the ticks the optimum actually enters and leaves the hold:

| | -1 | **0** | +1 | worse | exact | inside one tick |
|---|---|---|---|---|---|---|
| entry switch | 346 | **826** | 61 | 0 | 67.0% | **100%** |
| exit switch | 97 | **1136** | 0 | 0 | 92.1% | **100%** |

**Inside one tick on every cell, at every price.** The exit is never late; the entry misses both
ways. Nothing is fitted in either condition — `0.056`, `0.09`, `0.036`, `0.128` and the two drags
are all read off the tick map.

Write the exit as the linear inequality rather than the ratio `mu_z/mu_y`: `mu_y` passes through
zero a few ticks into the hold (upward velocity is worth less than nothing while you are still
diving), and the ratio blows up there while the inequality stays well behaved.

    python3 tools/glide_phase.py runs/steady/nlamsweep runs/atlas/nsweepv0fine runs/atlas/mapsweep

### Why no myopic rule can be exact here

`mu` needs the whole future, and in this phase there is no continuation-free way to guess it. Take
the price you would assign if you were going to glide at pitch 0 forever — the fixed point of
`mu = c + A_0^T mu` with `c = (1, w)`, which is parameter-free and closed-form:

    mu_z = w / (1 - DRAG_Z) = 100 w        mu_y = (1 - 8.91 w) / 0.118 = 8.4746 - 75.508 w

That ratio reaches 3.5196 only at `w = 0.0816`, i.e. `lambda = 1.25`. So the coasting price says
"flick immediately" for every cycle below that and "never flick" above it: bang-bang, with no
interior switch anywhere. At `lambda = 0` it says forward speed is worth exactly nothing, which is
correct for a glide that continues and wrong for one that ends in a zoom. **All of the hold's
value at `lambda = 0` is in the zoom that has not happened yet**, so a rule that cannot see the
zoom cannot price the hold. That `v_z has peaked` lands on the right tick anyway is not explained
by anything here.

Still open:

- **A myopic rule for the entry.** `vz_peaked` covers the exit; nothing here covers the entry
  without `mu`. The state spread at the first flat tick is wide — over three corpora `gamma`
  13-33%, the glide ratio 15-37%, `v_y` 24-36%, speed and `v_z` 30-50% (the `entry` table in
  `tools/glide_phase.py`) — and the corner condition above says that is what to expect, not a
  failure to look hard enough.
- **A myopic rule that tracks `lambda`** at the exit. The measured correction to `vz_peaked` is
  about `-0.48 * lambda` ticks, or a threshold of `-0.0146 w` in `dv_z`; neither constant is
  derived.
- The entry is **not** a gentle ramp, which is what I assumed before measuring it: the largest
  single step into the hold is 14.98 deg (10-90% 12.40..19.87) against 15.97 deg (10.37..21.76)
  out of it. Both ends jump. The dive's pitch coming down over the preceding ticks is the dive
  rule still running, not a transition.

## The steady glide against pitch, and the corner at 0

`myopic glide` sweeps the terminal velocity of the constant-pitch tick map over the whole pitch
domain; `myopic crit` reads its three critical points from both sides;
`tools/plot_glide.py` draws it. Horizontal velocity is `sim`'s `z` — yaw is pinned to zero — but
is written `vx` here, which is the convention everywhere outside the sim. **Velocities below are
blocks/second**, not the `sim`'s blocks/tick; the glide ratio is a ratio of two velocities, so it
is dimensionless and reads the same either way.

| critical point | pitch | `vy` | `vx` | glide ratio | γ |
|---|---|---|---|---|---|
| max glide ratio | **0** | −2.9898310 | 30.2034233 | **10.10205** | 5.6533° |
| min sink | **−13.058** | **−1.4157544** | 8.7913229 | 6.20964 | 9.1484° |
| max forward speed | **53.366** | −20.1912793 | **67.7758263** | 3.35669 | 16.5895° |

Nothing is unbounded: at either pole the glide degenerates to a vertical fall at `vy = −78.400`,
`vx = 0`. The range is merely wide — `vy` spans 55× between min sink and a vertical dive — which is
why the plot gives `vy` both a full-range and a zoomed panel.

The last entry refines the `53.35° / 3.389 b/t / 16.58°` quoted under the dive's γ floor. That number
came from `ceiling()`, which sweeps at 0.05° and deliberately does not refine; the top is flat to
`1e-9`, so the two agree to well inside anything that matters.

**Pitch 0 is a corner, not a jump, and it is one-sided.** For `p >= 0` the tick map depends on
pitch *only* through `lift_force = cos^2(p)`: yaw is zero, so `look_angle.x` is zero and every use
of `look_angle.z / look_hor_length` collapses to 1, cancelling `cos(p)` out of the direction
terms, and the `lean_angle < 0` branch is off. `cos^2` is even, so every curve above is even and
flat to second order on the right of 0. For `p < 0` that branch switches on with a term linear in
`sin(p)`. So the one-sided derivatives of the glide ratio are

    nose-up   0.405 per degree      nose-down   3.42e-3 per degree^2, no linear term

and 0 is a genuine maximum — there is no second attractor and no discontinuity. Iterating the map
from 96 spread initial conditions lands on the same fixed point at every pitch on a 0.25° grid
across the domain, to `1e-6`. But the max is enormously lopsided: **1% of the glide ratio costs
0.29° of nose-up, or 5.45° of nose-down** (measured, not extrapolated from the slopes; the nose-up
branch curves over that distance). If the snap's pitch 0 has to be approximated, err nose-down.

Minecraft's own trig sharpens this. `Mth.sin` is a 65536-entry table, so the `p < 0` branch is
quantized into steps of `2*pi/65536 = 0.0055°`, and inside `|p| < 0.0055°` — the index
`(int)(p * 10430.378)` truncating to 0 — it returns exactly zero and the branch is dead outright — the corner becomes a short flat shelf followed by a staircase. The
`p >= 0` side is bit-identical between `libm` and `mth_lut`, exactly as `sim/mth.rs` predicts,
because `cos(p)` cancels and `lift_force` is a `double` `Math.cos` in vanilla either way. The
min-sink pitch does move, to about −13.05, and its neighborhood is a jittery staircase rather
than a smooth peak, so that critical point carries roughly ±0.01° of quantization noise. The max
forward speed is unmoved.

    myopic glide 0.01 > runs/glide/glide_libm.csv
    myopic glide 0.01 --trig mth_lut > runs/glide/glide_mth.csv
    myopic glide 0.0002 -0.06 0.06 > runs/glide/zoom_libm.csv
    myopic glide 0.0002 -0.06 0.06 --trig mth_lut > runs/glide/zoom_mth.csv
    tools/plot_glide.py runs/glide/glide_{libm,mth}.csv runs/glide/zoom_{libm,mth}.csv runs/glide/glide.svg

## Phase 3, flick (~6 ticks): ramp to about −88°

Nothing to find. This is where two independently optimized cycles disagree most (11°), so the
values genuinely do not matter. It does need to be a ramp rather than a step, though a chunk of
the measured cost of stepping is probably the optimizer over-fitting the tick grid.

## Phase 4, gain (~86 ticks): argmax ΔTE over ~20 ticks, not 1

> The lookahead here is fitted. "The gain phase without a lookahead" below removes it: the
> stationary pitch has a closed form, the horizon is the apex, and what is left is one scalar
> handed over by the dive. This section is what the fitted version costs and how far it holds.

Same metric as elytrasim's `argmax_over_pitch_of_delta_energy`, with the lookahead extended.
Holding pitch constant for `n` ticks and taking the argmax of the total-energy change:

| n | 1 | 2 | 4 | 8 | 12 | 16 | 20 | 24 | 32 | 48 |
|---|---|---|---|---|----|----|----|----|----|----|
| gain-phase RMS, ° | 17.9 | 16.9 | 15.0 | 11.4 | 7.5 | 3.8 | **1.1** | 3.8 | 7.1 | 10.8 |

One tick is not merely imprecise: it pins to the −90° bound for ticks 214–230 while the optimum
recovers through −79° to −45°, and is still 40° off at tick 228.

**Is n = 20 overfit?** No. Re-optimizing the cycle against `TE + w·z` gives a family of optimal
cycles trading climb against ground covered (`myopic polish <file> <passes> <w>`, then
`myopic family`). Negative w is the operationally interesting half — climbing in as little space as
possible — since extra distance can always be bought by flying more cycles.

| w | | climb b/s | dist b/s | dive hold-γ median | best n | RMS at best n | RMS at n=1 |
|-------|-----|--------|--------|------|----|------|------|
| -.010 | tightest | 1.1940 | 19.233 | 1.15 | 18 | 1.65 | 22.1 |
| -.005 |     | 1.3926 | 21.292 | 0.39 | 19 | 1.21 | 22.6 |
| -.002 |     | 1.4321 | 22.205 | 0.35 | 19 | 1.49 | 23.2 |
|  0    | reference | 1.4425 | 23.300 | 0.34 | 19 | 2.02 | 22.7 |
|  .002 |     | 1.4340 | 24.368 | 0.42 | 20 | 2.15 | 23.2 |
|  .005 |     | 1.3782 | 25.509 | 0.61 | 20 | 2.81 | 23.8 |
|  .010 |     | 1.2338 | 27.096 | 0.60 | 21 | 2.98 | 26.6 |
|  .020 | longest | 0.6579 | 30.164 | 0.73 | 23 | 6.22 | 30.8 |

Across 19–30 b/s of ground covered the best lookahead stays in **18–23**, and the dive rule stays
well under a degree. If anything the rules fit the min-distance cycles better than the reference.

The dive's γ floor *is* cycle-specific, and it moves monotonically: 17.4° at w = -.003 down to
14.7° at w = .020, crossing the fastest-steady-glide angle within a whisker of w = 0. Below
w = -.005 there is no floor to report — γ climbs through the whole dive. See the last section.

`n` is a compromise rather than a constant, though. The *implied* lookahead — the `n` whose argmax
lands exactly on the optimum, from `myopic probe` — runs ~20 at the start of the gain phase, sags
to ~12 around tick 275, then climbs steeply near the end. It is not the time remaining to the apex:
that falls monotonically 94 → 10 across the same window, so the two are anticorrelated over the
last thirty ticks.

**Past tick ~280 the implied lookahead means nothing**, because the family stops disagreeing
(`myopic sweepn` dumps the whole tick × lookahead matrix). The spread across n = 12..24 falls from
28° at tick 216 to under 1° by tick 280, and even n = 1 against n = 60 differ by only ~2° there. On
the horizoned 300-tick problem the implied lookahead duly collapses to 1 near the end; horizon-free
it climbs past 40. Both are fitting sub-degree noise. The quantity is only determined in the first
half of the gain phase, where the fan is wide.

What *is* clean is a bracket. **`n = 12` is nose-up of the optimum at every tick of the gain phase
and `n = 37` is nose-down at every tick** (76/76 ticks over rel 220-295, 82/86 over the whole
phase), mean width 7.8°. Every lookahead between them crosses the optimum somewhere; nothing
outside ever does.

Narrower still is the set that is ever actually *right*. Over the window where the family still
disagrees — rel 214-278, before the n=12..24 spread falls under a degree — the implied lookahead
takes exactly nine values, **12 through 20**, and walks them **monotonically from 20 down to 12**.
So the gain phase is not "use n = 20", it is "start on a twenty-tick horizon and shorten to twelve
as you climb". A fixed 20 is the best single stand-in and costs about a degree.

This is the useful property for a cloud-of-bugs display: draw n = 12..20, the cloud's width is the
uncertainty, and the member you should be on migrates from the nose-down edge to the nose-up edge
as the climb progresses.

Note the ΔTE family is not merely imprecise in the **dive** — it is bimodal there, splitting
between "stay level" (short n) and "give up and zoom now" (long n, around -40 to -52), and
describing the optimum at neither.

### Pricing the lookahead with `ΔJ` buys a tick or two of horizon and nothing else

`argmax ΔTE` is blind to the price on distance, so the natural way to carry it to `λ ≠ 0` is to
maximize the cycle's *own* objective over the same held rollout: `ΔJ = ΔTE + w·Δz`. That is the
right generalization and not merely the obvious one. Expanding `ΔTE = (|v_n|² − |v_0|²)/2g + Σ v_y`,

    ΔJ = Σ_k (v_y + w·v_z)  +  KE(v_n) − KE(v_0)

— the cycle's own running reward accumulated over the rollout, plus a terminal value. So it is the
climb's own problem truncated at `n`, with the control frozen and the apex price `mu(apex)` replaced
by the energy gradient `v_n/g`: it fixes the running reward, which `ΔTE` gets wrong the moment
`w ≠ 0`, and leaves the terminal price wrong, which is what `n` was standing in for all along. At
`w = 0` the two are the same expression, which is the harness check.

**It does not describe the priced cycles better.** `myopic djn <profile>` scores both rules against
the optimum's own pitches over the ticks where its climb is nose-up and off its own `|pitch| ≤ 85`
bound; `python3 tools/dj_phase.py runs/steady/nlamsweep` runs it over all 1233 periodic cells. `n`
stays tuned for both rules, so the fair comparison is each at its *own* best lookahead:

| λ | -2 | -1 | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|---|---|
| `ΔJ` best n | 18 | 18 | 19 | 19 | 19 | 19 | 19 | 19 | 19 | 18 |
| `ΔJ` RMS, ° | **1.32** | **1.52** | 1.78 | 1.77 | 2.48 | 3.36 | 4.26 | 4.91 | 6.30 | **8.69** |
| `ΔTE` best n | 18 | 18 | 19 | 20 | 19 | 20 | 20 | 21 | 21 | 20 |
| `ΔTE` RMS, ° | 1.73 | 1.68 | 1.78 | **1.63** | **2.32** | **3.23** | **4.19** | **4.85** | **6.29** | 8.72 |
| cells where `ΔJ` wins | 99% | 97% | — | 10% | 21% | 30% | 40% | 43% | 62% | 71% |

Every entry is a median over that λ's 31–151 cells, so the last row is the one to read: it pairs
the two rules cell by cell, which the two medians above it do not.

It helps where distance is *charged* for — 0.15 to 0.40°, on 97–99% of cells at `λ < 0` — and costs
a little where distance is paid for, which is the half the rule was invented for. The 20 cells that
neither climb nor sink (`|Δy| < 1` block over the whole cycle, all at `λ` 3–6) behave like the rest:
`ΔTE` is closer on 14 of the 20, by 0.06–0.40°, and `ΔJ` on the other six — all `λ` 5–6 — by
0.06–0.12°.

**The price and the lookahead turn out to be the same knob**, which is why adding one cannot buy
anything the other did not already have. `myopic djn <profile> equiv` matches each `ΔJ` lookahead to
the `ΔTE` lookahead whose pitches are closest and reports what is left over. On `n0400_lamP4`
(`λ = 4`, `w = 0.261`):

| `ΔJ` at n | 10 | 15 | 20 | 25 | 30 |
|---|---|---|---|---|---|
| closest `ΔTE` n | 11 | 16 | 21 | 27 | 33 |
| residual between the two, ° | 0.70 | 0.62 | 0.54 | 0.57 | 0.58 |
| `ΔJ`'s own error against the optimum, ° | 10.9 | 6.4 | 3.2 | 5.2 | 7.5 |

**One tick of horizon**, and the leftover is six times smaller than the error the rules are trying
to explain. At `λ = -2` the shift runs the other way (n → n−1 at n = 20, n−3 at n = 40) and at
`λ = 3` it is n → n+1 to n+2; the residual is 0.4–0.8° everywhere. So the win at `λ < 0` and the
loss at `λ > 0` are that half-degree of leftover happening to point the right way, not the price
term doing its job.

Why they should coincide, as far as this goes: the pitch a linear price picks depends on the state
only through `v_z` and the *ratio* `mu_z/mu_y` — see the closed form below, where `v_y` does not
enter — so two rules that trace the same ratio along the arc fly the same pitches whatever their
parameters are called. Raising `w` and shortening `n` both raise that ratio. A held-`n` rollout is
not literally a one-tick maximization of `mu.f`, so this is the reading the measurement supports
rather than a derivation of it; what the measurement establishes on its own is the half-degree.

Run the knob the other way and it says the same thing. `myopic djn <profile> wsweep` re-scores the
rule at a grid of *its own* `λ`, independent of the cycle's, and reports the best lookahead and RMS
at each. The best lookahead walks monotonically down as the rule's price goes up — 25 at `λ_rule` =
−12 to 17 at +12 — and the RMS has a shallow interior minimum that **does not sit at the cycle's own
price and barely moves with it**: `λ_rule` = −4.5 for a `λ = 0` cell, −5.5 and −6.0 for two `λ = 4`
cells. A knob that read the price would track it.

**Where the error actually is.** Split the climb three quarters / one quarter along its arc and the
λ-dependence is almost entirely in the last quarter, where the pitch comes back to 0 — the same
window the exact climb-to-apex rule degrades in:

| λ | -2 | -1 | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|---|---|
| `ΔTE`, first three quarters, ° | 1.70 | 1.66 | 1.48 | 1.44 | 1.58 | 1.74 | 2.13 | 3.27 | 6.10 | 8.72 |
| `ΔTE`, last quarter, ° | 1.81 | 1.73 | 2.11 | 2.36 | 4.24 | 6.85 | 8.90 | 13.79 | 17.48 | 23.03 |

Through the body of the climb a twenty-tick lookahead is worth about two degrees at `λ = 4`, against
1.5° at `λ = 0`. It is the approach to the corner that no fixed horizon survives, and the corner
arrives earlier and harder the more distance is worth — which the corner condition
`mu_z/mu_y ≥ GAIN_RATE` below already predicts.

**Read the absolute numbers at `λ > 0` with the corpus's own cut in mind.** A `--steady` profile is
closed in velocity but optimized with its terminal velocity free, so its first-order conditions
carry the open-horizon terminal price at the cut and its pitch jumps there: median `|p[0] − p[n−1]|`
runs 7.5° at `λ = −2` to 24.9° at `λ = 7`, against a typical per-tick step of 0.28° falling to
0.03° over the same range. Where that cut lands inside the climb moves the score a long way. Over
the 21 cut phases of the one `n = 300, λ = 4` cell, `ΔTE` at its best `n` scores **1.34°** when
only a tenth of the climb sits past the cut and **5.95°** when nearly half does, monotonically in
between. The comparison above is unaffected — both rules are scored on the same ticks — but the
level is not a property of the price alone.

## The gain phase without a lookahead: two clocks and one handoff

`n = 20` is a fitted constant, and "The exact one-tick rule" below says what it stands in for:
`argmax ΔTE over n` prices the state it lands on with the energy gradient `(v_y, v_z)/g` and
carries that back along a held-pitch rollout, so `n` is whatever makes that the best stand-in for
the real `mu`. On the climbing arc the real `mu` can be written down. Once it is, the lookahead goes
away and one number takes its place — a number the climb cannot derive, because it is what the
*dive* will pay for what the climb hands over.

`myopic gain <file> [w] [bvp] [dump]` measures everything here;
`python3 tools/gain_phase.py runs/steady/nlamsweep` runs it across the corpus. The polished family
is the reference cycle tiled three times, re-polished at each `w`, cut apex-to-apex and re-closed:

    myopic polish tiled900.txt 40 <w> > p900.txt        # tiled900 = REPLAY_PITCHES_300 x3
    myopic cyclecut p900.txt > cut.txt
    cargo run --release --example wobble -- cut.txt 0 > cyc.pitches
    myopic gain cyc.pitches <w> bvp

Pass `w` to `gain` explicitly: a profile header's own `w` is derived from its `lambda`, so a
hand-written header cannot carry it.

### The pitch, in closed form, with `v_y` absent

Take the arc where the down-to-forward branch is off — `v_y + g(0.75 cos²p − 1) ≥ 0` — and the
pitch is nose-up. There the tick map is only

    v_y' = DRAG_Y (v_y + g(0.75 cos²p − 1) + 0.128 s v_z)        s = sin|p|
    v_z' = DRAG_Z v_z (1 − 0.036 s)

so `d(mu . v')/ds = 0` is one line — the price of the up you buy equals the price of the forward
you spend:

    mu_y (GAIN_UP v_z − GAIN_LIFT s) = GAIN_FWD mu_z v_z

    GAIN_UP = 0.128 DRAG_Y     GAIN_LIFT = 1.5 g DRAG_Y     GAIN_FWD = 0.036 DRAG_Z

and it solves for the pitch outright:

    sin|p| = v_z (GAIN_UP mu_y − GAIN_FWD mu_z) / (GAIN_LIFT mu_y)

**`v_y` does not appear.** Given the prices, the climb's pitch is set by the forward speed and by
nothing else about where it is. Read backwards it gives the price ratio a pitch implies,
`mu_z/mu_y = GAIN_RATE − (GAIN_LIFT/GAIN_FWD)·s/v_z`, where `GAIN_RATE = GAIN_UP/GAIN_FWD =
3.519641` is the elytra's forward-to-up exchange rate — the same constant that ends the pitch-0
hold.

Against the solved `mu` this reproduces the polished cycle's own pitches to **0.072° RMS, 0.014°
median** over the 90 interior ticks of its climb, and stays under a quarter degree across a
polished family spanning `w = −0.125 .. 0.250` (the reference cycle is the `w = 0` column):

| w | −.125 | −.0625 | −.025 | 0 | .025 | .0625 | .125 | .250 |
|---|---|---|---|---|---|---|---|---|
| stationary-pitch RMS, ° | 0.009 | 0.164 | 0.044 | 0.072 | 0.251 | 0.032 | 0.052 | 0.163 |
| `kappa = mu_z(apex)` | 2.77 | 6.11 | 9.41 | 11.53 | 13.82 | 16.93 | 22.27 | 32.22 |
| `mu_y(apex)` | 6.26 | 3.05 | 0.80 | 0.80 | −0.99 | −0.31 | −1.90 | −5.10 |

### Both prices are clocks, on complementary arcs

The Jacobian is triangular on each arc of the cycle, and in *opposite* directions:

* where the down-to-forward branch is off, `dv_z'/dv_y = 0`, so `mu_y` obeys
  **`mu_y,t = 1 + DRAG_Y·mu_y,t+1`** — autonomous, with no state and no control in it;
* where the pitch is not nose-up, `dv_y'/dv_z = 0`, so `mu_z` obeys
  **`mu_z,t = w + DRAG_Z·mu_z,t+1`** — the same thing one axis over, running through the whole
  dive and the pitch-0 hold.

Only the flick has neither. Median residual **4.9e-11** and **4.6e-11** over the 1233 periodic
cells of `runs/steady/nlamsweep`; these are identities of the tick map, not fits.

Solving the first, the price of upward velocity is a **clock**:

    mu_y(t) = mu_inf − (mu_inf − mu_y(apex))·DRAG_Y^(T−t)        mu_inf = 1/(1 − DRAG_Y) = 50

`T` is the apex, where the branch switches. So `mu_y` is just the discounted count of ticks left
before you stop climbing, and `mu_y(apex)` is small — 0.80 on the reference cycle, and between
+6.3 and −10.6 across the whole corpus against a peak `mu_y` near 42. **That is what makes the
apex the right horizon:** it is the tick at which upward velocity stops being worth anything, so a
rule that stops there needs no terminal value for it. Contrast `argmax ΔTE`, whose terminal price
is the energy gradient `(v_y, v_z)/g` — it agrees with `mu` in *direction* only at the apex, where
`v_y` and `mu_y` both vanish for unrelated reasons, and even there its forward component is 4.7x
too small on the reference cycle (2.45 against `kappa` = 11.53). A short `n`
buys back what the wrong terminal price costs, which is why the implied lookahead is not the time
remaining and why it has to shorten as the climb proceeds.

### Two consecutive pitches read out the countdown

The two recursions and the closed form are three relations in `mu_y`, `mu_z` and the pitch, so two
consecutive gain-phase pitches pin the price *level* with nothing else supplied. With `r_t` the
ratio tick `t`'s pitch implies and
`K_t = (GAIN_UP·s_{t+1} + DRAG_Z(1 − 0.036 s_{t+1})·r_{t+1})/DRAG_Y`,

    mu_y,t+1 = (w − K_t) / (r_t − K_t)

which is exact (it recovers the solved costate to 3e-6 when fed the solved ratios) and recovers it
to **0.76% median, 3.2% worst** when fed the optimum's own pitches instead. Inverting the clock
turns that into ticks-to-apex:

| cycle | median miss, ticks | inside one tick | p10..p90 spread |
|---|---|---|---|
| polished optimum | −0.12 | 76% | 2.27 |
| polished family, `w = −.125 .. .250` | −0.12 .. +0.39 | 75–98% | 0.83–2.36 |
| `REPLAY_PITCHES_300` | −3.14 | 0% | 1.09 |
| the four bugs' own limit cycle | +1.51 | 4% | 37.05 |

`mu_y(apex)` is solved from the cycle, not fitted to the countdown, so the median column is a real
test and not a calibration: `REPLAY_PITCHES_300` reads its own apex three ticks off. The spread
column is the separate claim that the countdown ticks down at the right *rate*, and it is what the
policy's limit cycle fails — a median near zero on top of a spread of 37.

### The climb ends on the hold's own corner

Past the point where `mu_z/mu_y` climbs back through `GAIN_RATE = 3.519641`, the stationary `sin|p|`
goes negative and the pitch clamps at 0. That is the same corner condition, in the same direction,
that decides the pitch-0 hold's exit (`s_out` in `tools/glide_phase.py`): **hold pitch ≥ 0 rather
than pitch up while `0.036·DRAG_Z·mu_z ≥ 0.128·DRAG_Y·mu_y`.** The hold and the gain phase are the
two sides of one switch. It fires **exactly** on the polished cycle and on every member of the
polished family that returns to pitch 0 at all (6 of 8; the two most distance-averse never do), and
`+3` and `+4` ticks late on `REPLAY_PITCHES_300` and the policy's limit cycle. On the regularized
corpus 932 of 1233 cells return to pitch 0, median `−1` tick, 52% inside one tick — the corpus's
curvature price smooths the last few degrees of nose-up, so the tick the pitch "reaches 0" is soft
there in a way it is not on a polished cycle.

### Three simpler rules that do not work

All three hold one pitch and pick a horizon from the state, so all three are parameter-free. All
four numbers below come from one run over rel 214–295 of an earlier 30-pass polish of the same
cycle, where `argmax ΔTE at n = 20` scores 1.20° and `n = 1` scores 17.4°.

| rule | RMS, ° | what happens |
|---|---|---|
| hold a pitch to *its own* apex, maximize `J` there | 10.85 | the horizon it picks runs 87 ticks at the start of the climb down to 3 at the end — the long-`n` end of the ΔTE table |
| argmax over (pitch, horizon) of `ΔJ/n` | 17.39 | collapses to `n = 1` at almost every tick, so it *is* one-tick greedy: the rate is highest on the first tick and falls from there |
| argmax over (pitch, horizon) of `ΔJ − ρn`, ρ the cycle's own rate | 9.67 | picks horizons of 80 down to 1; `ΔJ` per tick runs an order of magnitude above ρ for most of the climb, so the time penalty barely bites |

The common failure is the terminal value, not the horizon: holding one pitch to the end of a long
rollout and then pricing what is left at its kinetic energy gets the price wrong in exactly the
way the section above measures. What fixes it is optimizing the whole remaining climb, not
lengthening a held-pitch one.

### The rule, and what it still has to be told

Put together: the gain phase is exactly the climb's own optimal-control problem — **from here,
choose pitches until the apex, maximizing `Σ(v_y + w·v_z)` plus `mu(apex)·v(apex)`.** No lookahead
constant; the horizon is where the down-to-forward branch switches on, which the candidate
trajectory locates itself, just as the hold's stopping rule is located by `v_z` peaking.
`myopic gain <file> bvp` solves it by forward-backward sweep at every tick of the climb and scores
the first pitch against the cycle's own:

| terminal price at the apex | RMS, ° | median, ° | max, ° |
|---|---|---|---|
| the cycle's own `mu(apex)` | **0.62** | **0.12** | 4.04 |
| `mu_y(apex)` forced to 0, `kappa` kept | 0.67 | 0.15 | 3.83 |
| both forced to 0 — "just climb as high as you can" | 2.56 | 1.22 | 11.13 |
| for comparison, `argmax ΔTE` over 20 ticks | 1.67 | 0.99 | — |

The per-tick error is about 0.1° through the body of the climb and grows at both ends: half a
degree on the ticks just off the −90° bound, and from −0.9° to −4.0° over the last ten ticks
before the apex — the same window where the ΔTE family stops disagreeing with itself and where
the implied lookahead was already meaningless.

So the fitted lookahead is gone and **one number is left: `kappa = mu_z(apex)`,** what the dive will
pay for a unit of the forward speed the climb hands it. It is not a constant. Over the corpus it
runs 2.9 to 49 and moves monotonically in both axes, falling with cycle length and rising with the
price on distance:

`kappa = mu_z(apex)`, median over the `v0` cells of each `(n, lambda)`:

| n \ λ | −2 | −1 | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|---|---|
| 150 | 16.91 | 20.67 | 24.42 | 28.11 | 31.74 | 35.39 | 39.16 | – | – | – |
| 200 | 13.06 | 17.75 | 21.98 | 26.18 | 30.23 | 34.48 | 38.55 | 42.44 | – | – |
| 250 | 6.02 | 11.79 | 16.87 | 21.66 | 26.61 | 31.43 | 36.20 | 40.86 | 45.61 | – |
| 300 | 2.89 | 5.78 | 11.70 | 17.30 | 22.70 | 28.15 | 33.41 | 38.74 | 44.24 | 49.29 |
| 350 | 2.87 | 4.28 | 7.61 | 13.69 | 19.58 | 25.29 | 31.03 | 36.79 | 42.64 | 48.40 |
| 400 | 2.87 | 4.22 | 5.74 | 11.05 | 17.33 | 23.28 | 29.26 | 35.31 | 41.17 | 47.45 |
| 450 | 2.86 | 4.21 | 5.63 | 9.71 | – | 22.15 | 28.33 | 34.52 | 40.67 | 46.90 |

It is not a free parameter either, and the `DRAG_Z` clock says what it is made of:
`kappa = DRAG_Z^M · mu_z(M)`, where `M` is the tick the pitch first goes nose-up and the corner
above is exactly what fixes *which* tick that is. On the reference cycle `M = 202`, `mu_z(202)` is
87.82, and `0.99^202 · 87.82 = 11.532` against a solved `kappa` of 11.532. So everything the climb
needs from the rest of the cycle arrives as one scalar, and that scalar is the length of the dive
discounting whatever the hold exits at — which is the useful decomposition even though it is not
yet a rule you can fly from the state alone. The exit *level* is what remains: it is set across
the flick, the only arc of the cycle with no clock on it, and the only one still unexplained.

## Flying only the bugs

`myopic policy opt` wires the four rules together with state-triggered switches, a pitch rate
limit, and eight tuned scalars. It reaches **1.375 b/s, 96% of the optimal cycle**, in 299 ticks
against 300, with every phase's energy budget within 0.11. `policy opt leak vzpeak` drops one of
those scalars for the parameter-free stopping rule and does better still, 1.386 b/s — see the
snap section.

Reassuringly, the tuner rediscovers the optimum's own switch points without being told them:
dive→snap at speed 2.40 where the optimum switches at 2.41, snap→flick at `v_y = −0.260` where the
optimum switches at −0.259. It also settled on `k ≈ 0.055` for the dive leak, which looked like a
third rediscovery at the time and was not — `k` is nearly unidentified, and the leak turned out to
be an entry correction rather than a rule.

Performance is far less sensitive to the lookahead than the pitch fit is (`NGAIN=<n> myopic policy opt`):

| n | 1 | 2 | 4 | 8 | 12 | 16 | 20 | 24 | 32 |
|---|---|---|---|---|----|----|----|----|----|
| b/s | 1.189 | 1.198 | 1.238 | 1.308 | 1.354 | 1.381 | 1.378 | **1.392** | 1.318 |
| % of optimum | 82.9 | 83.6 | 86.4 | 91.2 | 94.5 | 96.4 | 96.1 | **97.1** | 92.0 |

So anything in 16–24 is within a point of the best, and one-tick greedy costs about 13 points —
bad, but it still flies a cycle once the switch points re-tune around it.

## Porting caveat

All of this uses `sim`'s libm trig. Vanilla's `Mth.cos` is a lookup table whose index truncates to
exactly zero at −90°, which trips every `lookHorLength > 0` guard and makes the flight ballistic.
The flick wants about −88, so it does not bite here, but do not port a −90° target across.

## The dive's γ floor: what sets it, and why the peak was not real

The floor moves **monotonically** with w, and there was never a peak at w = 0. Measured as the
interior trough of γ over the middle of the dive (`myopic floor`):

| w | -.003 | -.002 | -.001 | **0** | .002 | .005 | .010 | .020 |
|---|-------|-------|-------|-------|------|------|------|------|
| γ trough | 17.37 | 17.25 | 17.01 | **16.72** | 16.32 | 15.94 | 15.60 | 14.74 |
| steady v_z there, % of ceiling | 99.92 | 99.94 | 99.98 | 99.997 | 99.99 | 99.94 | 99.85 | 99.46 |

Steeper when you are told to cover less ground, shallower when you are told to cover more, exactly
as it should be. It crosses `myopic eq`'s fastest-steady-glide angle of **16.577°** between w = 0
and w = .002; the raw 300-tick cycle troughs at 16.80°.

**The old peak was the statistic, not the cycle.** `min γ over the whole dive window` assumes the
dive settles. Near w = 0 it does — γ dips to a plateau around the middle and comes back up as the
snap approaches — but from w = -.005 down the shape inverts and γ climbs monotonically through the
dive. There is no floor there at all, and the minimum lands on the early transient instead: 12.80°
at 22% of the way in, 14.48° at 20%, 16.67° at 20%. Those three numbers were the entire left-hand
descent of the "peak". `myopic gprofile` prints the shape; `myopic floor` reports the trough and
says `(no floor)` when it is not interior.

**The rate turnpike is the wrong object.** `myopic eqrate` computes the equilibrium maximizing the
objective rate `GRAVITY*eq_vy + w*eq_vz`, which is what the cycle's objective integrates. At w = 0
that reduces to `argmax eq_vy` — the *minimum-sink* glide, pitch **-13.06°**, nose up, γ 9.15°,
speed 0.445 — and not to `argmax eq_vz` at all. `argmax eq_vz` is the other end of the same family:
it is the w → +∞ limit, the pure-distance turnpike (γ → 16.53 at w = 10).

It cannot work, for a structural reason. The turnpike argument presumes that loitering at the best
fixed point is nearly optimal. Here every steady glide sinks at 1.4 b/s or worse while the cycle
*climbs* at 1.44 b/s, so the best fixed point is about 2.9 b/s worse than the orbit. The optimum is
a strict limit cycle and there is no steady state for it to head toward.

Replacing the guessed weights with the optimum's own shadow prices does not rescue it either.
`myopic prices` recovers the costate from the stationarity condition `λ·∂f/∂p = 0` — in the (v_y,
v_z) plane that pins λ up to sign and scale — and the extraction checks out: the optimum's pitch is
the *global* argmax of `λ·f` at every dive tick, to 0.000°. The prices are λ ≈ (-0.42, +0.91):
upward velocity is worth **less than nothing** during a dive, which is why forcing λ_y > 0 gives
nonsense. But the equilibrium those prices prefer runs γ 18.3° → 21.3° *rising* through the dive
while the actual γ falls to 16.8° and levels — off by 3.7° and moving the wrong way.

**What does set it is a ceiling, not an optimum.** `argmax eq_vz` is where `d(eq_vz)/dp = 0`: the
fastest forward speed any sustained glide can hold, 3.38879 blocks/tick. At w = 0 the dive's whole
job is to arrive at the snap fast, and a long dive at constant γ *heads toward* the equilibrium for
that γ, so the best γ to hold is the one whose equilibrium is quickest. That derivation never
invokes a rate, which is why it survives while the turnpike version fails.

**It heads toward it; it does not arrive.** Ten ticks before the snap the dive is at 0.54–0.89 of
the steady speed for its own pitch, rising with dive length and never reaching 1. The dive is a
cut-off transient, not a settled glide — see "The dive has two pitches" below, which is also where
the dive-length dependence of γ* lives.

**And the floor is a badly conditioned readout of it.** eq_vz is very flat on top — γ from 16.0° to
17.1° is all within 0.05% of the ceiling, and the entire family's floors, 14.7° to 17.4°, sit above
99.4%. So γ swings by degrees at essentially no cost in what the dive is actually buying. Treat
sub-degree floor movements as unresolved; the honest statement is that the whole family pins the
dive to the top of the speed curve and the exact angle is barely determined.

Still open, but smaller:

- Why does the shape invert below w = -.005 — why does a min-distance cycle steepen through the
  dive rather than settling? Those cycles also stop matching `hold γ` as well.
- `pitch 0` is a genuine corner of the equilibrium locus, not a smooth point: for p ≥ 0 the
  equilibrium is frozen (eq_vz 1.51017 → 1.51022 over the first 0.2°) while for p < 0 it moves
  fast, so the locus has a vertex there and a whole range of w parks the turnpike at exactly 0.000.
  It is also exactly the best-glide-ratio point (γ 5.65°). The cause is the `lean_angle < 0.0`
  guard on the forward-to-up conversion in `update_fall_flying_movement`: that term is off for
  every nose-down pitch and switches on linearly in `-sin(lean_angle)` the moment the nose comes
  up, so `df/dp` is discontinuous at exactly zero. Not an open question any more, but worth
  knowing — a pitch of exactly 0 is a special point of the physics, not just a round number.

## The dive has two pitches: the ramp's average (~39°) and where it ends (~46.5°)

The dive does not converge to a pitch. It is a **monotone ramp** — roughly 31°→46° with a 3-tick
spike to ~50° at the very end — so "the dive's pitch" is ambiguous, and its average and its
endpoint are different numbers with different explanations. Conflating them is easy and was done
here once already.

`tools/dive_pitch.py <corpus>...` regenerates every table below from a corpus's `best.csv`.
Numbers here are from `runs/steady/nlamsweep` (1233 cyclic profiles, λ −2..7, n 150–450) and the
`runs/atlas/*` corpora, as of 2026-09-21 02:05 EDT.

### The average, ~39°, is flat in everything

| T (dive ticks) | 60–99 | 100–139 | 140–179 | 180–219 | 220–259 | 260–299 | 300–339 |
|---|---|---|---|---|---|---|---|
| mean pitch over the dive | 39.69 | 38.55 | 38.62 | 38.58 | 40.49 | 39.19 | 39.53 |
| terminal pitch (at T−10) | 43.57 | 46.68 | 46.81 | 46.74 | 46.53 | 46.68 | 46.51 |
| the dive's realized glide ratio | 2.275 | 2.799 | 3.075 | 3.395 | 3.586 | 3.859 | 4.057 |

Pooled ramp median **39.54**, 10–90% band 37.54..40.83; on `atlas/nsweepv0fine` (4949 profiles, 49
starting velocities) it is 39.10, band 38.42..40.42. It does not move with dive length, λ, or v0.

**A parameter-free criterion lands near it.** The dive spends height to buy kinetic energy, and the
energy is later cashed for distance at the glide ratio, so the distance a dive ultimately buys per
block of height goes like `eq_v_z · |eq_v|² / (−eq_v_y)` — glide ratio times kinetic energy.
`argmax` over pitch is **40.668°** (γ 10.21, GR 5.55), against 145 at pitch 0 and 262 at the
53.37° speed ceiling.

Treat this as suggestive, not established. It is 1.1° above the measured 39.54 and sits at the 90th
percentile of the distribution, and the criterion is too flat to be tested at that resolution —
pitches 38°–42.5° are all within 1% of its peak. It picks the right *statistic* and the right
neighborhood; it does not pin the number.

### The endpoint, ~46.5°, is not a steady-glide critical point at all

| criterion | pitch | γ |
|---|---|---|
| max glide ratio | 0.009 | 5.653 |
| glide ratio × kinetic energy | 40.668 | 10.214 |
| max forward speed `eq_v_z` | 53.366 | 16.589 |
| `\|v\|² · eq_v_z` | 55.366 | 18.238 |

Nothing sits at 46.5. Looking for it among the steady glides is the wrong search, because **the
dive is nowhere near a steady glide when it ends** — at T−10 its speed is 0.71 of the equilibrium
speed for its own pitch (0.54 at short dive lengths, 0.89 at long).

**What is invariant there is the vertical channel only.** Ranked by 10–90% spread at T−10:

| quantity | median | 10–90% band | rel. spread |
|---|---|---|---|
| pitch | 46.538 | 44.158..47.494 | 7.2% |
| `v_y / eq_v_y(pitch)` | 0.949 | 0.906..0.973 | 7.1% |
| `v_y` | −0.691 | −0.715..−0.604 | 16.0% |
| γ | 16.878 | 13.989..20.134 | 36.4% |
| speed | 2.384 | 1.807..2.880 | 45.0% |
| `v_z` | 2.284 | 1.697..2.795 | 48.1% |

The top three are one fact, not three. `v_y` is **autonomous** in the tick map — with yaw zero,
`v_y' = k(L)·(v_y − 0.08 + 0.06L)` contains no `v_z` (see `docs/elytra-tick-algebra.md`) — so `v_y`
relaxes toward `eq_v_y(pitch)` on its own and is found at 95% of it, and `eq_v_y` is monotone in
pitch. So "terminal pitch 46.5" and "terminal sink rate 0.69 b/tick" are the same statement, and
the forward channel is simply unconstrained: `v_z` ranges over 1.7–2.8 at the same pitch.

**Why the endpoint looks constant: two opposing trends cross.** Holding γ heads for the steady
glide whose flight-path angle is γ, at pitch `p_inf(γ)`. Short dives hold a steep γ*, so they aim
at a far target and fall short; long dives hold a shallow γ* and nearly arrive.

| T | 100–139 | 140–179 | 180–219 | 220–259 | 260–299 | 300–339 |
|---|---|---|---|---|---|---|
| γ* held | 19.11 | 16.64 | 14.38 | 13.41 | 12.47 | 11.79 |
| `p_inf(γ*)` aimed at | 56.31 | 53.44 | 50.10 | 48.38 | 46.51 | 44.96 |
| terminal pitch reached | 46.94 | 46.99 | 46.94 | 46.57 | 46.65 | 46.51 |
| `v/v_eq` at T−10 | 0.618 | 0.695 | 0.768 | 0.827 | 0.860 | 0.888 |

The target falls 11° while the endpoint moves 0.5°. **Caveat: this describes the flatness without
deriving it.** The two rows are not independent measurements — the "how far it got" reading is
computed from the same endpoint it is supposed to explain — so this rules out "46.5 is a steady
glide" but does not establish "46.5 is a coincidence" either. It is the best current account.

### γ* is set by dive length more than by λ

This qualifies the w-dependence tabulated above. Binned by dive length, γ* runs 20.9° (T≈95) down
to 11.8° (T≈335) with λ spanning −2..7 *inside every bin*. At fixed dive length 140–179, γ* reads
17.10° in a λ=0-only corpus, 16.76° at λ ∈ −2..2, and 15.73° at λ ∈ −2..8. So **dive length moves
γ* ~5° over 100–300 ticks; λ moves it ~1° over −2..8.** The w-dependence is real but smaller than
the length effect it was confounded with, and the README's 16.577° is the value for a ~155-tick
dive — which is what the reference 300-tick cycle happens to have.

### It generalizes off the periodic corpus

| corpus | endpoint | v0 | n | λ | profiles | terminal pitch, median (10–90%) |
|---|---|---|---|---|---:|---|
| `steady/nlamsweep` | periodic | own fixed point | 150–450 | −2..7 | 1233 | **46.54** (44.16..47.49) |
| `atlas/mapsweep` | free | (0, 0.4) | 86–350 | −2..8 | 5222 | 45.60 (40.95..47.06) |
| `atlas/nsweepv0fine` | free | 49 different | 150–350 | 0 | 4949 | 46.52 (44.41..47.34) |
| `atlas/crossproduct` | free | 16 different | 150–450 | −2..2 | 218 | 46.87 (42.40..47.68) |
| `atlas/mapfine` | free | (0, 0.4) | 120–200 | −2..6 | 6600 | 44.20 (41.70..45.74) |

Periodicity is doing no work. Across 28 starting velocities in `nsweepv0fine` with T ≥ 100 the
terminal pitch spans **46.59–46.90**, a 0.31° range — insensitive to where the flight starts.
`mapfine` reads low because its dives are only 36–129 ticks, and short dives end lower everywhere:
**~46.5 is an asymptote the ramp reaches by T ≈ 100, not a switching threshold.** `hold γ` also
transfers, at 0.44–1.32° median |error| against 0.41° on the periodic corpus.

## The exact one-tick rule, and why you cannot fly it

There *is* a myopic metric the optimum follows exactly, in every phase. Over a closed cycle the
kinetic terms cancel, so the objective is `sum_t c . v_{t+1}` with `c = (GRAVITY, w)`, and
Pontryagin says the optimal pitch maximizes

    mu_{t+1} . f(v_t, p)

over pitch at every tick — one tick, no horizon. `mu` is a price vector on velocity: how much an
extra unit of upward velocity and an extra unit of forward velocity are each worth. The score is
the price-weighted value of the velocity you are left with next tick.

**Reading `mu` off the optimum's own pitch is circular** — every schedule has, at each tick, some
direction making its pitch stationary, namely the normal to `df/dp`. That was the weakness in the
first version of this. The non-circular version is that `mu` is not free: it obeys
`mu_t = c + A_t^T mu_{t+1}` with `A_t = df/dv`, and periodicity `mu_N = mu_0` closes it, so
`(I - M) mu_0 = b` determines it with **no free parameters at all**. The N tangency conditions are
then N falsifiable predictions against zero degrees of freedom. `myopic adjoint` solves it;
`myopic consist` measures how far the solved `mu` lands from perpendicular to `df/dp`.

It is very much a property of the optimum (`myopic consist`, median degrees off perpendicular):

| cycle | climb b/s | dive | gain |
|---|---|---|---|
| polished optimum (`cyclecut polished900`) | 1.4406 | **0.019** | **0.066** |
| `REPLAY_PITCHES_300` | 1.4334 | 0.496 | 4.22 |
| the four bugs' own limit cycle | 1.3734 | 0.659 | 3.85 |
| `REPLAY_PITCHES_300` + 1° wobble | 1.3513 | 0.308 | 10.22 |
| + 3° | 1.0289 | 1.46 | 27.6 |
| + 10° | -3.70 | 24.6 | 65.9 |

The gain column roughly tracks climb rate, with one inversion (`REPLAY_PITCHES_300` at 1.4334
scores worse than the policy cycle at 1.3734). Do not oversell it: the residual tests
*stationarity* — whether the cycle is a critical point of the problem — not how good it is. A
cycle can sit at a critical point and still be beaten. The dive column does not track anything,
and that is the interesting part.

**Snap+flick sits at 26° even on the optimum** because the flick saturates near -88: it is a bang
arc, the boundary binds, and there is no interior stationarity to satisfy. That is the formal
version of "nothing to find in the flick".

**Two reasons it is not a bug.** First, `mu` is a shadow price — computing it needs the whole
future, which is the thing a myopic rule is supposed to avoid. Second, and worse, `myopic singular`
measures the curvature of the score at the optimum's own pitch:

| | dive | snap | flick | gain |
|---|---|---|---|---|
| `d^2 S/dp^2` | 1.5e-6 | 1.3e-6 | 2.8e-5 | 2.25e-5 |
| half-width within 1e-6 of the max | 1.17° | 7.35° | 7.37° | 0.43° |

The **entry** is flatter still — `5.8e-7`, half-width `2.0°`, 2.5x flatter than the dive and 38x
flatter than the gain. `mu` is least able to pin the pitch in exactly the phase that is still
unsolved.

The dive's score is **15x flatter** than the gain's. A fraction of a percent of error in `mu` throws
the argmax by ten degrees there — which is exactly what happens with `REPLAY_PITCHES_300`, whose
`mu` agrees with the tangency direction to ~1% and still shows a 10° argmax gap through the dive.

**In blocks per second**, which is the only unit that settles whether "flat" means anything.
`myopic sens` nudges one tick's pitch, lets the schedule re-converge to its own limit cycle, and
reads the change in climb. Cost of being 1° off at a single tick:

| | dive | snap | flick | gain |
|---|---|---|---|---|
| b/s lost, median | 1.7e-6 | **5.0e-3** | 3e-6 | 2.5e-5 |
| relative to the dive | 1x | **3000x** | 2x | 15x |

The dive-to-gain ratio comes out at 15x, the same as the curvature ratio, which is a fair check
that the abstract measure meant something. But the headline is the column I had not looked at:
**the snap is where the pitch matters, by three orders of magnitude.** A correlated 3° error costs
0.41 b/s across the snap (29% of the climb), 0.077 b/s across the whole dive (5%), 0.05 b/s across
the gain (3.5%). The flick drops back to dive-level insensitivity, which is the quantitative
version of "the flick's values do not matter".

**Flatness is not on its own an argument that errors are dangerous, and it is not on its own an
argument that they are safe.** It says both "hard to find the top" and "cheap to miss it". What
decides between them is whether the error is *correlated*. Independent per-tick jitter in the dive
is free; a one-signed bias is not, because the cross terms dominate:

| whole-dive shift | 1° | 3° | 5° | 10° |
|---|---|---|---|---|
| b/s lost | 0.001–0.013 | 0.050–0.077 | 0.17–0.18 | **0.57–0.86** |

A 10° correlated shift costs 40–60% of the climb rate, roughly 30x what the same amplitude costs
applied tick-independently. This is the form the `mu` hazard actually takes: a biased `mu` produces
a one-signed offset across the whole dive (the adjoint on `REPLAY_PITCHES_300` sits at a steady
+10 to +13° over ticks 40–180, not scatter), which lands in the expensive column, not the cheap one.

This is a caveat on everything above. The dive rule's 0.27° median error is worth about 5e-7 b/s —
the dive fits a clean rule partly because nearly anything reasonable fits there. The rules are
still the right description of what the optimum *does*; just do not read the dive's tight fit as
evidence that the dive is where the cycle is won.

**What the flatness does not explain.** It says the *linear* score `mu . f` is flat. The ΔTE rules
optimize a different, non-linear objective, and nothing here explains why the ΔTE family is
bimodal in the dive — that was an overclaim in an earlier version of this file and it is
withdrawn. Likewise "the gain is a regular arc, so the pitch is an argmax" was circular phrasing:
"arc" is only a label for a stretch of trajectory and implies nothing. The defensible claim is
narrower — in the gain phase the score has enough curvature that an approximate `mu` still locates
the pitch, and in the dive it does not.

**And it explains the 1-tick failure quantitatively.** One-tick greedy is argmax of
`TE(v') - TE(v)`, whose gradient in `v'` is `(v'_y + GRAVITY, v'_z)` — a price vector in its own
right, just the wrong one. Against the true `mu`:

| | dive start | dive end | gain |
|---|---|---|---|
| angle between the TE gradient and `mu` | 0.4° | 10.9° | 13–30° |

In the gain phase the TE gradient consistently *overvalues* upward velocity relative to forward,
which is precisely why one-tick greedy is nose-up of the optimum at every gain tick. Longer
lookaheads are approximating `mu` better; n ≈ 20 is where the approximation is best on average.

On the gain phase `mu` is not only solvable but writable: see "The gain phase without a
lookahead", where `mu_y` turns out to be a clock and the stationary pitch a closed form.

## The leak was an entry correction

Hand the first T ticks of *every* dive to the optimum's own pitches, let a rule take the rest, and
retune the switch speed for each cell (`myopic prefix`). `g_star` pinned at the derived 16.5773°.

| prefix | hold current | target g\* | floor clamp | leak k=.04 |
|---|---|---|---|---|
| 0 | 0.6600 | -1.5215 | 1.2392 | 1.3722 |
| 5 | 0.8635 | -1.5175 | 1.0349 | 1.3693 |
| 10 | 1.1532 | -1.4936 | 1.1532 | 1.3746 |
| 20 | 1.1672 | -1.5159 | 1.1672 | 1.4003 |
| 30 | 1.3408 | -1.5205 | 1.3408 | 1.3977 |
| 40 | 1.3343 | 1.3720 | 1.3343 | 1.3954 |
| **60** | **1.4038** | 1.3893 | 1.4038 | 1.3938 |
| 80 | 1.3983 | 1.3916 | 1.3983 | 1.3926 |
| 120 | 1.3936 | 1.3928 | 1.3938 | 1.3930 |

**The dive needs no constant.** The leak's advantage falls from 0.71 b/s to 0.06 by T = 30, and by
T = 60 the exact hold *wins*: 1.4038 against 1.3938, which is 97.9% of the optimal cycle and better
than the fully tuned leaking policy's 1.3745. Everything the leak was doing, it was doing in the
first thirty-odd ticks.

**Target the current angle, not the floor.** Steering straight at `g_star` is catastrophic until
T = 40 — it saturates at -90° trying to drag γ down in one tick, the same failure the one-shot
`γ → 16.58` rule showed — and once it does work it is *worse* than holding at every T from 60 on.
The floor is the right description of where the dive ends up; it is the wrong thing to aim at.

**The floor clamp is an entry device too.** From T = 10 onward `floor clamp` and `hold current` are
the same number to four figures: once the entry is handled, γ never goes shallower than the ceiling
angle, so the clamp never binds. It earns its keep only at T = 0 and 5 (1.2392 against 0.6600).

So the dive proper is one parameter-free rule — hold the flight-path angle — and all the difficulty
has moved into the phase before it, which needs a name and a rule of its own. That phase is the
"weirdness at the start where it likes harshly pitching down" from the very first pass over this
problem, and it is now the only part of the dive that is unexplained.
