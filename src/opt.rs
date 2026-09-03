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
    /// Stop when a pass gains less than this in `J` (blocks).
    pub tol: f64,
}

impl Default for PolishOpts {
    fn default() -> Self {
        PolishOpts {
            max_passes: 200, global_every: 4, global_step: 0.25,
            local_span: 8.0, local_step: 0.05, ternary_iters: 70, tol: 1e-9,
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
}

/// Best pitch for tick `t`, holding every other tick fixed: a global sweep, then a ternary
/// refine inside the winning cell, scored on the exact tail. Returns `(pitch, J)` with the
/// pitch already rounded to `f32`, because the sim casts pitch to `f32` anyway -- so the value
/// returned is the one that will actually be flown, and its score is the score of flying it.
fn best_pitch_at(obj: &Objective, s: &State, tail: &[f64], cur: f64, lo: f64, hi: f64,
                 step: f64, ternary_iters: usize) -> (f64, f64) {
    let score = |p: f64| -> f64 {
        let mut st = ticked(s, p);
        for &q in tail { st = ticked(&st, q) }
        obj.j(&st)
    };
    let steps = ((hi - lo) / step).round() as i64;
    // The tail replays are independent, so the sweep is exactly parallel -- no approximation,
    // just the same evaluations on more cores.
    let (mut bp, mut bs) = (cur, score(cur));
    let (gp, gs) = (0..steps + 1)
        .into_par_iter()
        .map(|i| { let p = lo + step * i as f64; (p, score(p)) })
        .reduce(|| (cur, f64::NEG_INFINITY), |a, b| if b.1 > a.1 { b } else { a });
    if gs > bs { bp = gp; bs = gs }

    let (mut a, mut b) = ((bp - step).max(-90.0), (bp + step).min(90.0));
    for _ in 0..ternary_iters {
        let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
        if score(m1) < score(m2) { a = m1 } else { b = m2 }
    }
    // Round first, then score: the schedule that gets written must be the schedule that was
    // measured, or a cell can be certified on a pitch it does not contain.
    let refined = (0.5 * (a + b)) as f32 as f64;
    let rs = score(refined);
    let bp32 = bp as f32 as f64;
    let bs32 = if bp32 == bp { bs } else { score(bp32) };
    if rs > bs32 { (refined, rs) } else { (bp32, bs32) }
}

/// One full global pass that changes nothing, reporting the largest gain it could have made.
///
/// This is the profile's certificate. It re-derives the claim from `(v0, n, lambda, pitches)`
/// alone, so it can be run by anyone holding the file and says nothing about how the schedule
/// was found. A residual at or below the writer's tolerance means the schedule is a coordinate
/// optimum to that tolerance.
pub fn certify(obj: &Objective, pitches: &[f64], step: f64) -> f64 {
    let states = obj.replay(pitches);
    (0..pitches.len())
        .map(|t| {
            let base = obj.eval(pitches);
            let (_, j) = best_pitch_at(obj, &states[t], &pitches[t + 1..], pitches[t],
                                       -90.0, 90.0, step, 70);
            j - base
        })
        .fold(0.0, f64::max)
}

/// Coordinate ascent over the schedule: sweep `t = 0..n`, replacing each pitch with the best
/// one given the rest. Non-unimodality in pitch is why the sweep has to be global, and the
/// corner at pitch 0 -- the forward-to-up conversion is gated on `lean_angle < 0` -- is why it
/// cannot be replaced by a derivative method.
pub fn polish(obj: &Objective, init: &[f64], opts: PolishOpts) -> Polished {
    assert_eq!(init.len(), obj.n, "schedule length must match the objective's horizon");
    let mut pitches: Vec<f64> = init.iter().map(|&p| p as f32 as f64).collect();
    let mut states = obj.replay(&pitches);
    let (mut passes, mut last_gain) = (0, f64::INFINITY);

    for pass in 0..opts.max_passes {
        let before = obj.eval(&pitches);
        for t in 0..obj.n {
            let cur = pitches[t];
            let (lo, hi, step) = if pass % opts.global_every == 0 {
                (-90.0, 90.0, opts.global_step)
            } else {
                ((cur - opts.local_span).max(-90.0), (cur + opts.local_span).min(90.0), opts.local_step)
            };
            let s = states[t].clone();
            let (np, _) = best_pitch_at(obj, &s, &pitches[t + 1..], cur, lo, hi, step,
                                        opts.ternary_iters);
            pitches[t] = np;
            // the prefix must stay consistent with the pitches just changed, or the next
            // tick's line search optimizes against a stale state and the schedule diverges
            states[t + 1] = ticked(&states[t], pitches[t]);
        }
        passes = pass + 1;
        last_gain = obj.eval(&pitches) - before;
        if last_gain < opts.tol { break }
    }

    let j = obj.eval(&pitches);
    let residual = certify(obj, &pitches, opts.global_step);
    Polished { pitches, j, passes, last_gain, residual }
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
        w(&format!("# commit      {}", self.commit));
        w(&format!("# dJ          {:.6}              # J(s_n) - J(s_0)", o.j(sn) - o.j(s0)));
        w(&format!("# dte         {:.6}              # TE(s_n) - TE(s_0), blocks", sn.total_energy() - s0.total_energy()));
        w(&format!("# dy          {:.6}", sn.pos.y - s0.pos.y));
        w(&format!("# dz          {:.6}", sn.pos.z - s0.pos.z));
        w(&format!("# v_end       {:.9} {:.9}", sn.vel.y, sn.vel.z));
        // Derived from the pitches, like dy and dz, and recorded for the same reason: the first
        // thing any analysis does is separate the pump cycles from the glides, and needing a
        // forward pass to do it is friction. See opt::shape.
        w(&format!("# structure   {}", shape(&self.pitches).structure));
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

/// Adapt a solved neighbor's schedule to a different horizon, for continuation along `n`.
/// Trimming takes the head; extending repeats the reference cycle's tail, which is the dive
/// the schedule would be entering anyway.
pub fn stretch(pitches: &[f64], n: usize) -> Vec<f64> {
    if pitches.len() >= n { return pitches[..n].to_vec() }
    let mut out = pitches.to_vec();
    let r = crate::replay_pitches::REPLAY_PITCHES_300;
    while out.len() < n { out.push(r[out.len() % r.len()] as f64) }
    out
}

// ---------------------------------------------------------------- determinism

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
    /// Dives to build speed, snaps flat, flicks up, and eases down through the gain.
    Cyclic,
    /// Holds a glide. No nose-down dive and no flick.
    Collapsed,
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
}

pub fn shape(pitches: &[f64]) -> Shape {
    let pitch_max = pitches.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let pitch_min = pitches.iter().cloned().fold(f64::INFINITY, f64::min);
    let flat_ticks = pitches.iter().filter(|p| p.abs() <= 1.0).count();
    let structure = if pitch_max > DIVE_PITCH && pitch_min < FLICK_PITCH {
        Structure::Cyclic
    } else {
        Structure::Collapsed
    };
    Shape { structure, pitch_min, pitch_max, flat_ticks }
}

impl std::fmt::Display for Structure {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self { Structure::Cyclic => "cyclic", Structure::Collapsed => "COLLAPSED" })
    }
}
