# The one-tick energy field: its troughs, ridges and creases

`G(v)` is the most total energy, in blocks, that any pitch makes in one tick from velocity
`v = (vy, vz)`. It's the background of elytra-vario's chart and of `tools/plot_field_replay.py`.
This is its geometry over the mod's chart, vy [−30, 40] by vz [−10, 60] **blocks/second**.

    python3 tools/plot_field_troughs.py --ridges     # runs/atlas/fig/field_troughs.png
    python3 tools/plot_field_gradient.py             # runs/atlas/fig/field_gradient.png

The computation is `src/bin/field.rs` (about 1 s for the chart at 1050 samples); the scripts call
it through `tools/field_data.py`, which caches each window under `runs/field/`. That cache is
keyed on the arguments, not the code: **delete `runs/field/` after changing `field.rs`**, or the
plots keep showing the old output. `tools/field_geometry.py` is the same math in numpy, for
one-off probes; `python3 tools/field_data.py --check` compares the two (bit-identical on
2026-10-02).

`--color=pitch` colors by the best pitch instead of the gain it makes (`field_pitch*.png` below
are the same windows with it). Hatching marks the three corners the best pitch sits on: `///`
−89, `\\\` +89, `---` level. Other windows take `--window=vz_lo,vz_hi,vy_lo,vy_hi` in
blocks/second. `--kinds` draws each kind of trough in its own style, `--pitch=5` contours the best
pitch where it's free, and `--stretch` gives each axis its own scale and resolution, for features
too thin to see at equal aspect. The other figures, all into `runs/atlas/fig/` (gitignored):

    tools/plot_field_troughs.py runs/atlas/fig/field_troughs_zero.png --ridges --kinds \
        --window=-10,20,-10,10 --samples=1500
    tools/plot_field_troughs.py runs/atlas/fig/field_troughs_junction.png --ridges --kinds \
        --stretch --pitch=5 --window=10,22,-0.5,0.8 --samples=1200,1000 --legend=lower_left
    tools/plot_field_troughs.py runs/atlas/fig/field_troughs_wide.png --ridges --kinds \
        --window=-40,90,-60,120 --samples=1600 --legend=lower_right
    tools/plot_field_troughs.py runs/atlas/fig/field_pitch.png --ridges --color=pitch
    tools/plot_field_troughs.py runs/atlas/fig/field_pitch_zero.png --ridges --kinds --color=pitch \
        --window=-10,20,-10,10 --samples=1500
    tools/plot_field_troughs.py runs/atlas/fig/field_pitch_wide.png --ridges --kinds --color=pitch \
        --window=-40,90,-60,120 --samples=1600 --legend=lower_right

Physics is the f64 `libm` transcription that `plot_field_replay.step` uses, not `mth_lut`. Numbers
are as of 2026-10-02 02:45 EDT, on top of commit 4cb73c4.

## Where the best pitch sits

69% of the chart has the best pitch *stuck* on a corner of `g(pitch)` rather than at a smooth
interior max: level (pitch 0) on 29%, the −89 clamp (nose up) on 18%, and +89 (nose down) on 22%.
Level is the corner described under "the corner at 0" in `README-myopic.md`. It's flat to second
order on the nose-down side, which matters for anyone refining an argmax: golden section stalls
about 2e-6° short of it, where `g` changes by ~1e-16. `golden_max` (in `field.rs` and
`field_geometry.py`) snaps onto it.

## Every trough is a crease

Defined as a local minimum along the direction G curves most, the smooth part of G has **no
troughs at all** on this chart. Only ridges are smooth. Every trough is a line where G has a kink.
A crease is a trough when G rises on both sides along its normal (a V), and a ridge when it falls
on both sides (a Λ). The third case is a crease with no extremum: G rises (or falls) on *both*
sides, at a different rate on each, like a road that steepens without topping out. The slope
jumps but keeps its sign, so G has a kink there and no min or max. Most crease length is that
third kind (dotted on the figures).

**The main trough, vy ≈ 0.4–0.66 b/s, runs the whole width.** It comes from the
descent-to-forward conversion, which only runs when vy after gravity is negative. That threshold
puts a *convex* kink in `g(pitch)`: just past the pitch that turns conversion on, gain jumps up.
The kink splits `g(pitch)` into two humps, level and a nose-down glide, and the trough is where
they swap. In two pieces:

- **vz < 15.99 b/s: the conversion threshold itself, at exactly vy = 0.4 b/s.** That's where vy
  after gravity at level (`vy − 0.02` b/t) crosses 0. Best pitch is level on both sides, and G
  kinks because conversion switches on underneath it. It's a V only for vz 11.47–15.99; further
  left the same crease has no extremum.
- **vz > 15.99 b/s: a tie between level and a nose-down glide.** Below the tie the best pitch is
  an interior glide nosed down just far enough that conversion runs; above it, level wins. At the
  tie the argmax jumps:

  | vz (b/s) | tie at vy (b/s) | glide pitch below it | vy after gravity at that pitch (b/s) |
  |---|---|---|---|
  | 17 | 0.401 | 11.2° | −0.044 |
  | 20 | 0.414 | 20.9° | −0.138 |
  | 30 | 0.486 | 33.1° | −0.271 |
  | 45 | 0.585 | 40.5° | −0.320 |
  | 60 | 0.661 | 44.5° | −0.329 |

  So it isn't the conversion threshold at the best pitch. The glide side converts with margin
  and the level side doesn't convert at all. The level hump is born at vy = 0.4 b/s (below that,
  level already converts, so nosing down only helps). The tie sits just above that.

  **The junction, vz = 15.99 b/s** (`field_troughs_junction.png`, stretched axes). The free-pitch
  glide region's edge comes up to the trough there in a cusp. Right of the cusp the glide hump
  exists just below vy = 0.4 and beats level; left of it, level wins on both sides. Near the
  cusp the tie is within ~1e-4 b/s of the 0.4 line (at vz 16.2, level wins already at 0.401) and
  the pitch jump shrinks toward 0 (5° at vz 16.2, 11° at 17).

**The other trough, lower left (vz −10 to −2, vy −3 to −0.7 b/s), is a tie between the two
clamps,** +89 and −89, in backward flight.

The tie between level and +89 (the hatched region's diagonal edge), the vz = 0 line (`g` uses
`|vz|`), and a tie at vy 11–16 b/s on the right are all creases with no extremum.

## Past the chart

`field_troughs_wide.png` is vz [−40, 90] by vy [−60, 120] b/s. Its top is past the chart's +30
because of the loss region over vz = 0. That's a wedge, not a band: G < 0 from vy = 59.3 b/s at
vz = 0 (level there) to at least 300, and its floor rises to 70 at vz = 5 and 80 at vz = 10.
Rising this fast, one tick's drag costs more than any pitch can win back.

The only **smooth trough** in that window is on the wedge's left flank, from about (−16.8, 39.1) to (0, 69.7).
It's backward flight with an interior nose-up best pitch (−60° at the bottom end, −12° at the
top), and no tie or kink. It starts on the −89 region's edge, where the Hessian jumps (see the
ridge break below), and stops on the vz = 0 kink.

## Ridges, and the break at (23, 30)

The long diagonal ridge from (11, 0) to (60, −19) b/s lies in the level region, where G is
`g(v, 0)`. That's a quadratic in v with a constant Hessian (largest eigenvalue −3.10 per (b/t)²),
so the ridge is an exact straight line.

The upper ridge **breaks where it meets the −89 clamp**, ending at about (23.5, 30.3) and
restarting at about (23.5, 27.5) inside the clamp region. G stays C¹ across that boundary
because the best pitch reaches −89 continuously. But its Hessian jumps: the envelope correction
`−g_vp g_vpᵀ / g_pp` applies while the pitch is free and vanishes once it's clamped. Along
vy = 29 b/s, ∂²G/∂vz² goes from +0.088 at pitch −84.7° to −0.918 at −89 (per (b/t)²). A ridge is
defined through the Hessian, so the ridge set breaks too. The zigzag in the first version of the
figure was a blurred derivative drawing a connector across that jump. It isn't in G.

## Reading the derivative panels

`field_gradient.png`'s features in the + + quadrant come from three regimes of the best pitch.
At level (below the line vy = 0.287 vz − 1.23 b/s) and at the −89 clamp (upper right), G is an
exact quadratic in v, so its gradient is linear and its Hessian constant there. Between them,
the best pitch is a free nose-up angle.

That boundary line is where nosing up starts to pay. A little nose-up moves `0.04 vz` per unit of
sin(pitch) from vz into vy at 3.2×; the turning term gives back a tenth, so vz loses 0.9× of it.
That pays when `3.2 (d_y² (vy − 0.02) + g d_y) > 0.9 d_z² vz` (b/t, `d` the drags). At level both
sides are linear in v, so the line is exactly straight.

- **∂G/∂vz's black line is vy = 0.5976 vz − 0.033 b/s, a climb angle of 30.9°.** Above it, more
  forward speed is worth having; below it, it's a cost. At −89, a unit of vz feeds 12.8% of itself
  into vy each tick (the 4% climb conversion, at 3.2×). That's worth more the faster you're
  already climbing. Against it, the vz you keep loses its drag and a net 3.6% bleed. The closed
  form gives the line above, and the grid's zero matches it with 0.000 b/s rms. It runs nearly
  through the origin because the constant (gravity) terms nearly cancel. In the free region it
  continues bent (≈ vy = 0.56 vz + 2.2).
- **∂G/∂vy's black ∪ is centered on (0, 29.9).** At vz = 0, G is the level quadratic and
  ∂G/∂vy = 0 at vy = (d_y g − 0.02 d_y²)/(1 − d_y²) = **29.895 b/s**. Below that, extra vertical
  speed buys more height in the tick (d_y per unit) than drag takes from it ((1 − d_y²) vy / g);
  above, drag wins. G along vz = 0 is a downward parabola with that vertex, whose roots are
  0.47 and **59.32 b/s**: the second root is the floor of the loss wedge over vz = 0. With any
  |vz|, the climb conversion (driven by |vz|, so backward too) feeds vy, and the balance needs
  more vy. So the zero curve rises on both sides, to 43.5 b/s at vz = ±12.9.
- **|∇G|'s local min at (0, 29.9) is a true zero of the gradient,** a degenerate critical
  point. ∂G/∂vy = 0 there by the balance above. ∂G/∂vz = 0 because at vz = 0 every vz term is
  second order: drag, the turning loss in backward flight, and the climb conversion's gain (the
  best nose-up pitch grows ∝ vz, so its payoff is ∝ vz²). Along vy it's a max; along vz it goes
  like vz·|vz|: G rises going forward and falls going backward, flat at 0.
- **Laplacian:** constant in the level and −89 regions. In the free region it is
  `Δg (at the fixed best pitch) + |∇ᵥ g_p|² / |g_pp|`. The first term is drag: negative and
  nearly constant (−0.0025 to −0.0035 per (b/s)²). The second is what re-optimizing the pitch
  adds: never negative, and large where g(pitch) is flat at its max so the best pitch swings
  with v. **The two black lines are where they cancel.** The lower one (from (5, 0.4) to
  (36, 16), close to vy = 0.50 vz − 2.1) is where the term decays away from the level edge,
  where it jumps from 0 to +0.17. The upper one (from (0, 8) to (22, 34)) is where it grows
  back with vy, because the climb conversion's payoff scales with vy. At vz = 30 the crossing is
  vy ≈ 13.0; at vz = 10 it's ≈ 21.7.
- **Why the Hessian panels look the same:** that pitch term is rank one: `g_vp g_vpᵀ / |g_pp|`
  adds curvature only along the direction in which v moves the best pitch. So H is drag, which
  is negative definite, plus one positive direction. Where that term is big, the largest
  eigenvalue is roughly the Laplacian, and det H and the Laplacian change sign close together.

## Method, and what it rules out

- **No grid derivatives.** grad G = `g_v` at the best pitch (envelope theorem). The Hessian is
  `g_vv − g_vp g_vpᵀ / g_pp` where the pitch is free and `g_vv` where it's stuck, from finite
  differences of the closed-form `g` at steps far below any feature (`field.rs`). As a
  check, the grid curl of that gradient is 3e-7 of the local |H| at the median and 3e-4 at the
  99th percentile, away from creases.
- **Smooth extrema:** candidates are the grid's zero crossings of `det[grad G, H grad G]`, which
  doesn't depend on the sign of an eigenvector. Each one is then moved onto the true curve by
  maximizing or minimizing G along the major eigenvector within two samples. Candidates with no
  interior extremum are dropped. That's what removes false zeros where H jumps.
- **Creases are found on grid edges, not by contouring.** An edge whose ends are in different
  regimes (argmax jumps, conversion flips, vz changes sign) gets bisected to its exact crease
  point. A tie only counts if both maxima are genuine local maxima of `g(pitch)` and together on
  top of G. Contouring a tie's signed gap fails here because the losing hump often exists only in
  a band a sample or two wide. The normal is the jump in grad G, and the class comes from G's
  one-sided slopes 1e-5 b/t either side.
- **Branches are followed inside their hump.** The conversion kink is convex, so it's the wall
  between two humps of `g(pitch)`, at `cos² p = (g − vy) / (0.75 g)`. A search bracket that
  crosses it climbs the next hump's flank and loses the branch it was following, which hid the
  tie for vz 16.0–16.4 until brackets stopped at the wall (`bracket` in `field.rs`). The same
  fact bounds the tie search: two humps closer than 3° need a conversion kink between them, so a
  small pitch change across an edge is only checked for a tie where conversion also differs.
- **An earlier version blurred G by 0.15 b/s before differentiating.** It drew the vy ≈ 0.5
  trough as one curve but couldn't show it was two, put a connector across the ridge break, and
  invented extrema at the chart's edges.
