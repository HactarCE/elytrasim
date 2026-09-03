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

That re-optimisation moves the cycle by 2.0° mean pitch and improves it 0.6% — the error bar on
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

**The floor is derivable.** The steady glide that maximises forward speed is at pitch `53.35°`,
`v_z = 3.389`, and its flight-path angle is `16.58°` (`myopic eq`). The dive is a slow approach to
the fastest steady glide.

Snapping straight to that angle instead of holding is a much weaker rule — it saturates at −90°
early and scores 34° RMS. The two rules bracket the optimum, but loosely (mean bracket 29°), and
the optimum sits ~99% of the way toward the hold end.

**Trap:** at low speed two pitch branches reach a given γ, and only the nose-down one accelerates.
A naive `argmin |γ' − γ*|` oscillates between them and never builds speed (that scored −3.0 b/s).
`bug_gamma_to` scans for the last upward crossing and then bisects.

## Phase 2, snap (~14 ticks): pitch 0

Literally zero, for about fourteen ticks. One-tick greedy independently says 0 here too.

## Phase 3, flick (~6 ticks): ramp to about −88°

Nothing to find. This is where two independently optimised cycles disagree most (11°), so the
values genuinely do not matter. It does need to be a ramp rather than a step, though a chunk of
the measured cost of stepping is probably the optimiser over-fitting the tick grid.

## Phase 4, gain (~86 ticks): argmax ΔTE over ~20 ticks, not 1

Same metric as elytrasim's `argmax_over_pitch_of_delta_energy`, with the lookahead extended.
Holding pitch constant for `n` ticks and taking the argmax of the total-energy change:

| n | 1 | 2 | 4 | 8 | 12 | 16 | 20 | 24 | 32 | 48 |
|---|---|---|---|---|----|----|----|----|----|----|
| gain-phase RMS, ° | 17.9 | 16.9 | 15.0 | 11.4 | 7.5 | 3.8 | **1.1** | 3.8 | 7.1 | 10.8 |

One tick is not merely imprecise: it pins to the −90° bound for ticks 214–230 while the optimum
recovers through −79° to −45°, and is still 40° off at tick 228.

**Is n = 20 overfit?** No. Re-optimising the cycle against `TE + w·z` gives a family of optimal
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

## Flying only the bugs

`myopic policy opt` wires the four rules together with state-triggered switches, a pitch rate
limit, and eight tuned scalars. It reaches **1.375 b/s, 96% of the optimal cycle**, in 299 ticks
against 300, with every phase's energy budget within 0.11.

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

**The rate turnpike is the wrong object.** `myopic eqrate` computes the equilibrium maximising the
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
job is to arrive at the snap fast, and a long dive at constant γ converges on the equilibrium for
that γ, so the best γ to hold is the one whose equilibrium is quickest. That derivation never
invokes a rate, which is why it survives while the turnpike version fails.

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

## The exact one-tick rule, and why you cannot fly it

There *is* a myopic metric the optimum follows exactly, in every phase. Over a closed cycle the
kinetic terms cancel, so the objective is `sum_t c . v_{t+1}` with `c = (GRAVITY, w)`, and
Pontryagin says the optimal pitch maximises

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
optimise a different, non-linear objective, and nothing here explains why the ΔTE family is
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
