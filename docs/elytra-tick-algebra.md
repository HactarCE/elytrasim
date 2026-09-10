# Elytra tick algebra

Analyzed from commit `c393bbed07c9634f4f2e021f10ff3ff4288efd56` at
2026-09-10 14:24:10 EDT.

Integration correction, 2026-09-10 14:52:00 EDT: the source intentionally computes lift with
`f64::cos(lean_angle as f64)`, while look direction and pull-up use `Mth`'s `f32` trigonometry.
The first exploratory benchmark transcribed lift as `f32` cosine. The algebra is unchanged when
casts are ignored, but those original microbenchmark timings are not the production evidence.
The integrated, trig-faithful `myopic-metrics` route is differential-tested below `1e-12` in
both trig modes and makes a representative `dp` transition build 2.12× faster.

## Result

The entire `State::ticked` → `Entity::travel` →
`update_fall_flying_movement` → `Entity::mov` path reduces to one horizontal
norm, two trigonometric pairs, three scalar conditional terms, and three final
velocity expressions. In the `yaw = 0` case used by the optimizers, the yaw
trigonometry and all x-axis flight-force work disappear.

The production-oriented yaw-zero route benchmarked **2.12× faster** while building a
representative `dp` transition table. This is large enough to justify the selectable
implementation now integrated into `myopic-metrics`. This is a physics-kernel workload, not a
claim that every end-to-end command becomes 2.12× faster.

## Names

Let the input position and velocity be

```text
r = (rx, ry, rz)
v = (vx, vy, vz)
```

and define

```text
p = pitch · π / 180
q = -yaw · π / 180
s = Mth::sin(p)                pull-up sine
c = Mth::cos(p)                horizontal look gate/sign
L = cos(p as f64)²             lift factor
h = hypot(vx, vz)              original horizontal speed
e = (sin(q), cos(q))           horizontal look direction when c > 0
gy = vy - 0.08 + 0.06 L        vertical velocity after gravity/lift
```

Minecraft's ordinary pitch domain is `[-90°, 90°]`, so `c >= 0` and the
normalized horizontal look direction is `e` away from the vertical endpoints.
For unrestricted pitch, replace `e` by `sign(c) e`.

The code computes `h` before any velocity changes. That detail matters: both
the pull-up force and turning target use the old horizontal speed.

## Collapse the three force sections

Define the downward-speed conversion:

```text
D = -0.1 gy L     if gy < 0 and c != 0
    0             otherwise
```

`D` is nonnegative whenever it is active. It is added both vertically and in
the horizontal look direction.

Define the two pull-up contributions:

```text
C =  0.04 h s     if p < 0 and c != 0    (horizontal; nonpositive)
    0             otherwise

U = -0.128 h s    if p < 0 and c != 0    (vertical; nonnegative)
    0             otherwise
```

Notice that `U = -3.2 C`. The original `convert` is positive while looking up;
the horizontal code applies its negative, hence the sign of `C`.

After the dive and pull-up sections, but before turning, the velocity is

```text
vh_before_turn = (vx, vz) + e(D + C)
vy_before_drag = gy + D + U
```

Turning is a 10% interpolation toward `e h`:

```text
vh_after_turn
  = vh_before_turn + 0.1(e h - vh_before_turn)
  = 0.9(vx, vz) + e[0.9(D + C) + 0.1h]
```

Applying drag gives the complete velocity update:

```text
A   = 0.9(D + C) + 0.1h

vx' = 0.99 [0.9vx + sin(q) A]
vy' = 0.98 [gy + D + U]
vz' = 0.99 [0.9vz + cos(q) A]
```

Finally, `mov` is simply semi-implicit Euler position integration using the
new velocity:

```text
rx' = rx + vx'
ry' = ry + vy'
rz' = rz + vz'
```

Those six equations are the fully inlined tick.

### Exactly vertical look

In real arithmetic, `c = 0` at `p = ±90°`, so the three guarded horizontal
sections do not run. Gravity/lift still runs. The literal source behavior is
slightly different because the `f32` cosine near 90° is a tiny nonzero number.
The source consequently normalizes a nearly zero horizontal look vector and
runs the guarded sections. The derivation deliberately treats the
trigonometric identities as exact, as requested; callers needing bit-for-bit
Minecraft emulation should preserve the original guard and lookup-table
behavior.

For an algebraically complete formula outside the ordinary pitch interior, set

```text
k = 1 if c != 0, else 0
n = sign(c)e if k = 1, else (0, 0)
```

and use

```text
vh' = 0.99 { (1 - 0.1k)(vx, vz) + n[0.9(D + C) + 0.1kh] }
```

with `D`, `C`, and `U` gated by `k` as above.

## `yaw = 0` specialization

The optimizer call sites construct `Rot { x: pitch, y: 0.0 }`. Then `q = 0`
and `e = (0, 1)`, so the reduced tick is

