//! The shared core: physics helpers, the myopic rules, the policy that flies them,
//! and the search primitives the optimizer and the sweep both stand on.
//!
//! Yaw is pinned to zero everywhere here, so the whole problem lives in the `(v_y, v_z)`
//! plane and a schedule is just a list of pitches in degrees. `myopic` analyzes one cycle
//! with these; `sweep` solves a grid of them.

use crate::sim::*;
use rayon::prelude::*;

pub const V0: Vec3 = Vec3::new(0.0, 0.167467, 0.200887);
pub const OPTIMAL_CYCLE_RATE: f64 = 1.43335; // blocks/second, REPLAY_PITCHES_300

pub fn rot(p: f64) -> Rot { Rot { x: p as f32, y: 0.0 } }
pub fn gamma(v: Vec3) -> f64 { (-v.y).atan2(v.z).to_degrees() }

pub fn ticked(s: &State, p: f64) -> State { s.ticked(rot(p)) }
pub fn run_n(s: &State, p: f64, n: usize) -> State {
    let r = rot(p);
    let mut s = s.clone();
    for _ in 0..n { s = s.ticked(r) }
    s
}
/// Replay a schedule from `v0` at the origin. Returns `pitches.len() + 1` states.
pub fn replay_from(v0: Vec3, pitches: &[f64]) -> Vec<State> {
    let mut v = vec![State { pos: Vec3::ZERO, vel: v0 }];
    for &p in pitches { let s = ticked(v.last().unwrap(), p); v.push(s) }
    v
}
pub fn replay(pitches: &[f64]) -> Vec<State> { replay_from(V0, pitches) }
/// A schedule file: whitespace-separated pitches in degrees. Everything from a `#` to the
/// end of its line is a comment, which is where a `sweep` profile keeps its header.
pub fn read_pitches(path: &str) -> Vec<f64> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .lines()
        .flat_map(|l| l.split('#').next().unwrap_or("").split_whitespace())
        .map(|s| s.parse().unwrap())
        .collect()
}

/// Coarse sweep for the global argmax, then a ternary refine inside the winning cell.
/// The objective is not unimodal in pitch, so the sweep has to be global.
pub fn argmax<F: Fn(f64) -> f64>(f: F, step: f64) -> f64 {
    let (mut bp, mut bs) = (0.0, f64::NEG_INFINITY);
    let n = (180.0 / step).round() as i64;
    for i in 0..=n {
        let p = -90.0 + step * i as f64;
        let v = f(p);
        if v > bs { bs = v; bp = p }
    }
    let (mut a, mut b) = ((bp - step).max(-90.0), (bp + step).min(90.0));
    for _ in 0..60 {
        let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
        if f(m1) < f(m2) { a = m1 } else { b = m2 }
    }
    let p = 0.5 * (a + b);
    if f(p) > bs { p } else { bp }
}
// ---------------------------------------------------------------- the rules

/// GAIN. Pitch maximizing the total-energy change over `n` ticks held constant.
/// n = 1 is elytrasim's `argmax_over_pitch_of_delta_energy`; the optimum wants n ~ 20.
pub fn bug_dte_n(s: &State, n: usize) -> f64 {
    let te = s.total_energy();
    argmax(|p| run_n(s, p, n).total_energy() - te, 0.125)
}

/// DIVE. Pitch whose next tick leaves the flight-path angle at `target`.
///
/// gamma(v') is monotone in pitch at dive speeds, but not at low speed, where two branches
/// reach a given angle and only the nose-down one accelerates. So: scan for the last upward
/// crossing, then bisect. Picking the wrong branch stalls the dive completely.
pub fn bug_gamma_to(s: &State, target: f64) -> f64 {
    let h = |p: f64| gamma(ticked(s, p).vel) - target;
    let (mut lo, mut hi) = (f64::NAN, f64::NAN);
    let (mut prev, mut pp) = (h(-90.0), -90.0);
    for i in 1..=1440 {
        let p = -90.0 + 0.125 * i as f64;
        let c = h(p);
        if prev <= 0.0 && c > 0.0 { lo = pp; hi = p }
        prev = c;
        pp = p;
    }
    if lo.is_nan() { return if h(90.0) < 0.0 { 90.0 } else { -90.0 } }
    for _ in 0..60 { let m = 0.5 * (lo + hi); if h(m) <= 0.0 { lo = m } else { hi = m } }
    0.5 * (lo + hi)
}
/// DIVE, as flown: hold the current angle, leaking toward `g_star` at rate `k` per tick.
/// k = 0 is an exact hold, which keeps whatever angle you entered the dive with and loses.
pub fn bug_dive(s: &State, g_star: f64, k: f64) -> f64 {
    let g0 = gamma(s.vel);
    bug_gamma_to(s, g0 + k * (g_star - g0))
}

/// DIVE, leak-free: hold the current angle, but never shallower than `ceiling()`.
///
/// The floor is derived, not fitted: it is the flight-path angle of the fastest steady glide,
/// which is what the dive's gamma settles onto. Entry overshoots it, and from there the rule is
/// an exact hold -- no rate constant anywhere.
pub fn bug_dive_floor(s: &State) -> f64 {
    bug_gamma_to(s, gamma(s.vel).max(ceiling().1))
}

/// The fastest steady glide, `argmax_p eq_vz(p)`, as (pitch, gamma). Swept rather than refined:
/// eq_vz is flat to 1e-9 across the top, so a ternary search there just wanders. Cached, because
/// each equilibrium is 40k iterations of the velocity map.
pub fn ceiling() -> (f64, f64) {
    static C: std::sync::OnceLock<(f64, f64)> = std::sync::OnceLock::new();
    *C.get_or_init(|| {
        let (mut bz, mut bp) = (f64::NEG_INFINITY, 0.0);
        for i in 0..=1800 {
            let p = 0.05 * i as f64;
            let z = equilibrium(p).z;
            if z > bz { bz = z; bp = p }
        }
        (bp, gamma(equilibrium(bp)))
    })
}

/// Terminal glide for a constant pitch: iterate the velocity map to its fixed point.
pub fn equilibrium(p: f64) -> Vec3 {
    let mut v = Vec3::new(0.0, -0.5, 1.0);
    for _ in 0..40000 { v = update_fall_flying_movement(v, rot(p)) }
    v
}

// ---------------------------------------------------------------- cycle segmentation

pub fn segment(ps: &[f64], st: &[State], tag: &str) -> Option<(usize, usize, usize, usize, usize)> {
    let apex: Vec<usize> = (1..ps.len() - 1).filter(|&t| st[t].vel.y > 0.0 && st[t + 1].vel.y <= 0.0).collect();
    if apex.len() < 3 { eprintln!("{tag}: only {} apexes, need 3", apex.len()); return None }
    let (a0, a1) = (apex[1], apex[2]);                       // middle cycle, apex to apex
    // The dive ends where the nose comes down and stays down. Require most of the cycle's
    // speed to be built first: a polished dive often has a level stretch early on, which
    // otherwise reads as the snap and collapses every window downstream.
    let v_top = (a0..a1).map(|t| st[t].vel.length()).fold(0.0, f64::max);
    let t_snap = (a0 + 30..a1 - 3)
        .find(|&t| ps[t] < 5.0 && ps[t + 1] < 5.0 && ps[t + 2] < 5.0 && st[t].vel.length() > 0.8 * v_top)
        .unwrap_or(a1);
    let t_gain = (t_snap..a1).find(|&t| st[t].vel.y > 0.0).unwrap_or(a1);
    let t_gend = (t_gain + 10..a1).find(|&t| ps[t] > 0.0).unwrap_or(a1);
    if t_snap <= a0 + 30 || t_gend <= t_gain + 5 {
        eprintln!("{tag}: could not segment the cycle (snap {t_snap}, gain {t_gain}..{t_gend} in {a0}..{a1})");
        return None;
    }
    Some((a0, t_snap, t_gain, t_gend, a1))
}

// ------------------------------------------------- the price vector, non-circularly

pub type M2 = [f64; 4];                                          // row-major [a b; c d] over (y, z)
pub fn mt_vec(m: &M2, v: (f64, f64)) -> (f64, f64) {             // M^T v
    (m[0] * v.0 + m[2] * v.1, m[1] * v.0 + m[3] * v.1)
}
pub fn mt_mat(a: &M2, m: &M2) -> M2 {                            // A^T M
    [a[0] * m[0] + a[2] * m[2], a[0] * m[1] + a[2] * m[3],
     a[1] * m[0] + a[3] * m[2], a[1] * m[1] + a[3] * m[3]]
}
/// d(next velocity)/d(velocity), central differences in the (v_y, v_z) plane.
pub fn jac(v: Vec3, p: f64) -> M2 {
    let h = 1e-6;
    let r = rot(p);
    let (ya, yb) = (update_fall_flying_movement(Vec3::new(0.0, v.y - h, v.z), r),
                    update_fall_flying_movement(Vec3::new(0.0, v.y + h, v.z), r));
    let (za, zb) = (update_fall_flying_movement(Vec3::new(0.0, v.y, v.z - h), r),
                    update_fall_flying_movement(Vec3::new(0.0, v.y, v.z + h), r));
    [(yb.y - ya.y) / (2.0 * h), (zb.y - za.y) / (2.0 * h),
     (yb.z - ya.z) / (2.0 * h), (zb.z - za.z) / (2.0 * h)]
}

// ---------------------------------------------------------------- the policy

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dive { Leak, Floor, Hold, Target }

#[derive(Clone, Copy, Debug)]
pub struct P {
    pub g_star: f64, pub k: f64, pub s_switch: f64, pub vy_flick: f64, pub s_exit: f64,
    pub slew: f64, pub p_push: f64, pub p_flick: f64, pub n_gain: usize, pub dive: Dive,
}

/// Fly the four bugs, switching on state rather than on the clock, with a pitch rate limit.
pub fn fly(par: P, ticks: usize) -> (Vec<f64>, Vec<u8>, Vec<State>) { fly_pre(par, ticks, &[]) }

/// As `fly`, but the first `pre.len()` ticks of *every* dive replay `pre` open-loop instead of
/// asking the rule. The prefix is part of the control law, not a one-off initial condition, so
/// the limit cycle it settles into is the honest measure of "what does the rule cost once the
/// early dive is handled for it".
pub fn fly_pre(par: P, ticks: usize, pre: &[f64]) -> (Vec<f64>, Vec<u8>, Vec<State>) {
    fly_from(V0, par, ticks, pre)
}

