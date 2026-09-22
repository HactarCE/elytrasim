# Forward speed while rising

Analyzed from commit `a961ea99f2972c422c5dd0ab1b0367f256135bef` at 2026-09-22 01:20:00 EDT.
Every table here is `myopic rising <part> --trig mth_lut`; the closed forms use exact `0.99`
and `0.98`, so they sit a uniform `9.6e-7` below the `f32`-drag sim.

Notation follows `docs/elytra-tick-algebra.md`: `L = cos^2 p` is the lift factor, `p` is pitch in
degrees with **positive nose-down**, and `h = hypot(v_x, v_z)` is horizontal speed. `h` is the
right quantity, not `v_z`: turning rotates horizontal velocity toward the yaw at 10% a tick, so
`v_z` can rise while `h` falls. Unpowered flight only — a firework rocket adds velocity along
the look vector unconditionally and none of this constrains it.

## Result

**While `v_y >= 0.08` no pitch and no yaw can raise horizontal speed, and with the yaw already
along your velocity no pitch can lower it either: `h' = 0.99 h`, exactly.** Pitch is not a weak
lever on forward speed there, it is not a lever at all. The only thing it still controls is lift,
i.e. how long the rise lasts. (Yaw remains a lever downwards: turning drags `h` toward a heading
you are not already on, and that always costs.)

That is the sharp form of "when moving up, you can't accelerate forwards", and of luna's
"when moving up, looking down doesn't give z-vel" (`TODO.md`). Both hold, with `v_y >= 0.08` as
the exact boundary — 0.08 is gravity, and the reason is one line of the tick map.

The interesting part is that the boundary is nowhere near `v_y = 0`. Gaining forward speed needs
a *descent*, and a steep one:

    some pitch raises h   <=>   h < 8.91 (0.02 - v_y)          for v_y <= -0.04

At cruise that reads as a glide angle: you gain forward speed only while sinking steeper than
about **6.4 degrees** below horizontal. Level flight is deceleration; a shallow descent is
deceleration.

## Why

From the yaw-zero tick map, with the yaw pointed along the horizontal velocity (any other yaw is
strictly worse, because turning then drags `h` toward a direction it is not already going):

    h' = 0.99 [h + 0.9 (D + C)]
    D  = -0.1 L (v_y - 0.08 + 0.06 L)     if that is positive, else 0     (down -> forward)
    C  = -0.04 h sin(-p)                  if p < 0, else 0                (forward -> up)

`D` is the only term that can add speed, and it is gated on `v_y - 0.08 + 0.06 L < 0` — the
vertical velocity *after* gravity and lift. `L <= 1`, so at `v_y >= 0.08` the gate is shut for
every pitch: `D = 0`. `C <= 0` always. So `h' <= 0.99 h`, with equality for every `p >= 0`.

Three corollaries, all measured in `myopic rising pitch`:

- Equality holds for the *whole* nose-down range, not just for one pitch. Diving at 5 degrees
  and diving at 89 degrees cost the same 1% of horizontal speed.
- Nose-up is strictly worse and never optimal *anywhere*, not only while rising: for any nose-up
  pitch, the nose-down pitch with the same `L` has the same `D` and pays no `C`. So the
  `h`-maximizing pitch is always nose-down or zero.
- It survives in three dimensions. Over 4e6 random `(v_x, v_y, v_z, pitch, yaw)` draws with
  `v_y >= 0.08`, the worst `h'/h` is exactly `0.99f32`.

There is one vanilla quirk: at pitch `-90`, `Mth.cos` returns exactly `0`, so `look_hor_length`
is zero and all three guarded sections — conversion, pull-up, turning — are skipped. Looking
straight up does not pull you up; it turns the elytra off. At pitch `+90` `Mth.cos` is
`1.2246e-16`, not zero, so the guards pass and turning still runs. For `h` the two ends are
identical once the yaw is aligned. See the traps section below.

## The frontier

Maximizing `D` over `L` in `[0, 1]` and solving `h' = h` gives the largest `h` at which some
pitch still raises `h`, together with the pitch that does it:

| region | frontier `h` | argmax pitch |
|---|---|---|
| `v_y >= 0.08` | `0` | — (nothing works) |
| `-0.04 <= v_y <= 0.08` | `3712.5 (0.008 - 0.1 v_y)^2` | `arccos(sqrt(L*))`, `L* = (0.008 - 0.1 v_y)/0.012` |
| `v_y <= -0.04` | `8.91 (0.02 - v_y)` | `0` |

Bisected against the sim (`myopic rising frontier`), every row agrees to `9.6e-7` relative, which
is the `f32` drag offset:

