# Boosters on EMC, and the climb out of them

Measured on the elytramc.net ("EMC") server on 2026-09-27, from one logged session on the
course *goldrush* (28 passes through one booster) and two HUD screenshots on *Monster*. Physics
throughout is `sim` with `mth_lut` trig, yaw pinned, `|pitch| <= 89`.

## How an EMC booster works

A booster is a server-side velocity set, nothing else:

- **It sends `ClientboundSetEntityMotionPacket` for the player, with a fixed speed per booster.**
  On goldrush all 84 booster packets have `|v|` between 1.64980 and 1.65002 b/t (33.00 b/s), so
  the speed is a setting of the booster, not a function of how you arrive.
- **The direction is your look direction as the server last saw it, 3–4 ticks earlier.** Matching
  each packet's direction against the logged look at `k` ticks before it arrived, the median
  angle is 0 at every lag from 1 to 4, but the p90 falls from 33° at lag 0 to 3.4° at lag 4 and
  rises again past it. The lag is round-trip ping plus a server tick; the HUD showed 108 and 196
  ms of latency during the two sessions. **Turning in the last ~200 ms before the boost does not
  count.**
- **It re-sends every tick you are inside the booster volume**, 2–5 packets per pass on goldrush.
  Normal elytra physics runs between packets, and the last packet sets the launch.
- So a pass through a booster is: `v := |v_b| * look(t - lag)`, repeated while inside. The launch
  angle is a free choice, made by where you look ~4 ticks before you leave the booster.

What is not a booster:

- A course reset is a `ClientboundPlayerPositionPacket` to spawn.
- Eight other motion packets (0.15–0.91 b/t) appeared in the goldrush session. All of them point
  exactly along a world axis (yaw ±90 or 180, or straight down), and some land mid-boost. They
  are unexplained; collision or knockback handling is the guess.

**The sim matches EMC's flight physics.** Predicting each logged tick's velocity from the one
before, with that tick's logged pitch, misses by 4e-6 b/t median over 229 gliding ticks. The large
misses are all wall hits. The pitch on a log row is the pitch that tick's physics used.

`|v|` for *Monster*'s booster is inferred, not logged: back-solving the HUD's post-boost velocity
(+7.64 b/s up, 58.30 b/s horizontal) through one tick of flight gives 2.99 b/t, so **3.0 b/t
(60 b/s)**, launched about 5.5° up. More ticks between boost and screenshot give 3.04–3.12.

## The best approach angle

"Approach angle" is the launch direction above horizontal, i.e. minus the look pitch at the
booster. For each booster speed and approach angle, the pitch schedule that maximizes apex
height was solved (`examples/booster.rs`, `apex_opt`):

| booster `|v|`, b/t | best approach | apex, blocks | time to apex |
|---|---|---|---|
| ≤ 0.6 | 90° (straight up) | ≤ 6.2 | ≤ 1.15 s |
| 1.0 | 44.0° | 17.8 | 2.30 s |
| 1.65 (goldrush) | ~30.5° | ~58 | ~3.8 s |
| 2.0 | 27.9° | 86.4 | 4.45 s |
| 3.0 (Monster) | 24.4° | 184.5 | 5.65 s |
| 4.0 | 23.0° | 296.4 | 6.50 s |
| 5.0 | 22.2° | 415.9 | 7.15 s |

The optimum is flat: from 1 b/t up, missing the best approach by 3° costs under 0.4% of the apex
and by 5° under 1.5% (at 3.0 b/t, 0.5 and 1.0 blocks).

**The best approach tends to 19.63° as `|v|` grows.** Every term of the elytra velocity update is
linear in velocity except gravity's `g(0.75 cos²p − 1)`, so scaling `|v|` by `λ` is the same flight
as dividing `g` by `λ`: the best approach is a function of `g/|v|` alone, and its limit is the
gravity-free problem. Solved out to 50 b/t (`runs/booster/asymptote/`, 1° grid over 14–24°,
quadratic refine):

| `|v|`, b/t | 3 | 5 | 10 | 20 | 30 | 50 |
|---|---|---|---|---|---|---|
| best approach | 24.43° | 22.24° | 20.85° | 20.22° | 20.02° | 19.86° |

Fitting a quadratic in `g/|v|` over `|v|` ≥ 6 gives

    θ* ≈ 19.63° + 11.4°/|v| + 8.0°/|v|²        (|v| in b/t)

with residuals of 0.001°. The fitted limit moves by under 0.02° across fit windows (≥ 3, 6, 10)
and between the quadratic and a linear fit on the upper range. It is an extrapolation: the
gravity-free problem was not solved directly.

**Flying the climb:** after the launch, the HUD's gain marker (`argmax ΔTE` held 20 ticks) and
the closed-form gain law (`s(s + A) = K v_z`, `K = 0.771`) both fly within 0.3 blocks of the
optimal apex from every level or upward launch at every speed. They fail on downward launches: at
5° down they lose up to 3 blocks, because the optimum first holds pitch 0 until `v_z` stops rising
(the cycle's snap, `vz_peaked`) and neither marker does. Lookahead 1 is not a climb marker after a
shallow boost: it stays level whenever `v_y/v_z < 0.036/0.128 ≈ 0.28`, because the first nose-up
tick loses energy there.

**Racing to a height instead of the apex.** A target reached sooner than the apex wants a more
nose-up climb than the gain marker, whose 20-tick horizon is sized for the whole climb. The
Monster ceiling, ~180 blocks above the booster and only 4.5 blocks under the best apex, is not that
case: the fastest schedule (25° approach) reaches it at tick 92, and the apex climb and both
markers at tick 93.

**Never aim exactly -90.** Vanilla's `Mth.cos(-90°)` is exactly 0, which skips every lift and
conversion term, and the mouse stop pins pitch there. The sim's libm `cos` is -4.4e-8, which
points the look backwards and costs ~17% of forward speed a tick. `gain_law_pitch` returns exactly
-90 above `v_z ≈ 2.03`, so clamp it (everything here uses 89).

## Reproducing

- `tools/minescript/booster_log.pyj` — the in-game logger (Minescript 5, Pyjinn). Copy it into the
  profile's `minescript/` and run `\booster_log <tag>`; it writes one row per client tick
  (position, velocity, look, and elytra-vario's own markers) and every velocity packet with its
  arrival tick to `minescript/booster_logs/`.
- `examples/booster.rs` — every analysis above; its header lists the modes (`explore`, `cell`,
  `launch`, `ceiling`, `race`, `replay`, `flights`, `lag`, `trace`). Set `TRIG=mth_lut LIM=89`;
  `FLIGHT=algebraic` is identical and 3.6x faster.
- `tools/booster_sweep.sbatch` with `tools/booster_solve.sh` (explorer cells) or
  `tools/booster_cell.sh` (apex only) — the cluster runs, one array task per node, work list
  `$RUN/work.txt` of `<speed> <angle>` lines. The 700-cell explorer took 3.3 minutes on 16 nodes.
- `tools/plot_booster_explorer.py` — `runs/booster/explorer/booster-explorer.html`, the interactive
  explorer (speed slider; every approach's profile at that speed, the best-apex one highlighted;
  apex, time, distance and forward speed against approach; best approach against speed; marker
  losses). Written by Codex from `runs/booster/explorer/BRIEF.md`.
- Data: `runs/booster/` (gitignored). `explorer/cells/` is the 700-cell grid (`|v|` 0.2–5.0 by 0.2;
  approach −5°, 0–90° by 5°, 20–30° by 1°); `asymptote/` is the high-speed grid.