/// As `fly_pre`, from an arbitrary start velocity. The sweep needs this: the policy is the
/// seed for every cell, and `v0` is one of the swept axes.
pub fn fly_from(v0: Vec3, par: P, ticks: usize, pre: &[f64]) -> (Vec<f64>, Vec<u8>, Vec<State>) {
    let mut s = State { pos: Vec3::ZERO, vel: v0 };
    let (mut ps, mut ph, mut st) = (vec![], vec![], vec![s.clone()]);
    let (mut phase, mut last) = (0u8, 0.0f64);
    let mut dive_t = 0usize;
    for _ in 0..ticks {
        let was = phase;
        phase = match phase {
            0 if s.vel.length() >= par.s_switch => 1,     // dive  -> snap
            1 if s.vel.y >= par.vy_flick        => 2,     // snap  -> flick
            2 if last <= par.p_flick + 1e-9     => 3,     // flick -> gain, once the ramp lands
            3 if s.vel.length() <= par.s_exit   => 0,     // gain  -> dive
            p => p,
        };
        if phase == 0 { if was != 0 { dive_t = 0 } } else { dive_t = 0 }
        let want = match phase {
            0 if dive_t < pre.len() => pre[dive_t],
            0 => if gamma(s.vel) < 0.0 { par.p_push } else {
                match par.dive {
                    Dive::Leak => bug_dive(&s, par.g_star, par.k),
                    Dive::Floor => bug_dive_floor(&s),
                    Dive::Hold => bug_gamma_to(&s, gamma(s.vel)),
                    Dive::Target => bug_gamma_to(&s, ceiling().1),
                }
            },
            1 => 0.0,
            2 => par.p_flick,
            _ => bug_dte_n(&s, par.n_gain),
        };
        // the prefix is the optimum's own schedule, so do not slew-limit it
        let p = if phase == 0 && dive_t < pre.len() { want }
                else { want.clamp(last - par.slew, last + par.slew) };
        if phase == 0 { dive_t += 1 }
        s = ticked(&s, p);
        last = p;
        ps.push(p); ph.push(phase); st.push(s.clone());
    }
    (ps, ph, st)
}

pub fn rate_of(par: P, t: usize) -> f64 { fly(par, t).2[t].pos.y / t as f64 * 20.0 }

// ---------------------------------------------------------------- the objective

/// Reference scales for the normalized weight. `REPLAY_PITCHES_300` climbs 21.5 blocks while
/// covering 330 blocks of z, so `lambda = 1` is the weight at which a cycle's height gain and
/// its distance are worth the same.
pub const Y_REF: f64 = 21.5;
pub const Z_REF: f64 = 330.0;

/// `w` from the normalized weight `lambda`. `w` is a pure exchange rate in block units:
/// blocks of height per block of distance.
pub fn w_of_lambda(lambda: f64) -> f64 { lambda * Y_REF / Z_REF }

/// What a profile is optimal *for*. This is the whole content of a profile's header: given
/// these four numbers and the physics, the optimum is determined.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Objective {
    pub v0: Vec3,
    pub n: usize,
    pub lambda: f64,
}

impl Objective {
    pub fn w(&self) -> f64 { w_of_lambda(self.lambda) }

    /// `J = TE(s) + w*z`, both terms in blocks. Terminal velocity is free.
    pub fn j(&self, s: &State) -> f64 { s.total_energy() + self.w() * s.pos.z }

    pub fn replay(&self, pitches: &[f64]) -> Vec<State> { replay_from(self.v0, pitches) }

    /// `J` at the end of the schedule.
    pub fn eval(&self, pitches: &[f64]) -> f64 {
        let mut s = State { pos: Vec3::ZERO, vel: self.v0 };
        for &p in pitches { s = ticked(&s, p) }
        self.j(&s)
    }

    /// `J` averaged over the jitter draws: the value of flying this schedule from a starting
    /// velocity you do not know exactly.
    pub fn eval_jittered(&self, pitches: &[f64], dv: &[Vec3]) -> f64 {
        dv.iter().map(|d| {
            let mut s = State { pos: Vec3::ZERO, vel: self.v0 + *d };
            for &p in pitches { s = ticked(&s, p) }
            self.j(&s)
        }).sum::<f64>() / dv.len() as f64
    }
}

// ---------------------------------------------------------------- robustness

/// Optimize against a *distribution of starting states* rather than one exact state.
///
/// Polished hard against a single exact trajectory, the optimizer finds a singular arc and
/// chatters between 0 and 90 degrees every tick through the late dive. That is not a numerical
/// artifact: at 90 degrees `cos(lean_angle)` is zero, so the lift force vanishes *and*
/// `look_hor_length` vanishes, which gates off both the descent-to-forward conversion and the
/// turning term. Bang-bang between "full aero" and "no aero" really does beat any fixed pitch
/// there -- and no human can fly it.
///
/// A slew-rate limit does not fix this, because the cycle contains genuine flicks: the snap
/// drops to 0 and the flick covers ~88 degrees in about six ticks. Any limit loose enough to
/// keep those is loose enough to keep the chatter.
///
/// Perturbing the *pitches* does not fix it either, and was measured: held-fixed draws made the
/// chatter worse (total variation 2406 against 3493), and per-pass resampling cut it only 29%
/// while costing 2.3 blocks of climb. The reason is that the schedule is not especially
/// sensitive to pitch at the chattering end -- `lift_force = cos^2(lean_angle)` is flat at 90
/// degrees, so a degree of pitch error there barely moves anything.
///
/// What works is perturbing the **initial velocity**. A knife-edge gain is knife-edge in the
/// *state*: it exists at one point in `(v_y, v_z)` and the tick that reaches it. Start from a
/// spread of velocities and no single schedule can sit on that point, because the draws arrive
/// at different states at every tick -- while a real flick, which is robust and merely has to
/// happen, keeps its value across the whole spread. This also states the honest problem: you do
/// not know your velocity to three decimals when you start a cycle.
///
/// `sigma` 0.1 on each component is what this was settled on with, against a reference start of
/// about `(0.17, 0.20)` -- a perturbation of the same order as the velocity itself, so the
/// schedule is being asked to work over a genuinely wide basin rather than to be locally smooth.
/// The draws are *common random numbers* within a pass: one set, reused for every candidate at
/// every tick, so the smoothed objective is a deterministic function of the schedule and the
/// line search is not chasing sampling noise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jitter {
    /// Standard deviation of the initial-velocity error, in blocks/tick, applied independently
    /// to `v_y` and `v_z`. 0 disables jitter.
    pub sigma: f64,
    /// How many draws to average over. Cost is linear in this.
    pub draws: usize,
    /// Redraw the perturbations every pass instead of holding one set fixed.
    ///
    /// This is the difference between smoothing and regularizing, and it matters. Held fixed,
    /// the draws define a deterministic surrogate objective with knife edges of its own, and
    /// the optimizer finds those instead. Redrawn each pass, no single realization can be
    /// exploited, so a gain has to survive fresh draws to be kept.
    pub resample: bool,
    pub seed: u64,
}

impl Default for Jitter {
    fn default() -> Self { Jitter { sigma: 0.0, draws: 8, resample: true, seed: 0x5eed_1eaf } }
}

impl Jitter {
    pub fn is_on(&self) -> bool { self.sigma > 0.0 && self.draws > 0 }
    pub fn k(&self) -> usize { if self.is_on() { self.draws.max(1) } else { 1 } }

    /// The velocity offsets, one per draw. Deterministic in `seed`, so a profile can be
    /// re-certified against the same smoothed objective it was optimized under.
    pub fn draws_at_0(&self) -> Vec<Vec3> { self.draws_at(0) }

    /// The offsets for pass `pass`. Identical for every pass unless `resample` is set.
    pub fn draws_at(&self, pass: u64) -> Vec<Vec3> {
        if !self.is_on() { return vec![Vec3::ZERO] }
        let salt = if self.resample { pass.wrapping_mul(0x9e37_79b9_7f4a_7c15) } else { 0 };
        let mut st = self.seed.wrapping_add(salt).wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut next = || -> f64 {                       // splitmix64 -> uniform in (0, 1)
            st = st.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = st;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^= z >> 31;
            ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let mut normal = || {                            // Box-Muller
            let (u1, u2) = (next(), next());
            self.sigma * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
        };
        (0..self.draws).map(|_| {
            let (y, z) = (normal(), normal());
            Vec3 { x: 0.0, y, z }
        }).collect()
    }
}

// ---------------------------------------------------------------- what a hand can do

/// A cost on how much *hand movement* a schedule asks for, and optional hard caps on it.
///
/// Why the second difference and not the first. Total variation cannot separate a flick from
/// chatter, and the numbers say why: the reference cycle's sharpest move is 38.3 deg/tick while
/// chatter runs at 90, a factor of 2.3, so any slew budget wide enough for the snap is wide
/// enough for the alternation. The *rate of change* of pitch separates them cleanly. Measured
/// over the whole 300-tick schedule:
///
/// ```text
///                       max|dp|  max|d2p|  p95|d2p|  sum|d2p|
/// reference cycle          38.3      37.9       0.4     147.1
/// hard-polished (chatter)  90.0     180.0     153.2    4211.1
/// ```
///
/// A factor of 4.7 at the max and 380 at the 95th percentile. The reference is a straight line
/// almost everywhere with a handful of corners -- its curvature is *sparse* -- which is why the
/// norm here is l1 and not l2. `sum |d2p|` with an l1 penalty is l1 trend filtering, whose
/// solutions are piecewise linear with adaptively placed breakpoints: it buys a corner wherever
/// one earns its keep and charges nothing for the straight runs between them. That is the shape
/// a hand produces, and it is the shape the reference cycle already has.
///
/// This is a *stated* term in the utility function, not a stopping rule. The point is that the
/// polish can then run to convergence: `mu` names the exchange rate between blocks of `J` and
/// degrees per tick squared of wrist, and the answer at that rate is a real optimum rather than
/// wherever the optimizer happened to be when it was interrupted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rough {
    /// Blocks of `J` charged per degree/tick^2 of summed |second difference|.
    pub mu: f64,
    /// Blocks of `J` charged per degree/tick of summed |first difference| -- plain total
    /// variation. Off by default; kept so the two norms can be compared on one footing.
    pub mu_tv: f64,
    /// Hard cap on |p[t+1] - 2p[t] + p[t-1]|, deg/tick^2. A constraint rather than a price, so
    /// inside the feasible set it does not distort `J` at all. `INFINITY` disables it.
    pub cap: f64,
    /// Hard cap on |p[t+1] - p[t]|, deg/tick. `INFINITY` disables it.
    pub slew_cap: f64,
    /// Largest |pitch| the schedule may use, in degrees. 90 -- the game's own clamp -- allows
    /// the optimizer right up to a discontinuity, and it goes there.
    ///
    /// Both ends of the range are gates, not limits. `look_hor_length` is |cos(pitch)| and every
    /// conversion term is guarded by `look_hor_length > 0`, so where cos underflows to zero the
    /// aerodynamics switch off entirely, and where it changes sign "forward" reverses. Which of
    /// those you hit depends on the trig:
    ///
    /// * `mth_lut` (vanilla). `Mth::cos(-90 deg)` indexes `SIN[0]`, which is exactly 0.0, so the
    ///   gate fails and pitch -90 is a dead tick -- no lift, no conversion, no turning, just
    ///   gravity. At +90 the same table gives +9.6e-5 and the gate passes. The flick wants the
    ///   conversion at its maximum, which is at -90, so the optimizer parks at -89.989: one
    ///   table cell short of the cliff. Overshoot by 0.05 degrees and the schedule loses 5.2
    ///   blocks.
    /// * `libm`. `cos(90 deg)` in f32 is -4.4e-8, *negative*, so `look_angle.z/look_hor_length`
    ///   flips to -1 and the turning term becomes `-0.2 * v_z` -- twenty percent of your forward
    ///   speed per tick. The optimizer parks where cos is +2e-7, within 1e-5 degrees of that.
    ///
    /// Neither is a knob anyone can hold. Minecraft delivers rotation in steps of about
    /// 0.15 * sensitivity degrees, so a margin of a degree or so costs the schedule almost
    /// nothing and removes both cliffs. This is a *control-space* statement, like `cap`: the
    /// pitches outside it are not worse, they are unavailable.
    pub limit: f64,
}