| `v_y` | frontier `h` | argmax pitch |
|---:|---:|---:|
| −1.000 | 9.0882 | 0.000 |
| −0.400 | 3.7422 | 0.000 |
| −0.14949 | 1.51016 | 0.000 |
| −0.040 | 0.53460 | 0.000 |
| 0.000 | 0.23760 | 35.264 |
| 0.040 | 0.05940 | 54.736 |
| 0.079 | 0.000037 | 84.762 |
| 0.080 | 0 | — |

Reading the three branches:

- **The bottom branch is already in the repo.** `h < 8.91 (0.02 - v_y)` is exactly
  `v_z >= 0.1782 - 8.91 v_y` negated — the `vz_peaked` line that ends the pitch-0 hold
  (`README-myopic.md`, phase 2). That rule is this frontier restricted to `v_y < -0.04`, which
  is why pitch 0 is its argmax. The frontier passes through the pitch-0 steady glide
  `(-0.14949, 1.51017)` by construction, and that glide's flight-path angle, `5.653`, is the
  frontier angle at that speed.
- **The middle branch tops out at `h = 0.5346`.** Between `v_y = -0.04` and `+0.08` the argmax
  pitch leaves 0 and walks to 90, and the frontier speed collapses to zero. At any speed a
  speedrun cares about the working rule is the line; the middle branch matters only down in the
  climb cycle's own entry, which starts at `h = 0.20`.
- **`v_y >= 0.08` is the wall.** No amount of nose-down buys anything, because the conversion is
  gated on beating gravity, not on looking down.

Two convenient equivalent forms of the working (bottom) branch:

    h + 8.91 v_y < 0.1782            gaining forward speed
    tan(gamma) > (1 - 0.1782/h)/8.91 the same, as a glide angle; -> 6.406 deg as h grows

## How far nose-up you can go and still accelerate

The claim also has a pitch-side version, and it is a curve rather than a threshold. `C` is linear
in the nose-up sine and `D` is quadratic in it, so for each `(v_y, h)` there is a steepest nose-up
pitch that still leaves `h' > h` (`myopic rising pitch`; blank means no pitch does):

| `v_y` \ `h` | 0.5 | 1.0 | 1.5 | 2.0 | 2.5 | 3.0 | 3.389 |
|---:|---:|---:|---:|---:|---:|---:|---:|
| −0.1 | −16.96 | −1.11 | — | — | — | — | — |
| −0.2 | −33.37 | −14.18 | −4.83 | — | — | — | — |
| −0.4 | −47.81 | −31.17 | −20.25 | −12.83 | −7.65 | −3.91 | −1.66 |
| −0.6 | −54.78 | −40.57 | −30.44 | −22.81 | −16.95 | −12.39 | −9.53 |
| −1.0 | −62.14 | −50.78 | −42.33 | −35.52 | −29.86 | −25.09 | −21.88 |
| −2.0 | −69.94 | −61.70 | −55.46 | −50.27 | −45.79 | −41.82 | −39.02 |

So "pitching up costs forward speed" is true as a derivative statement everywhere, but the budget
is real: in a committed dive at `v_y = -1.0`, `h = 2.0` you can hold 35 degrees nose-up and still
be speeding up. It is the *shallow* descents where any nose-up at all is immediately a loss.

## Spending an overshoot

If you convert too much speed into height and arrive at the top with `v_y > 0` and little `v_z`,
the question "what is the fastest way back to `v_y < 0`" and the question "what keeps the most
forward speed" have the **same** answer, and for a simple reason: horizontal speed is not on the
table. `h' = 0.99 h` every tick of the rise regardless of pitch, so

    h at the end of the rise = h_0 * 0.99^T

and the only thing to minimize is `T`. `T` is minimized by zero lift — pitch `±90` — because
`v_y' = 0.98 (v_y - 0.08 + 0.06 L)` is increasing in `L` and nothing else in the state depends on
`L` while the gate is shut. There is no tradeoff to tune inside the rise; the whole decision is
one scalar, how much lift to keep, and it trades height against time at a fixed rate.

With zero lift the rise is exactly solvable: `v_y(k) = -3.92 + (v_y0 + 3.92) 0.98^k`, so

    T = ceil( ln((v_y0 + 3.92)/4) / 0.020203 )       ticks until v_y < 0.08

which matches the sim on every row. The cost of keeping lift instead (`myopic rising clock`):

| `v_y0` | `T`, no lift | `0.99^T` | `dy` | `T`, full lift | `0.99^T` | `dy` |
|---:|---:|---:|---:|---:|---:|---:|
| 0.167467 | 2 | 0.9801 | 0.091 | 4 | 0.9606 | 0.445 |
| 0.20 | 2 | 0.9801 | 0.154 | 6 | 0.9415 | 0.721 |
| 0.30 | 3 | 0.9703 | 0.400 | 10 | 0.9044 | 1.673 |
| 0.40 | 4 | 0.9606 | 0.753 | 14 | 0.8687 | 2.939 |
| 0.80 | 9 | 0.9135 | 3.171 | 26 | 0.7700 | 10.158 |