```text
h   = hypot(vx, vz)
gy  = vy - 0.08 + 0.06c²
D   = if gy < 0 { -0.1gyc² } else { 0 }
C   = if p < 0 {  0.04hs } else { 0 }
U   = if p < 0 { -0.128hs } else { 0 }

vx' = 0.891vx
vy' = 0.98[gy + D + U]
vz' = 0.99[0.9vz + 0.9(D + C) + 0.1h]
r'  = r + v'
```

The x velocity only experiences turning/drag. Pitch-dependent forces affect z
and y, and the original x/z mixture enters only through `h`.

The descending vertical case can also be written without `D`:

```text
vy' = 0.98[(1 - 0.1c²)gy + U]    when gy < 0
vy' = 0.98[gy + U]               otherwise
```

## Direct Rust form

This is the candidate shape used by the integrated route. It retains the source's split
between `Mth` look/pull-up trigonometry and double-precision lift, including the vertical-look
gate and the rounded direction at the pitch endpoints.

```rust
const H_DRAG: f64 = 0.99_f32 as f64;
const V_DRAG: f64 = 0.98_f32 as f64;

fn tick_velocity_yaw_zero(v: Vec3, pitch: f32) -> Vec3 {
    let p = pitch * (std::f64::consts::PI / 180.0) as f32;
    let look_cos = Mth::cos(p);
    let h = if v.x == 0.0 { v.z.abs() } else { v.x.hypot(v.z) };
    let lift = (p as f64).cos().powi(2);
    let gravity_y = v.y - 0.08 + 0.06 * lift;

    if !(look_cos.abs() > 0.0) {
        return Vec3::new(v.x * H_DRAG, gravity_y * V_DRAG, v.z * H_DRAG);
    }
    let look_z = if look_cos.is_sign_negative() { -1.0 } else { 1.0 };

    let dive = if gravity_y < 0.0 {
        -0.1 * gravity_y * lift
    } else {
        0.0
    };
    let climb = if p < 0.0 { h * -Mth::sin(p) as f64 * 0.04 } else { 0.0 };

    Vec3::new(
        0.9 * v.x * H_DRAG,
        (gravity_y + dive + climb * 3.2) * V_DRAG,
        (0.9 * v.z + look_z * (0.9 * (dive - climb) + 0.1 * h)) * H_DRAG,
    )
}
```

For arbitrary yaw, compute `(sin_q, cos_q) = q.sin_cos()`, compute
`along_look = 0.9 * (dive + climb_h) + 0.1 * h`, and use

```rust
x: (0.9 * v.x + sin_q * along_look) * horizontal_drag,
y: (gravity_y + dive + climb_y) * vertical_drag,
z: (0.9 * v.z + cos_q * along_look) * horizontal_drag,
```

## Validation

The original exploratory validation checked the cast-agnostic derivation. The integrated Rust
test instead compares the actual reference function with the routed yaw-zero function and
preserves the source's mixed trigonometry. It checks:

- 1,000,000 deterministic random inputs with each velocity component in
  `[-5, 5]`, pitch in `[-89.9°, 89.9°]`, and yaw in `[-180°, 180°]`;
- explicit zero-velocity, positive/negative velocity, near-zero-pitch, and
  near-vertical-pitch cases; and
- a state-dependent 10,000-tick yaw-zero trajectory with changing pitch.

Maximum componentwise absolute errors were:

| Check | Bound |
|---|---:|
| 100,000 random yaw-zero cases under libm | `< 1e-12` |
| 100,000 random yaw-zero cases under `Mth` LUT | `< 1e-12` |
| Explicit `-90`, `-89.999`, signed zero, `89.999`, and `90` pitches | `< 1e-12` |

The optimized route handles the endpoint sign and zero gate explicitly, so it does not rely on
the mathematically idealized `cos(±90°) = 0` when reproducing source behavior.

`cargo test --all-targets` also passes on the unmodified source tree (there are
currently zero repository tests and 33 existing compiler warnings).

## Benchmark

Environment: Apple arm64, Rust 1.98.0 (`aarch64-apple-darwin`, LLVM 22.1.8), release build,
Minecraft `Mth` LUT mode. A `dp` transition-table build over 66,049 velocity states and 341
pitch controls measured:

| Route | Transition build |
|---|---:|
| Reference | 0.121 s |
| Algebraic yaw zero | 0.057 s |
| Speedup | **2.12×** |

The resulting DP value table and selected schedule were byte-identical. The main wins are
eliminating look-vector construction and normalization, avoiding yaw trigonometry, reusing
pitch trig, collapsing the mutation passes, and replacing `hypot(0, z)` by `abs(z)` in the
optimizer's `(v_y, v_z)` plane.

## If this becomes production code

The yaw-zero specialization is the compelling version. A reasonable production
shape would keep a general function for arbitrary rotations and route the
optimizer directly to a small yaw-zero function. Before merging it, add the
random differential test as a real test target and benchmark an actual grid or
pitch-search workload; that will reveal the application-level speedup.

There is another likely gain beyond the benchmark: pitch searches repeatedly
visit the same discrete pitch values, so `(sin(p), cos(p), cos²(p))` can be
precomputed per candidate pitch. That optimization is separate from the
algebra here and was not included in the reported 2.45×.