impl Default for Rough {
    fn default() -> Self {
        Rough { mu: 0.0, mu_tv: 0.0, cap: f64::INFINITY, slew_cap: f64::INFINITY, limit: 90.0 }
    }
}

/// The five pitches `Rough` needs around tick `t`, lifted out of the schedule so a coordinate
/// search can hold them while the schedule itself is mid-sweep. `None` off the ends.
pub type Win5 = [Option<f64>; 5];

/// The window at `t`: indices `t-2 ..= t+2`, `None` where that runs off the schedule.
pub fn win5(p: &[f64], t: usize) -> Win5 {
    let n = p.len() as isize;
    let t = t as isize;
    std::array::from_fn(|k| {
        let i = t + k as isize - 2;
        if i >= 0 && i < n { Some(p[i as usize]) } else { None }
    })
}

impl Rough {
    pub fn is_on(&self) -> bool {
        self.mu != 0.0 || self.mu_tv != 0.0 || self.cap.is_finite() || self.slew_cap.is_finite()
            || self.limit < 90.0
    }
    pub fn prices(&self) -> bool { self.mu != 0.0 || self.mu_tv != 0.0 }

    /// The whole schedule's roughness cost, in blocks of `J`.
    pub fn cost(&self, p: &[f64]) -> f64 {
        let mut c = 0.0;
        if self.mu != 0.0 {
            for j in 0..p.len().saturating_sub(2) { c += self.mu * (p[j] - 2.0 * p[j + 1] + p[j + 2]).abs() }
        }
        if self.mu_tv != 0.0 {
            for j in 0..p.len().saturating_sub(1) { c += self.mu_tv * (p[j + 1] - p[j]).abs() }
        }
        c
    }

    /// Just the terms the pitch at the window's center takes part in, evaluated at `x`. This is
    /// what a coordinate search has to add to its score: everything else is a constant.
    pub fn local(&self, w: &Win5, x: f64) -> f64 {
        if !self.prices() { return 0.0 }
        let g = |i: usize| if i == 2 { Some(x) } else { w[i] };
        let mut c = 0.0;
        if self.mu != 0.0 {
            for j in 0..3 {
                if let (Some(a), Some(b), Some(d)) = (g(j), g(j + 1), g(j + 2)) {
                    c += self.mu * (a - 2.0 * b + d).abs()
                }
            }
        }
        if self.mu_tv != 0.0 {
            for j in 1..3 {
                if let (Some(a), Some(b)) = (g(j), g(j + 1)) { c += self.mu_tv * (b - a).abs() }
            }
        }
        c
    }

/// How far outside the admissible set a schedule is, in degrees; 0 when it is inside.
    ///
    /// This exists because `certify` was quietly lying without it. `residuals_reg` asks
    /// `feasible` what each pitch may be, and when the *neighbors* already break a cap the
    /// answer is empty, so the search collapses to the single point it started from and reports
    /// a gain of zero. A schedule with 240 deg/tick^2 of curvature under a cap of 0 certified at
    /// residual 0.0 -- a perfect score for a profile that does not satisfy its own header.
    ///
    /// A residual only means anything for a schedule that is *in* the set it claims to be in, so
    /// `certify_reg` checks this first and returns infinity when it is not.
    pub fn violation(&self, p: &[f64]) -> f64 {
        let mut v: f64 = 0.0;
        for &x in p { v = v.max(x.abs() - self.limit) }
        if self.cap.is_finite() {
            for j in 0..p.len().saturating_sub(2) {
                v = v.max((p[j] - 2.0 * p[j + 1] + p[j + 2]).abs() - self.cap)
            }
        }
        if self.slew_cap.is_finite() {
            for j in 0..p.len().saturating_sub(1) {
                v = v.max((p[j + 1] - p[j]).abs() - self.slew_cap)
            }
        }
        v.max(0.0)
    }

    /// The interval the center pitch may take without breaking a cap, intersected with
    /// `[lo, hi]`. Returns `None` when the caps and the neighbors are already inconsistent,
    /// which a warm start can arrive in; the caller then leaves the tick alone.
    pub fn feasible(&self, w: &Win5, lo: f64, hi: f64) -> Option<(f64, f64)> {
        let (mut a, mut b) = (lo.max(-self.limit), hi.min(self.limit));
        let mut clip = |l: f64, h: f64| { a = a.max(l); b = b.min(h) };
        if self.cap.is_finite() {
            let c = self.cap;
            // |p[t-2] - 2p[t-1] + x| <= c
            if let (Some(p0), Some(p1)) = (w[0], w[1]) { let m = 2.0 * p1 - p0; clip(m - c, m + c) }
            // |p[t-1] - 2x + p[t+1]| <= c
            if let (Some(p1), Some(p3)) = (w[1], w[3]) { let s = p1 + p3; clip(0.5 * (s - c), 0.5 * (s + c)) }
            // |x - 2p[t+1] + p[t+2]| <= c
            if let (Some(p3), Some(p4)) = (w[3], w[4]) { let m = 2.0 * p3 - p4; clip(m - c, m + c) }
        }
        if self.slew_cap.is_finite() {
            let c = self.slew_cap;
            if let Some(p1) = w[1] { clip(p1 - c, p1 + c) }
            if let Some(p3) = w[3] { clip(p3 - c, p3 + c) }
        }
        if a <= b { Some((a, b)) } else { None }
    }
}

/// Summed |second difference| of the schedule, deg/tick^2. The statistic `Rough::mu` prices,
/// and the one that separates a flick from chatter -- see `Rough`.
pub fn curvature_l1(p: &[f64]) -> f64 {
    (0..p.len().saturating_sub(2)).map(|j| (p[j] - 2.0 * p[j + 1] + p[j + 2]).abs()).sum()
}

/// The largest |second difference|, deg/tick^2: the peak angular acceleration the schedule asks
/// the hand for. 37.9 on the reference cycle, 180 -- the geometric maximum -- on a chattering one.
pub fn curvature_max(p: &[f64]) -> f64 {
    (0..p.len().saturating_sub(2)).map(|j| (p[j] - 2.0 * p[j + 1] + p[j + 2]).abs()).fold(0.0, f64::max)
}

// ---------------------------------------------------------------- the optimizer

/// How hard to polish. The defaults are `cmd_polish`'s, which is the schedule every number in
/// `README-myopic.md` was produced with.
#[derive(Clone, Copy, Debug)]
pub struct PolishOpts {
    pub max_passes: usize,
    /// Sweep the whole pitch range every this many passes; otherwise search near the current
    /// pitch. The global sweep is what handles the objective not being unimodal in pitch.
    pub global_every: usize,
    pub global_step: f64,
    pub local_span: f64,
    pub local_step: f64,
    pub ternary_iters: usize,
    /// Stop once the schedule has gained less than this many blocks of `J` over the last
    /// `stall_window` passes. Default 0.1: past that the optimizer is tuning below the
    /// precision anyone could fly, and the extra precision is the part most likely to be
    /// brittle.
    ///
    /// Note what this is *not*. The per-tick residual that `certify` reports bounds only
    /// single-coordinate moves, and a schedule can sit blocks below a better optimum with no
    /// single tick improving by more than a hundredth -- because reaching it needs many ticks
    /// to move together. Measured: a 130-tick cell stopped at residual 0.066 came out 4.7
    /// blocks short of the same cell polished properly. So convergence is judged on progress
    /// in `J`, and the residual is reported as a certificate rather than used as a target.
    pub tol: f64,
    /// Block size for `block_step`, run before each global pass. 0 disables it, which is the
    /// default: it is a wash on wall-clock. See `block_step`.
    pub block: usize,
    /// Optimize against a jittered schedule. See `Jitter`; costs `draws` times as much.
    pub jitter: Jitter,
    /// How many passes of near-no-progress to require before stopping. Coordinate ascent on
    /// this problem stalls and then jumps -- a global pass finds a new basin every so often --
    /// so a single quiet pass means nothing.
    pub stall_window: usize,
    /// Stop as soon as the schedule turns degenerate: the lag-1 correlation of its per-tick
    /// changes falling below this ends the polish and *discards* the pass that crossed it.
    ///
    /// This is the regularizer. Polished to convergence the exact objective genuinely prefers a
    /// chattering singular arc -- residual 9.4e-6 at 200 passes, so it is a real coordinate
    /// optimum, not a failure to converge -- and no perturbation of the objective that was
    /// measured avoids it: initial-velocity jitter is flat in schedule smoothness across an 8x
    /// range of sigma (total variation 925, 871, 838, 826, 884 at sigma 0, 0.05, 0.1, 0.2, 0.4
    /// over 60 passes) while costing up to 8 blocks of distance. Stopping time is what works.
    ///
    /// It matters more under continuation than it looks: a warm-started cell inherits its
    /// neighbor's polish, so a per-cell pass budget accumulates along the path and a cell far
    /// from the anchor has been polished many times over. A budget cannot bound that; a
    /// property of the schedule can.
    ///
    /// `f64::NEG_INFINITY` disables the guard.
    pub lag1_floor: f64,
    /// What the schedule is charged for hand movement, and any hard caps on it. See `Rough`.
    ///
    /// This is the alternative to a stopping rule. `lag1_floor` bounds chatter by ending the
    /// polish before it appears, which leaves the answer depending on when the optimizer was
    /// interrupted; a price on curvature bounds it by making it cost something, which leaves an
    /// answer that can be polished to convergence and certified.
    pub rough: Rough,
}