A tick of rising costs `1 - 0.99^2 = 1.99%` of horizontal kinetic energy, which in the repo's
block units is **`0.1244 h^2` blocks per tick** — quadratic in speed. That is what makes the
overshoot cheap for a climber and expensive for a speedrunner: at the optimal cycle's entry speed
`h = 0.2` a rising tick costs 0.005 blocks, and at the fastest steady glide `h = 3.389` it costs
1.43 blocks.

So the two regimes fall out of one number. Comparing total energy at the end of the rise
(`myopic rising tradeoff`), the crossover speed `h*` above which dumping the lift wins:

| `v_y0` | 0.15 | 0.20 | 0.30 | 0.40 | 0.50 | 0.80 | 1.20 |
|---|---:|---:|---:|---:|---:|---:|---:|
| `h*` | 0.92 | 1.12 | 1.29 | 1.44 | 1.63 | 2.16 | 2.70 |

Below `h*`, hold the lift and bank the height; above it, dump it. For scale the pitch-0 steady
glide is `h = 1.510` and the fastest steady glide is `h = 3.389`. This is why `REPLAY_PITCHES_300`
opens its entry at 2.2 degrees — nearly full lift — from `v0 = (0.167467, 0.200887)`: at
`h = 0.2` the rise is nearly free and the height is worth having. A speedrunner at `h = 3` is on
the other side of every row in that table.

Two caveats on the recovery. First, the comparison above stops at the end of the rise; it does
not price what you then do with the height, and the frontier says the height only comes back as
speed once you are descending past `h/8.91`. Second, altitude is not in the state — `myopic` and
`dp` both drop position from the `(v_y, v_z)` plane — so "dump the lift" is advice about energy,
not about the floor.

## Two `±90` traps for the simulator

Both are trig-mode dependent, and only `mth_lut` is Minecraft. One tick from `v = (0, 0.30, 2.00)`
(`myopic rising down`):

| pitch | `Mth::cos`, `mth_lut` | `v_z'/v_z` | `v_y'` | `Mth::cos`, `libm` | `v_z'/v_z` | `v_y'` |
|---|---:|---:|---:|---:|---:|---:|
| 89.999 | 9.587e-5 | 0.990 | 0.2156 | 1.748e-5 | 0.990 | 0.2156 |
| 90 | 1.2246e-16 | 0.990 | 0.2156 | −4.371e-8 | **0.792** | 0.2156 |
| −89.999 | **0** | 0.990 | 0.2156 | 1.748e-5 | 0.954 | **0.4665** |
| −90 | **0** | 0.990 | 0.2156 | −4.371e-8 | 0.828 | 0.4665 |

Two separate things go wrong, at opposite ends, in opposite modes.

**`libm`, pitch `+90`.** `cos(90 deg as f32)` comes out *negative* (`-4.371e-8`), so the
normalized look direction flips and turning pulls horizontal velocity backwards:
`0.99 (0.9 v_z - 0.1 h) = 0.792 v_z`, a 20.8% loss in one tick instead of 1%. Vanilla does not do
this — its LUT index lands on `SIN[32768] = 1.2246e-16`, tiny but positive — so a study that
sweeps pitch to `+90` under the default trig mode is measuring a cliff Minecraft does not have.

**`mth_lut`, pitch near `-90`.** `Mth.cos` is exactly `0` for the whole `[-90, -89.9945]` LUT
bucket, so `look_hor_length` is zero and *all three* guarded sections — conversion, pull-up and
turning — are skipped. In vanilla, looking straight up does not pull you up: it switches the
elytra off and leaves gravity and drag. Under `libm` that bucket instead has the pull-up at full
strength (`v_y` goes 0.30 → 0.4665 in the table above), which is the opposite behavior.

So the two modes disagree about which end of the pitch range is degenerate, and about what the
degeneracy is. For the frontier this changes nothing — every nose-down route still gives
`h' = 0.99 h` while rising — but it does mean `±90` is never a safe place to
read the physics off, and `Jitter`'s note in `opt.rs` about the singular arc ("at 90 degrees
`cos(lean_angle)` is zero, so ... `look_hor_length` vanishes") describes the `mth_lut` `-90`
case, not `+90` and not `libm` at all.

## Reproducing

    cargo run --release --bin myopic -- rising all --trig mth_lut

`frontier` bisects the frontier against the sim, `pitch` runs the pitch-invariance and 3D checks
and the nose-up table, `clock` and `tradeoff` do the overshoot, `down` does the `±90` traps.
