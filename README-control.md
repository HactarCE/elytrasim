# The control space, and where the weird answers come from

`README-sweep.md` records that polishing the exact objective to convergence produces a schedule
no human can fly, and that the way out was to stop the optimizer early -- a pass budget, or the
`--lag1-floor` guard. That works, and it is unsatisfying for a specific reason: it makes the
answer a property of when the optimizer was interrupted rather than a property of the problem.
It also invites the reading "the optimum really is weird, we just cannot reach it", which would
mean the corpus is a record of an optimizer's stamina.

This document is the follow-up. It says what the degenerate answers actually are, shows that a
stronger optimizer finds them faster rather than avoiding them, and replaces the stopping rule
with two statements about the admissible control -- both of them stated in the profile header,
both of them leaving the polish free to run to convergence.

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

Step 1 is not optional and not a mistake to be avoided. The chattering optimum is what locates
the basin; the smooth answer inherits its quality, monotonically:

| what was projected at `mu = 1e-3` | its own J | J after projection |
|---|---|---|
| coordinate ascent from a chattering seed | 22.346 | **22.213** |
| Adam from the reference | 22.198 | 22.073 |
| coordinate ascent from the reference | 22.180 | 22.061 |
| nothing -- `mu` annealed 0.3 -> 0.001 from the reference, never chattering | - | 21.924 |
| nothing -- `mu = 1e-3` straight from the reference | - | 21.923 |

Graduated smoothing does not bridge the gap: annealing `mu` down from 0.3 lands within 0.001
blocks of where the direct run lands, 0.29 blocks below the projected answer. The chatter is
worth something as *search* even though it is worthless as an *answer*.