impl Default for PolishOpts {
    fn default() -> Self {
        PolishOpts {
            max_passes: 200, global_every: 4, global_step: 0.25,
            local_span: 8.0, local_step: 0.05, ternary_iters: 70, tol: 0.1, block: 0, stall_window: 12,
            // Off by default. It reads well but interacts badly with continuation: a warm
            // start arrives with a seam where `stretch` joined, that reads as chatter, and the
            // guard then stops the polish that would have repaired it -- freezing a chain of
            // cells at an unimproved copy of their parent. Stopping time is applied as a pass
            // budget instead. Set it explicitly for a single cold polish, where it works.
            lag1_floor: f64::NEG_INFINITY,
            jitter: Jitter::default(),
            rough: Rough::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Polished {
    pub pitches: Vec<f64>,
    pub j: f64,
    pub passes: usize,
    /// What the last pass gained. Small means the schedule stopped moving under *this* search.
    pub last_gain: f64,
    /// What a fresh full global pass can still find, from `certify`. This is the number that
    /// makes the file's claim checkable; it does not depend on how the schedule was reached.
    pub residual: f64,
    /// The degeneracy statistic at the schedule that was kept. See `lag1`.
    pub lag1: f64,
    /// Whether the polish ended because the schedule was turning degenerate rather than
    /// because it stopped gaining. Recorded because it changes what the residual means: the
    /// profile is deliberately short of the coordinate optimum, not converged to it.
    pub stopped_degenerate: bool,
    /// What the roughness price took off `j`. Reported separately so the raw `J` of a
    /// regularized answer is still readable: the schedule is worth `j` blocks and cost
    /// `rough_cost` blocks of wrist to fly.
    pub rough_cost: f64,
}

/// The nearest `f32` to `x` that lies inside `[lo, hi]`, or `None` if the interval holds none.
///
/// Pitch is stored as `f32`, so every candidate is rounded before it is flown -- and rounding is
/// nearest-even, which can step back *out* of an interval the caller just clamped into. Small:
/// a zero-pass polish at `limit = 0.1` wrote 0.10000000149011612, over by 1.5e-9 degrees. But
/// `Rough::violation` gates certification now, so "over by 1.5e-9" is the difference between a
/// profile satisfying its own header and failing its own `verify`.
fn f32_inside(x: f64, lo: f64, hi: f64) -> Option<f64> {
    let nudge = |v: f32, up: bool| -> f32 {
        if v == 0.0 { return if up { f32::from_bits(1) } else { -f32::from_bits(1) } }
        let b = v.to_bits();
        f32::from_bits(if (v > 0.0) == up { b + 1 } else { b - 1 })
    };
    let mut v = x.clamp(lo, hi) as f32;
    for _ in 0..3 { if (v as f64) < lo { v = nudge(v, true) } else { break } }
    for _ in 0..3 { if (v as f64) > hi { v = nudge(v, false) } else { break } }
    ((v as f64) >= lo && (v as f64) <= hi).then_some(v as f64)
}

/// Best pitch for tick `t`, holding every other tick fixed: a global sweep, then a ternary
/// refine inside the winning cell, scored on the exact tail. Returns `(pitch, J)` with the
/// pitch already rounded to `f32`, because the sim casts pitch to `f32` anyway -- so the value
/// returned is the one that will actually be flown, and its score is the score of flying it.
///
/// `pen` is the roughness price of putting a given pitch here -- the terms of `Rough::cost`
/// that this tick takes part in. It is subtracted from the score, so the coordinate search
/// optimizes the same regularized objective the polish reports, not `J` alone.
fn best_pitch_at(obj: &Objective, s: &[State], tail: &[f64],
                 cur: f64, lo: f64, hi: f64, step: f64, ternary_iters: usize,
                 pen: &(dyn Fn(f64) -> f64 + Sync)) -> (f64, f64, f64) {
    // One prefix state per jitter draw. The draws differ only in where they started, so the
    // tail is the exact schedule flown from each of them -- this is E[J | v0 + dv] under common
    // random numbers, with no per-tick noise to average away.
    let score = |p: f64| -> f64 {
        s.iter().map(|s0| {
            let mut st = ticked(s0, p);
            for &q in tail { st = ticked(&st, q) }
            obj.j(&st)
        }).sum::<f64>() / s.len() as f64 - pen(p)
    };
    let steps = ((hi - lo) / step).round() as i64;
    // `steps` is a rounded count, so `lo + step*steps` overshoots `hi` whenever the interval is
    // not an exact multiple of the step -- about half of them, by up to half a step. That is
    // 0.125 degrees at the global step, and it would let the search return a pitch outside the
    // admissible set the caller just computed. Clamp the sample, not just the ternary bracket.
    let at = |i: i64| f32_inside(lo + step * i as f64, lo, hi);
    // The tail replays are independent, so the sweep is exactly parallel -- no approximation,
    // just the same evaluations on more cores.
    let j_cur = score(cur);
    // The incumbent has to be *inside* the interval, or a warm start that arrives out of bounds
    // is never brought in: it would simply out-score every candidate and be kept. `j_cur` still
    // reports the score of where we actually are, because that is what a residual is measured
    // against.
    // No representable pitch inside the interval: leave the tick alone rather than step out of it.
    let Some(start) = f32_inside(cur, lo, hi) else { return (cur, j_cur, j_cur) };
    let (mut bp, mut bs) = (start, if start == cur { j_cur } else { score(start) });
    let (gp, gs) = (0..steps + 1)
        .into_par_iter()
        .filter_map(|i| at(i).map(|p| (p, score(p))))
        .reduce(|| (start, f64::NEG_INFINITY), |a, b| if b.1 > a.1 { b } else { a });
    if gs > bs { bp = gp; bs = gs }

    // Bracket inside [lo, hi]: the refine must not walk back out of the admissible set.
    let (mut a, mut b) = ((bp - step).max(lo), (bp + step).min(hi));
    for _ in 0..ternary_iters {
        let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
        if score(m1) < score(m2) { a = m1 } else { b = m2 }
    }
    // Round first, then score: the schedule that gets written must be the schedule that was
    // measured, or a cell can be certified on a pitch it does not contain.
    let refined = f32_inside(0.5 * (a + b), lo, hi);
    let (p, j) = match refined {
        Some(r) => { let rs = score(r); if rs > bs { (r, rs) } else { (bp, bs) } }
        None => (bp, bs),
    };
    (p, j, j_cur)
}

/// What a full global pass would do at every tick, without doing it: the best pitch, the move
/// it implies, and what that move is worth. This is the raw material for both `certify` (the
/// largest gain) and for asking whether the moves point the same way.
pub fn residuals(obj: &Objective, pitches: &[f64], step: f64, jit: Jitter) -> Vec<(f64, f64)> {
    residuals_reg(obj, pitches, step, jit, Rough::default())
}

/// `residuals` against the regularized objective. A profile written under a roughness price is
/// a coordinate optimum of *that* objective, so its certificate has to be re-derived under the
/// same price or it will report a gain the writer deliberately did not take.
pub fn residuals_reg(obj: &Objective, pitches: &[f64], step: f64, jit: Jitter, rough: Rough)
    -> Vec<(f64, f64)> {
    let dv = jit.draws_at_0();
    let states = jittered_replays(obj, pitches, &dv);
    (0..pitches.len())
        .map(|t| {
            let row: Vec<State> = states.iter().map(|st| st[t].clone()).collect();
            let w = win5(pitches, t);
            let pen = |x: f64| rough.local(&w, x);
            let (lo, hi) = rough.feasible(&w, -90.0, 90.0).unwrap_or((pitches[t], pitches[t]));
            let (p, j, j_cur) = best_pitch_at(obj, &row, &pitches[t + 1..], pitches[t],
                                              lo, hi, step, 70, &pen);
            (p - pitches[t], j - j_cur)
        })
        .collect()
}

/// One replay per jitter draw. `out[k][t]` is the state at tick `t` having started from
/// `v0 + dv[k]`.
fn jittered_replays(obj: &Objective, pitches: &[f64], dv: &[Vec3]) -> Vec<Vec<State>> {
    dv.iter().map(|d| {
        let mut v = vec![State { pos: Vec3::ZERO, vel: obj.v0 + *d }];
        for &p in pitches {
            let s = ticked(v.last().unwrap(), p);
            v.push(s);
        }
        v
    }).collect()
}

/// One full global pass that changes nothing, reporting the largest gain it could have made.
///
/// This is the profile's certificate. It re-derives the claim from `(v0, n, lambda, pitches)`
/// alone, so it can be run by anyone holding the file and says nothing about how the schedule
/// was found. A residual at or below the writer's tolerance means the schedule is a coordinate
/// optimum to that tolerance.
pub fn certify(obj: &Objective, pitches: &[f64], step: f64, jit: Jitter) -> f64 {
    certify_reg(obj, pitches, step, jit, Rough::default())
}

/// `certify` under a roughness price. See `residuals_reg`.
///
/// Infinite for a schedule that is outside the admissible set its own header names. A residual
/// is "how much could one coordinate move gain", and the coordinate search cannot move at all
/// when the neighbors are already illegal -- so without this check an infeasible profile scores
/// a perfect 0.0. See `Rough::violation`.
pub fn certify_reg(obj: &Objective, pitches: &[f64], step: f64, jit: Jitter, rough: Rough) -> f64 {
    if rough.violation(pitches) > 0.0 { return f64::INFINITY }
    residuals_reg(obj, pitches, step, jit, rough).iter().map(|x| x.1).fold(0.0, f64::max)
}

/// Is the per-tick correction *smooth in t*?
///
/// A different question from whether the corrections share a sign overall. A delta that is
/// smoothly positive over the first half and negative over the second sums to nothing, so it
/// looks like balanced noise in bulk, while being highly structured and worth exploiting.
/// What matters for a big-step method is whether neighboring ticks want the same correction,
/// because then the whole error lives in a handful of smooth modes.
///
/// Returns the lag-1 correlation of the deltas, the mean length of a same-sign run, and the
/// fraction of the deltas' energy captured by the first `k` cosine modes for a few `k`.
pub fn delta_structure(d: &[f64]) -> (f64, f64, Vec<(usize, f64)>) {
    let n = d.len();
    // Ticks whose pitch is already best are exactly 0, and a run of zeros correlates with
    // itself perfectly -- which would report a smooth correction where there is no correction
    // at all. Measure on the ticks that actually want to move.
    let live: Vec<f64> = d.iter().cloned().filter(|x| x.abs() > 1e-9).collect();
    let m = live.len();
    let lag1 = if m > 2 {
        let mean = live.iter().sum::<f64>() / m as f64;
        let var: f64 = live.iter().map(|x| (x - mean).powi(2)).sum();
        // adjacent *in the schedule*, both live
        let pairs: Vec<(f64, f64)> = (0..n - 1)
            .filter(|&t| d[t].abs() > 1e-9 && d[t + 1].abs() > 1e-9)
            .map(|t| (d[t] - mean, d[t + 1] - mean)).collect();
        if var > 0.0 && !pairs.is_empty() {
            pairs.iter().map(|(a, b)| a * b).sum::<f64>() / var * (m as f64 / pairs.len() as f64)
        } else { 0.0 }
    } else { 0.0 };

    let mut runs = 1usize;
    for t in 1..m { if (live[t] >= 0.0) != (live[t - 1] >= 0.0) { runs += 1 } }
    let run_len = m as f64 / runs as f64;

    // DCT-II. O(n^2), and n is at most a few hundred.
    let total: f64 = d.iter().map(|x| x * x).sum();
    let coef: Vec<f64> = (0..n).map(|k| {
        (0..n).map(|t| d[t] * (std::f64::consts::PI / n as f64
                               * (t as f64 + 0.5) * k as f64).cos()).sum::<f64>()
    }).collect();
    // Parseval for DCT-II: sum_k c_k^2 * (2 - [k==0]) / (2n) = sum_t d_t^2
    let energy = |k: usize| -> f64 {
        let e: f64 = (0..k.min(n)).map(|j| coef[j] * coef[j] * if j == 0 { 1.0 } else { 2.0 }).sum();
        if total > 0.0 { (e / (2.0 * n as f64) / total).min(1.0) } else { 0.0 }
    };
    let ks = [1, 2, 4, 8, 16, 32].iter().map(|&k| (k, energy(k))).collect();
    (lag1, run_len, ks)
}

/// Move every tick toward its own best pitch at once, and line-search how far to go.
///
/// Coordinate ascent changes one tick at a time, so when many ticks want to move the same way
/// it makes the move n times over, each one partly undone by its neighbors. Measured: a
/// schedule 4.7 blocks short of its optimum had 0.79 coherence -- the sum of its per-tick moves
/// was 709 degrees against 901 degrees of absolute movement, nearly all one-signed -- while a
/// well-polished one sat at -0.09, balanced noise. So the one-sidedness is a symptom of being
/// stuck, and this step is the cure for exactly that case and a no-op otherwise.
///
/// Measured, and it does not pay, which is why it is off by default. The best step is only
/// `alpha` 0.23 to 0.43 and captures 23-43% of the summed per-tick gains: the ticks want to
/// move together but they *interact*, so moving them all at once invalidates each one's target.
/// Coordinate ascent already handles that by updating the state as it sweeps, and matches or
/// beats this at every pass count. Kept because the diagnostic is the evidence.
///
/// Costs one global pass plus a handful of replays. Returns `None` when no step helps.
pub fn jacobi_step(obj: &Objective, pitches: &[f64], step: f64, jit: Jitter)
    -> Option<(Vec<f64>, f64, f64)> {
    let d = residuals(obj, pitches, step, jit);
    let dv = jit.draws_at_0();
    let base = obj.eval_jittered(pitches, &dv);
    let at = |alpha: f64| -> Vec<f64> {
        pitches.iter().zip(&d)
            .map(|(p, (delta, _))| (p + alpha * delta).clamp(-90.0, 90.0) as f32 as f64)
            .collect()
    };
    // Coarse sweep then bisection-free refine: the gain along alpha is not unimodal either,
    // since the schedule crosses the pitch-0 corner at different alphas for different ticks.
    let (mut best_a, mut best_j) = (0.0, base);
    let mut alpha = 0.02;
    while alpha <= 1.6 {
        let j = obj.eval(&at(alpha));
        if j > best_j { best_j = j; best_a = alpha }
        alpha += 0.02;
    }
    if best_a == 0.0 { return None }
    // refine around the winner
    let (lo, hi) = (best_a - 0.02, best_a + 0.02);
    for i in 1..20 {
        let a = lo + (hi - lo) * i as f64 / 20.0;
        let j = obj.eval(&at(a));
        if j > best_j { best_j = j; best_a = a }
    }
    Some((at(best_a), best_a, best_j - base))
}

/// Line-search a separate step size for each block of ticks.
///
/// The corrections are locally correlated but not globally so: lag-1 around +0.46 to +0.50 on
/// an under-converged cell, with same-sign runs of 15 to 24 ticks, while only a quarter of
/// their energy sits in the first 32 cosine modes. That is the shape of an error that is
/// roughly constant over a phase and unrelated between phases -- so one step size for the
/// whole schedule averages independent corrections against each other and finds nothing, and a
/// low-mode projection misses them too. A step size per block matches the measured structure.
///
/// Blocks are taken greedily, so each one's line search already sees the blocks before it.
///
/// Measured, and it is a wash, which is why it is off by default. It does beat a single
/// whole-schedule step, confirming the diagnosis -- but the block size barely matters (5
/// through 80 all land within 0.01 blocks), so the gain is not really about matching the block
/// structure, and on wall-clock it buys at 16 passes what plain coordinate ascent reaches at
/// 19, while costing 22% more per pass.
///
/// The underlying reason no big step pays: each tick's target is computed holding the others
/// fixed, and they interact strongly. The best step along the aggregate direction is only
/// `alpha` 0.23 to 0.43 and captures 23-43% of the summed per-tick gains. Coordinate ascent
/// already accounts for the interaction by updating the state as it sweeps, which is exactly
/// what a simultaneous step throws away.
pub fn block_step(obj: &Objective, pitches: &[f64], step: f64, block: usize, jit: Jitter)
    -> Option<(Vec<f64>, f64)> {
    block_step_reg(obj, pitches, step, block, jit, Rough::default())
}

/// `block_step` against the regularized objective.
///
/// The unregularized one is *wrong* to use under a price and not subtly: it line-searches on
/// `J` alone, so it accepts a step that raises `J` while raising the curvature far more, and
/// reports the size of the `J` gain as though that were the improvement. Measured at `mu = 0.1`
/// it took a schedule from 0.188 to -4.168 on the regularized objective and called it a gain of
/// 0.19; at `mu = 1`, from 0.188 to -48.2. `polish` calls this one.
pub fn block_step_reg(obj: &Objective, pitches: &[f64], step: f64, block: usize, jit: Jitter,
                      rough: Rough) -> Option<(Vec<f64>, f64)> {
    let d = residuals_reg(obj, pitches, step, jit, rough);
    let dv = jit.draws_at_0();
    let score = |p: &[f64]| obj.eval_jittered(p, &dv) - rough.cost(p);
    let base = score(pitches);
    let mut cur = pitches.to_vec();
    let mut best_j = base;
    for start in (0..pitches.len()).step_by(block) {
        let end = (start + block).min(pitches.len());
        let at = |alpha: f64| -> Vec<f64> {
            let mut v = cur.clone();
            for t in start..end {
                v[t] = (cur[t] + alpha * (pitches[t] + d[t].0 - cur[t]))
                    .clamp(-rough.limit, rough.limit) as f32 as f64;
            }
            v
        };
        let (mut ba, mut bj) = (0.0, best_j);
        let mut alpha = 0.05;
        while alpha <= 1.5 {
            let j = score(&at(alpha));
            if j > bj { bj = j; ba = alpha }
            alpha += 0.05;
        }
        if ba > 0.0 { cur = at(ba); best_j = bj }
    }
    // A block step moves whole spans at once, so it can leave the admissible set even though
    // every endpoint is clamped -- reject rather than repair, since the caller has a perfectly
    // good schedule already.
    if rough.violation(&cur) > 0.0 { return None }
    (best_j > base).then(|| (cur, best_j - base))
}

/// Coordinate ascent over the schedule: sweep `t = 0..n`, replacing each pitch with the best
/// one given the rest. Non-unimodality in pitch is why the sweep has to be global, and the
/// corner at pitch 0 -- the forward-to-up conversion is gated on `lean_angle < 0` -- is why it
/// cannot be replaced by a derivative method.
pub fn polish(obj: &Objective, init: &[f64], opts: PolishOpts) -> Polished {
    assert_eq!(init.len(), obj.n, "schedule length must match the objective's horizon");
    // Project the seed into the admissible set first. A warm start routinely arrives outside it
    // -- every relaxed solution parks at +-90 -- and the sweep is a local move, not a repair:
    // where the neighbors are out of bounds `Rough::feasible` has nothing to offer and every
    // tick is skipped.
    let lim = opts.rough.limit;
    let mut pitches: Vec<f64> = project_cap(init, opts.rough)
        .into_iter().map(|p| f32_inside(p, -lim, lim).unwrap_or(0.0)).collect();
    let mut dv = opts.jitter.draws_at(0);
    let mut states = jittered_replays(obj, &pitches, &dv);
    let (mut passes, mut last_gain) = (0, f64::INFINITY);
    let mut stopped_degenerate = false;
    let mut recent: std::collections::VecDeque<f64> = std::collections::VecDeque::new();

    for pass in 0..opts.max_passes {
        if opts.jitter.is_on() && opts.jitter.resample && pass > 0 {
            dv = opts.jitter.draws_at(pass as u64);
            states = jittered_replays(obj, &pitches, &dv);
        }
        let before = obj.eval_jittered(&pitches, &dv) - opts.rough.cost(&pitches);
        let prev = pitches.clone();
        let global = pass % opts.global_every == 0;
        // Before each global pass, try moving the whole schedule at once. When the per-tick
        // moves point the same way this leaps; when they do not it finds no step and costs
        // one pass.
        if global && opts.block > 0 {
            if let Some((next, gain)) = block_step_reg(obj, &pitches, opts.global_step, opts.block,
                                                       opts.jitter, opts.rough) {
                if gain > 0.0 { pitches = next; states = jittered_replays(obj, &pitches, &dv) }
            }
        }
        let mut worst_tick = 0.0f64;
        for t in 0..obj.n {
            let cur = pitches[t];
            let (lo, hi, step) = if global {
                (-90.0, 90.0, opts.global_step)
            } else {
                ((cur - opts.local_span).max(-90.0), (cur + opts.local_span).min(90.0), opts.local_step)
            };
            let row: Vec<State> = states.iter().map(|st| st[t].clone()).collect();
            // The neighbors as they stand right now, so the price this tick pays reflects the
            // ticks already moved in this sweep -- Gauss-Seidel on the regularized objective,
            // not on `J` with a correction bolted on afterwards.
            let w = win5(&pitches, t);
            let pen = |x: f64| opts.rough.local(&w, x);
            // A tick with no admissible move still has to advance its own prefix: the tick
            // before it may have moved this pass, which makes every state downstream stale, and
            // the rest of the sweep would then optimize against a trajectory nobody is flying.
            let Some((lo, hi)) = opts.rough.feasible(&w, lo, hi) else {
                for k in 0..states.len() { states[k][t + 1] = ticked(&states[k][t], pitches[t]) }
                continue
            };
            let (np, j, j_cur) = best_pitch_at(obj, &row, &pitches[t + 1..], cur,
                                               lo, hi, step, opts.ternary_iters, &pen);
            if global { worst_tick = worst_tick.max(j - j_cur) }
            pitches[t] = np;
            // every draw's prefix must stay consistent with the pitch just changed, or the
            // next tick's line search optimizes against a stale state and the schedule diverges
            for k in 0..states.len() {
                states[k][t + 1] = ticked(&states[k][t], pitches[t]);
            }
        }
        // The regularizer: if this pass pushed the schedule into chatter, throw the pass away
        // and stop. Discarding rather than merely stopping matters -- the crossing pass is the
        // one that did the damage, and keeping it would write out exactly what we are avoiding.
        // Only a pass that makes it *worse* stops the polish. A warm start can arrive already
        // below the floor -- `stretch` leaves a seam at the join, which reads as chatter -- and
        // an absolute test would then revert every pass and freeze the schedule, propagating an
        // unimproved copy down the whole continuation chain. Measured: three consecutive cells
        // with identical lag-1 and falling dy, each writing out its stretched parent untouched.
        let l1_now = lag1(&pitches);
        if l1_now < opts.lag1_floor && l1_now < lag1(&prev) {
            pitches = prev;
            stopped_degenerate = true;
            break;
        }
        passes = pass + 1;
        last_gain = obj.eval_jittered(&pitches, &dv) - opts.rough.cost(&pitches) - before;
        let _ = worst_tick;
        recent.push_back(last_gain);
        if recent.len() > opts.stall_window { recent.pop_front(); }
        // Stop when the last `stall_window` passes together earned less than `tol`. Judging on
        // one pass would stop in the lulls between the jumps that the global passes find.
        if recent.len() == opts.stall_window && recent.iter().sum::<f64>() < opts.tol { break }
    }

    let j = obj.eval(&pitches);
    let residual = certify_reg(obj, &pitches, opts.global_step, opts.jitter, opts.rough);
    let l1 = lag1(&pitches);
    let rough_cost = opts.rough.cost(&pitches);
    Polished { pitches, j, passes, last_gain, residual, lag1: l1, stopped_degenerate, rough_cost }
}

// ---------------------------------------------------------------- the profile file

/// A solved cell, as it lives on disk.
///
/// The invariant the format is built around: the header states everything that determines the
/// optimum -- objective, initial conditions, horizon, physics -- so the claim "these pitches
/// are optimal" is checkable by `certify` from the file alone. How the schedule was reached is
/// deliberately absent, because with warm-start continuation the initialization is a path
/// through the grid rather than a compact set of hyperparameters, and it does not matter.
#[derive(Clone, Debug)]
pub struct Profile {
    pub obj: Objective,
    pub trig: TrigMode,
    /// The jitter the schedule was optimized against. Part of the utility function, so a
    /// profile optimized at sigma = 1 is a different object from one optimized at sigma = 0,
    /// and `verify` re-certifies against whatever the header says.
    pub jitter: Jitter,
    /// The roughness price the schedule was optimized under. Part of the utility function in
    /// exactly the way `jitter` is: a profile written at `mu = 0.002` is a coordinate optimum
    /// of a different objective than one written at `mu = 0`, and `verify` has to know which.
    pub rough: Rough,
    pub commit: String,
    pub pitches: Vec<f64>,
    /// The residual a full global pass could still find, in blocks of `J`.
    pub residual: f64,
    pub passes: usize,
}

impl Profile {
    pub fn to_string(&self) -> String {
        let o = &self.obj;
        let st = o.replay(&self.pitches);
        let (s0, sn) = (&st[0], st.last().unwrap());
        let mut out = String::new();
        let mut w = |l: &str| { out.push_str(l); out.push('\n') };
        w("# elytrasim optimal pitch schedule");
        w("# objective   maximize J = TE(s_n) + w*z_n; TE in blocks (KE = |v|^2/(2g), PE = y); v_n free");
        w(&format!("# lambda      {}", self.obj.lambda));
        w(&format!("# w           {:.10}          # lambda * Y_REF/Z_REF, Y_REF = {Y_REF}, Z_REF = {Z_REF}", o.w()));
        w(&format!("# v0          {:.9} {:.9}    # vy vz", o.v0.y, o.v0.z));
        w(&format!("# n           {}", o.n));
        w(&format!("# trig        {}", self.trig));
        if self.jitter.is_on() {
            w(&format!("# jitter      {} {} {}    # v0 sigma in blocks/tick, draws, seed",
                       self.jitter.sigma, self.jitter.draws, self.jitter.seed));
        } else {
            w("# jitter      0                     # optimized from the exact starting velocity");
        }
        if self.rough.is_on() {
            w(&format!("# rough       {} {} {} {} {}    # mu (per deg/tick^2), mu_tv (per deg/tick), \
cap, slew_cap, |pitch| limit",
                       self.rough.mu, self.rough.mu_tv, self.rough.cap, self.rough.slew_cap,
                       self.rough.limit));
        } else {
            w("# rough       0 0 inf inf 90         # no price on hand movement, no pitch margin");
        }
        w(&format!("# commit      {}", self.commit));
        w(&format!("# dJ          {:.6}              # J(s_n) - J(s_0)", o.j(sn) - o.j(s0)));
        w(&format!("# dte         {:.6}              # TE(s_n) - TE(s_0), blocks", sn.total_energy() - s0.total_energy()));
        w(&format!("# dy          {:.6}", sn.pos.y - s0.pos.y));
        w(&format!("# dz          {:.6}", sn.pos.z - s0.pos.z));
        w(&format!("# v_end       {:.9} {:.9}", sn.vel.y, sn.vel.z));
        // Derived from the pitches, like dy and dz, and recorded for the same reason: the first
        // thing any analysis does is separate the pump cycles from the glides, and needing a
        // forward pass to do it is friction. See opt::shape.
        let sh = shape(&self.pitches);
        w(&format!("# structure   {}", sh.structure));
        w(&format!("# cycles      {}                     # dive-then-flick transitions; the \
corpus sweeps one cycle, so more than one is degenerate", sh.cycles));
        // How much hand movement the schedule asks for. Chatter shows up here and nowhere else
        // in the header: the reference cycle sits near 248 degrees, a hard-polished one at 3000+.
        w(&format!("# variation   {:.1}                 # summed |pitch change|, deg",
                   total_variation(&self.pitches)));
        w(&format!("# lag1        {:+.3}                # lag-1 correlation of the per-tick changes; below ~0.2 is chatter", lag1(&self.pitches)));
        // The curvature statistics: what the schedule asks of the wrist. `curv_l1` is what a
        // roughness price is levied on, `curv_max` is the peak angular acceleration -- 37.9 on
        // the reference cycle and 180, the geometric maximum, on a chattering one.
        w(&format!("# curv_l1     {:.1}                 # summed |second difference|, deg/tick^2",
                   curvature_l1(&self.pitches)));
        w(&format!("# curv_max    {:.1}                 # peak |second difference|, deg/tick^2",
                   curvature_max(&self.pitches)));
        w(&format!("# certified   full global pass at {:.2}deg, exact tail eval, improves J by {:.2e} ({} passes)",
                   PolishOpts::default().global_step, self.residual, self.passes));
        for p in &self.pitches { w(&format!("{p}")) }
        out
    }

    /// Read a profile back. The header is authoritative: a file replayed under the physics or
    /// weight it was *not* optimized for is not the same object, so `verify` reads the mode
    /// from the file rather than the command line.
    pub fn parse(text: &str) -> Result<Profile, String> {
        let field = |k: &str| -> Option<String> {
            text.lines()
                .find_map(|l| l.strip_prefix("# ")?.strip_prefix(k)
                    .map(|v| v.split('#').next().unwrap_or("").trim().to_string()))
        };
        let need = |k: &str| field(k).ok_or_else(|| format!("missing header field '{k}'"));
        let num = |k: &str| -> Result<f64, String> {
            need(k)?.parse().map_err(|e| format!("bad '{k}': {e}"))
        };
        let v0 = need("v0")?;
        let mut vs = v0.split_whitespace();
        let mut nextf = |what: &str| -> Result<f64, String> {
            vs.next().ok_or_else(|| format!("v0 needs two numbers ({what})"))?
              .parse().map_err(|e| format!("bad v0: {e}"))
        };
        let (vy, vz) = (nextf("vy")?, nextf("vz")?);
        let pitches = text.lines()
            .flat_map(|l| l.split('#').next().unwrap_or("").split_whitespace())
            .map(|s| s.parse::<f64>().map_err(|e| format!("bad pitch {s:?}: {e}")))
            .collect::<Result<Vec<f64>, _>>()?;
        let n = num("n")? as usize;
        if pitches.len() != n {
            return Err(format!("header says n = {n} but the file holds {} pitches", pitches.len()));
        }
        Ok(Profile {
            obj: Objective { v0: Vec3::new(0.0, vy, vz), n, lambda: num("lambda")? },
            trig: need("trig")?.parse()?,
            jitter: match field("jitter") {
                None => Jitter::default(),
                Some(v) => {
                    let f: Vec<&str> = v.split_whitespace().collect();
                    let g = |i: usize| f.get(i).and_then(|x| x.parse().ok());
                    Jitter {
                        sigma: g(0).unwrap_or(0.0),
                        draws: g(1).unwrap_or(1.0) as usize,
                        resample: Jitter::default().resample,
                        seed: f.get(2).and_then(|x| x.parse().ok())
                                .unwrap_or(Jitter::default().seed),
                    }
                }
            },
            rough: match field("rough") {
                None => Rough::default(),
                Some(v) => {
                    let f: Vec<&str> = v.split_whitespace().collect();
                    let g = |i: usize, d: f64| f.get(i).and_then(|x| x.parse().ok()).unwrap_or(d);
                    Rough { mu: g(0, 0.0), mu_tv: g(1, 0.0),
                            cap: g(2, f64::INFINITY), slew_cap: g(3, f64::INFINITY),
                            limit: g(4, 90.0) }
                }
            },
            commit: field("commit").unwrap_or_default(),
            pitches,
            residual: field("certified").and_then(|s| s.split("improves J by ").nth(1)
                .and_then(|r| r.split_whitespace().next()?.parse().ok())).unwrap_or(f64::NAN),
            passes: 0,
        })
    }
}

/// The git commit the binary was built from, for the header. Resolved at build time by
/// build.rs; "unknown" if the build had no git.
pub fn commit_hash() -> &'static str { env!("ELYTRASIM_COMMIT") }

// ---------------------------------------------------------------- seeding a cell

/// The best-scoring constants for each dive rule, as tuned by `myopic policy opt <rule>`
/// against the 300-tick limit cycle. `Leak` is the strongest of the four (95.9% of the optimal
/// cycle) and so is the default seed.
///
/// Note what these are and are not. They fly a *limit cycle*: the phase switches are on state,
/// not on the clock, so the policy is horizon-independent -- but that is not the same as being
/// horizon-*appropriate*. At `n = 100` the horizon is shorter than one cycle, so the optimum is
/// not a truncated cycle at all and there is no reason these thresholds should seed it well.
/// Continuation down the `n` axis is the answer; `sweep pilot` measures whether it is needed.
pub fn tuned_policy(dive: Dive, n_gain: usize) -> P {
    match dive {
        Dive::Leak => P { g_star: 17.73, k: 0.055, s_switch: 2.40, vy_flick: -0.260,
                          s_exit: 0.21, slew: 12.7, p_push: 23.0, p_flick: -88.5, n_gain, dive },
        Dive::Floor => P { g_star: f64::NAN, k: f64::NAN, s_switch: 2.177, vy_flick: -0.2685,
                           s_exit: 0.45, slew: 12.92, p_push: 24.01, p_flick: -79.44, n_gain, dive },
        Dive::Hold => P { g_star: f64::NAN, k: f64::NAN, s_switch: 2.127, vy_flick: -0.260,
                          s_exit: 0.29, slew: 8.34, p_push: 47.0, p_flick: -41.51, n_gain, dive },
        Dive::Target => P { g_star: f64::NAN, k: f64::NAN, s_switch: 2.177, vy_flick: -0.2685,
                            s_exit: 0.45, slew: 12.92, p_push: 24.01, p_flick: -79.44, n_gain, dive },
    }
}

/// A cold start for a cell: fly the tuned policy from this cell's `v0` for `n` ticks.
pub fn seed_from_policy(obj: &Objective) -> Vec<f64> {
    fly_from(obj.v0, tuned_policy(Dive::Leak, 20), obj.n, &[]).0
}

/// A cold start from the reference cycle, tiled and cut to length. The cycle is ~300 ticks, so
/// for `n > 300` this repeats it; phases have absolute durations, not fractional ones, so the
/// schedule is extended rather than rescaled.
pub fn seed_from_reference(n: usize) -> Vec<f64> {
    let r = crate::replay_pitches::REPLAY_PITCHES_300;
    (0..n).map(|i| r[i % r.len()] as f64).collect()
}

/// Resample a schedule to length `n`, linearly: a cycle of period 300 becomes a cycle of
/// period `n`.
///
/// The third seed for continuation, and the one that covers the gap tiling leaves. Tiling only
/// lands well at integer multiples of the cycle -- at n = 400 a tiled 300-tick cycle ends 100
/// ticks into the next one, mid-dive, which is where the energy goes. Rescaling instead asks
/// for a single cycle that happens to take n ticks.
///
/// Phases do have absolute durations, so this is not physically exact for large ratios; it is a
/// seed, and it competes with the others on measured `J` rather than on principle.
pub fn rescale(pitches: &[f64], n: usize) -> Vec<f64> {
    if pitches.is_empty() { return vec![0.0; n] }
    if pitches.len() == 1 { return vec![pitches[0]; n] }
    let m = pitches.len();
    (0..n).map(|i| {
        let x = i as f64 * (m - 1) as f64 / (n.max(2) - 1) as f64;
        let (lo, f) = (x.floor() as usize, x - x.floor());
        let hi = (lo + 1).min(m - 1);
        ((1.0 - f) * pitches[lo] + f * pitches[hi]) as f32 as f64
    }).collect()
}

/// Adapt a solved neighbor's schedule to a different horizon, for continuation along `n`.
///
/// This is `rescale`: a cycle of period `m` becomes a cycle of period `n`. The corpus sweeps
/// **one** cycle across `num_ticks`, so a 500-tick cell must be a single cycle that takes 500
/// ticks -- not the 300-tick cycle flown once and two-thirds, and not two 250s, which is
/// something you can tile post hoc from two cells of this corpus if you want it.
///
/// Tiling was tried and is wrong, however good it looks. Seeding n = 600 with the 300-tick
/// cycle twice scores dJ 41.08 against 19.47 at n = 300 -- but that is a two-cycle schedule,
/// degenerate here in the same way the constant-pitch-up glide is: a different regime that
/// wins on J while not being the object under study. Total energy *should* fall away from
/// n = 300, because 300 is the optimal length for a single cycle.
///
/// Phases have absolute durations, so a uniform time rescale is not physically exact for large
/// ratios. It is a seed, and the polish moves it; what matters is that it stays in the
/// single-cycle family.
pub fn stretch(pitches: &[f64], n: usize) -> Vec<f64> {
    if pitches.len() == n { return pitches.to_vec() }
    rescale(pitches, n)
}

// ---------------------------------------------------------------- determinism

/// Local averaging over `k` ticks: the projection a chattering schedule needs before it can be
/// polished under a roughness price.
///
/// A chattering control is the discrete stand-in for a *relaxed* control -- at each tick the
/// optimizer is really choosing a distribution over pitches, and it realizes the mixture by
/// alternating. The ordinary control that means the same thing is the local mean, which is what
/// this computes. Without it, coordinate ascent under an l1 curvature price can stall: the price
/// couples three neighboring coordinates, so it is a fused-l1 term, and coordinate descent is
/// not guaranteed to reach a stationary point of a nonsmooth objective that couples coordinates.
/// Measured: seeded from a schedule alternating -90/+90 it converged (residual 2.2e-7) to
/// `curv_l1` 1295, against 162 from the same price seeded from the local mean.
///
/// `k <= 1` is the identity. Ends are handled by clamping the window, so a ramp stays a ramp.
///
/// The width matters and is not free: a box filter smears the snap and the flick along with the
/// chatter, and from a destroyed cycle the price-constrained polish cannot rebuild one. Sweep it.
/// Projecting one relaxed optimum (J 22.346) at k = 3, 5, 9, 15, 25 gives 22.165, 22.161, 22.156,
/// 22.130, 22.047 -- all fine; projecting a *more violently* chattering one (J 22.402) at k = 9
/// lands at 15.66, converged (residual 8.9e-6) and 6.5 blocks worse. So the projection width is a
/// hyperparameter to sweep and score on the regularized objective, not a constant to pick once.
pub fn smooth_box(p: &[f64], k: usize) -> Vec<f64> {
    if k <= 1 { return p.to_vec() }
    let n = p.len() as isize;
    let h = (k / 2) as isize;
    (0..n).map(|i| {
        let (mut s, mut c) = (0.0, 0.0);
        for d in -h..=h { s += p[(i + d).clamp(0, n - 1) as usize]; c += 1.0 }
        (s / c) as f32 as f64
    }).collect()
}

/// Median over a window of `k` ticks. Available, and **not** the right projection here -- kept
/// because the reason is worth writing down.
///
/// A median filter looks like the obvious choice: it passes a step through untouched while
/// removing short runs, which is exactly the shape of "a piecewise-smooth control with a few
/// genuine corners plus noise on top". But it removes runs shorter than *half the window*, and a
/// tick-rate alternation has runs of length one with fifty percent duty -- so every window holds
/// a majority of whichever value sits at its center, and the median reproduces the alternation
/// instead of removing it. Chatter is not impulsive noise; it is a square wave at Nyquist.
///
/// Measured, projecting the same relaxed optimum and then polishing at `mu = 1e-3`: median 3, 5,
/// 7, 9 give J 15.32, 15.69, 15.92, 16.01, against 22.16 for the local mean. Only at width 13,
/// where the chatter is irregular enough for the majority to break, does it recover (22.14).
///
/// `k` is forced odd and `k <= 1` is the identity.
pub fn smooth_median(p: &[f64], k: usize) -> Vec<f64> {
    if k <= 1 { return p.to_vec() }
    let k = k | 1;
    let n = p.len() as isize;
    let h = (k / 2) as isize;
    (0..n).map(|i| {
        let mut w: Vec<f64> = (-h..=h).map(|d| p[(i + d).clamp(0, n - 1) as usize]).collect();
        w.sort_by(f64::total_cmp);
        w[w.len() / 2]
    }).collect()
}

/// Bring a schedule inside the admissible set -- `limit`, `slew_cap` and `cap` -- by integrating
/// a rate-limited tracker through it from the left.
///
/// A coordinate sweep cannot do this on its own. `Rough::feasible` asks what one pitch may be
/// given its neighbors, and when the neighbors are themselves out of bounds the answer is
/// nothing at all -- every tick returns `None` and the polish is a no-op. So a seed that breaks
/// the cap has to be projected before the sweep starts, exactly as one that breaks `limit` is.
///
/// The core of it is the causal projection: walk forward, and wherever the schedule asks for
/// more than `cap` of angular acceleration, give it `cap`. That is what a hand with a bounded
/// acceleration would produce trying to track the target.
///
/// On its own it is not enough, and the failure is immediate rather than exotic. A tracker
/// following a violent seed accumulates rate, runs into `+-limit`, and the clamp *is* a large
/// second difference -- so the pass ends with a violation where it hit the wall. Alternating
/// -60/+60 with `cap = 25` and `limit = 90` comes out with a worst of 90, unchanged in kind from
/// the 240 it started with.
///
/// So: pre-smooth first, widening the window until the causal pass comes out clean. This
/// terminates, because at a window as wide as the schedule the mean is a constant and every
/// second difference is zero. The width it settles on is a fair measure of how far outside the
/// admissible set the seed was.
pub fn project_cap(p: &[f64], rough: Rough) -> Vec<f64> {
    // Quantize as we go. The projection's output is a control that will be flown, so it has to be
    // representable *and* inside the set -- clamping in f64 and rounding afterwards can land back
    // outside, which is the whole point of `f32_inside`.
    let q32 = |x: f64| f32_inside(x, -rough.limit, rough.limit).unwrap_or(0.0);
    let causal = |src: &[f64]| -> Vec<f64> {
        let mut q: Vec<f64> = src.iter().map(|&x| q32(x)).collect();
        // Rate first, then acceleration: a schedule inside the slew limit is a milder input to
        // the curvature pass, and the curvature pass cannot make the rate worse than 2*cap.
        if rough.slew_cap.is_finite() {
            for j in 0..q.len().saturating_sub(1) {
                let d = q[j + 1] - q[j];
                if d.abs() > rough.slew_cap {
                    q[j + 1] = q32(q[j] + d.clamp(-rough.slew_cap, rough.slew_cap));
                }
            }
        }
        if rough.cap.is_finite() {
            for j in 0..q.len().saturating_sub(2) {
                let d = q[j + 2] - 2.0 * q[j + 1] + q[j];
                if d.abs() > rough.cap {
                    q[j + 2] = q32(2.0 * q[j + 1] - q[j] + d.clamp(-rough.cap, rough.cap));
                }
            }
        }
        q
    };
    if !rough.cap.is_finite() && !rough.slew_cap.is_finite() { return causal(p) }   // already q32
    // `violation` is the termination test, so this cannot claim success on a constraint it
    // forgot to enforce -- which is exactly how the first version of this function was wrong.
    let mut k = 1;
    while k <= p.len() + 1 {
        let q = causal(&smooth_box(p, k));
        if rough.violation(&q) == 0.0 { return q }
        k += 2;
    }
    // A wide `smooth_box` is *not* a constant: it clamps at the ends rather than wrapping, so it
    // keeps a slope, and the widening loop can run out while still failing the cap. Fall back to
    // the flattest thing there is, whose differences are all exactly zero. It throws the schedule
    // away, which is why it is a fallback and not the method.
    let m = p.iter().sum::<f64>() / p.len().max(1) as f64;
    let m = f32_inside(m, -rough.limit, rough.limit).unwrap_or(0.0);
    vec![m; p.len()]
}

/// FNV-1a over the raw bits of a canonical replay, for checking that two machines agree.
///
/// The `Mth` table removes the f32 trig divergence between platforms, but `lift_force` still
/// goes through f64 `cos` in `entity.rs` -- faithfully, because vanilla's `liftForce` really is
/// `Math.cos` on a double -- so bit-identity across libms is *not* guaranteed. This makes any
/// drift a number that shows up in a diff rather than a silent change in the corpus.
pub fn physics_fingerprint() -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut feed = |b: u64| { h ^= b; h = h.wrapping_mul(0x100000001b3) };
    let mut v = Vec3::new(0.0, 0.167467, 0.200887);
    for i in 0..20_000i32 {
        // sweep the whole pitch range, and restart when the state runs away so the walk stays
        // in the region the sweep actually visits
        let p = ((i % 361) - 180) as f64 * 0.5;
        v = update_fall_flying_movement(v, rot(p));
        if !v.y.is_finite() || v.y.abs() > 1e6 { v = Vec3::new(0.0, 0.167467, 0.200887) }
        feed(v.y.to_bits());
        feed(v.z.to_bits());
    }
    h
}

// ---------------------------------------------------------------- structure

/// Whether a schedule still flies the pump cycle, or has fallen into the glide.
///
/// This is *not* the same question as whether it gains energy, and conflating the two is a
/// mistake: at short horizons a genuine cycle can lose height and still be a cycle. A 120-tick
/// optimum loses 10.5 blocks while showing the full dive / snap / flick / gain structure, and
/// is not collapsed. What collapse means is that the schedule stops pumping and just holds a
/// glide -- pitch stays in a narrow band a few degrees below level, never noses down to build
/// speed and never flicks up to cash it in.
///
/// The two regimes are separated by a mile, so the thresholds are not delicate: measured
/// against known-collapsed and known-uncollapsed 110-, 115- and 120-tick optima, the cyclic
/// ones span roughly [-55, +90] degrees and the collapsed ones [-15.3, +0.10].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Structure {
    /// Dives to build speed, snaps flat, flicks up, and eases down through the gain. One cycle.
    Cyclic,
    /// Holds a glide. No nose-down dive and no flick.
    Collapsed,
    /// Two or more cycles inside the horizon.
    ///
    /// Degenerate for this corpus in the same way the collapsed glide is: a different regime
    /// that happens to score better, not the object being swept. 300 ticks is the optimal
    /// length for *one* cycle, so total energy falling away from 300 is the answer, not a
    /// defect -- and a 500-tick cell must be a single cycle that takes 500 ticks. Two 250s
    /// tiled is something you can build post hoc from two cells of this corpus.
    MultiCycle,
}

/// A schedule noses down past this to build speed. The collapsed glide never exceeds +0.11.
const DIVE_PITCH: f64 = 20.0;
/// And flicks up past this to convert it. The collapsed glide bottoms out near -15.3.
const FLICK_PITCH: f64 = -30.0;

#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub structure: Structure,
    pub pitch_min: f64,
    pub pitch_max: f64,
    /// Ticks spent within 1 degree of level after the dive -- the snap.
    pub flat_ticks: usize,
    /// Dive-then-flick transitions. One is the object being swept; more is degenerate.
    pub cycles: usize,
}

