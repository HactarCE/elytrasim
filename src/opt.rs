//! The shared core: physics helpers, the myopic rules, the policy that flies them,
//! and the search primitives the optimizer and the sweep both stand on.
//!
//! Yaw is pinned to zero everywhere here, so the whole problem lives in the `(v_y, v_z)`
//! plane and a schedule is just a list of pitches in degrees. `myopic` analyzes one cycle
//! with these; `sweep` solves a grid of them.

use crate::sim::*;

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
