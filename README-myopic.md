# Myopic metrics of the optimal climb cycle

Which one-tick-ish rules does the global optimum agree with, phase by phase?

Run things with `cargo run --release --bin myopic -- <subcommand>`; the subcommands are
documented at the top of `src/bin/myopic.rs`. Everything is measured against `sim`'s physics
with yaw pinned to zero, so the state is just `(v_y, v_z)` plus height.

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

The hold is not exact, and the leak is the whole rule: γ decays first-order from ~25° toward a
floor near 16.8° at roughly `k = 0.04–0.055` of the remaining gap per tick. An exact hold keeps
whatever γ you entered with and the cycle loses height at −3.5 b/s.

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

Reassuringly, the tuner rediscovers things it was not told: `k ≈ 0.055` for the dive leak, and the
optimum's own switch points (dive→snap at speed 2.40 where the optimum switches at 2.41;
snap→flick at `v_y = −0.260` where the optimum switches at −0.259).

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