/// Total variation of the schedule: summed absolute pitch change, in degrees. How much hand
/// movement the whole flight asks for, and the direct measure of chatter -- a hard-polished
/// 300-tick schedule reaches 3243 degrees against the reference cycle's 35.
pub fn total_variation(pitches: &[f64]) -> f64 {
    pitches.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
}

/// Lag-1 correlation of the per-tick pitch changes: the degeneracy statistic.
///
/// Total variation cannot tell a flick from chatter -- both are large moves. What separates
/// them is whether neighboring ticks move *together*. A real manoeuvre is a run of same-signed
/// steps, so the deltas correlate positively; chatter alternates, so they correlate negatively.
/// Measured on the reference cycle this reads +0.48, on the same schedule polished 200 passes
/// -0.74, and the sign flip is the whole diagnosis.
pub fn lag1(pitches: &[f64]) -> f64 {
    if pitches.len() < 3 { return 0.0 }
    let d: Vec<f64> = pitches.windows(2).map(|w| w[1] - w[0]).collect();
    let mean = d.iter().sum::<f64>() / d.len() as f64;
    let den: f64 = d.iter().map(|x| (x - mean).powi(2)).sum();
    if den <= 0.0 { return 1.0 }        // constant or perfectly linear: maximally smooth
    d.windows(2).map(|w| (w[0] - mean) * (w[1] - mean)).sum::<f64>() / den
}

/// How many dive-then-flick transitions the schedule makes: the number of cycles.
///
/// Scanned with hysteresis, so the chatter in a degenerate schedule -- which crosses both
/// thresholds repeatedly within a few ticks -- does not read as extra cycles. A cycle is
/// counted only when the schedule has been committed to the dive (above `DIVE_PITCH`) and then
/// commits to the climb (below `FLICK_PITCH`); it must return to the dive side before another
/// can be counted.
pub fn cycles(pitches: &[f64]) -> usize {
    let (mut n, mut armed) = (0usize, false);
    for &p in pitches {
        if p > DIVE_PITCH { armed = true }
        else if armed && p < FLICK_PITCH { n += 1; armed = false }
    }
    n
}

pub fn shape(pitches: &[f64]) -> Shape {
    let pitch_max = pitches.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let pitch_min = pitches.iter().cloned().fold(f64::INFINITY, f64::min);
    let flat_ticks = pitches.iter().filter(|p| p.abs() <= 1.0).count();
    let n_cycles = cycles(pitches);
    let structure = if !(pitch_max > DIVE_PITCH && pitch_min < FLICK_PITCH) {
        Structure::Collapsed
    } else if n_cycles >= 2 {
        Structure::MultiCycle
    } else {
        Structure::Cyclic
    };
    Shape { structure, pitch_min, pitch_max, flat_ticks, cycles: n_cycles }
}

impl std::fmt::Display for Structure {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Structure::Cyclic => "cyclic",
            Structure::Collapsed => "COLLAPSED",
            Structure::MultiCycle => "MULTICYCLE",
        })
    }
}
