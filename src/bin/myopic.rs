//! Which *myopic* metrics does the globally optimal climb cycle agree with, phase by phase?
//!
//! The optimal cycle splits into four phases, and each one turns out to follow a rule that
//! needs only the current velocity:
//!
//! | phase | ticks | rule                                              |
//! |-------|-------|---------------------------------------------------|
//! | dive  | ~190  | hold the flight-path angle (leaking toward ~16.6°) |
//! | snap  | ~14   | pitch 0                                           |
//! | flick | ~6    | ramp to about -88°; the values do not matter      |
//! | gain  | ~86   | argmax over pitch of delta TE over ~20 ticks      |
//!
//! Everything here is measured against `sim`'s physics with yaw pinned to zero, so the whole
//! problem lives in the (v_y, v_z) plane. See README-myopic.md for the numbers.
//!
//! Subcommands:
//!   profiles                             cycle closure and climb rate of the built-in profiles
//!   eq                                   terminal-glide table, and the fastest steady glide
//!   polish   <file> [passes] [w]         coordinate-ascent polish of a schedule; maximises TE + w*z
//!   cycle    <file> <off>                per-tick dump: pitch, gamma, and each rule's answer
//!   score    <file> <off> <lo> <hi>      RMS pitch error of a menu of rules, per phase
//!   probe    <file> <off> <lo> <hi>      implied lookahead n*(t) through the gain phase
//!   family   <file> <tag>                auto-detect the phases and summarise the fit
//!   policy   [opt]                       fly the four bugs; NGAIN=<n> sets the gain lookahead
//!
//! A schedule file is whitespace-separated pitches in degrees. `<off>` is the tick offset of
//! the cycle to read, so a 3x-tiled 900-tick flight is read horizon-free at offset 300.

use elytrasim::sim::*;

pub const V0: Vec3 = Vec3::new(0.0, 0.167467, 0.200887);
const OPTIMAL_CYCLE_RATE: f64 = 1.43335; // blocks/second, REPLAY_PITCHES_300

fn rot(p: f64) -> Rot { Rot { x: p as f32, y: 0.0 } }
fn gamma(v: Vec3) -> f64 { (-v.y).atan2(v.z).to_degrees() }

fn ticked(s: &State, p: f64) -> State { s.ticked(rot(p)) }
fn run_n(s: &State, p: f64, n: usize) -> State {
    let r = rot(p);
    let mut s = s.clone();
    for _ in 0..n { s = s.ticked(r) }
    s
}
fn replay(pitches: &[f64]) -> Vec<State> {
    let mut v = vec![State { pos: Vec3::ZERO, vel: V0 }];
    for &p in pitches { let s = ticked(v.last().unwrap(), p); v.push(s) }
    v
}
fn read_pitches(path: &str) -> Vec<f64> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect()
}

/// Coarse sweep for the global argmax, then a ternary refine inside the winning cell.
/// The objective is not unimodal in pitch, so the sweep has to be global.
fn argmax<F: Fn(f64) -> f64>(f: F, step: f64) -> f64 {
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

/// GAIN. Pitch maximising the total-energy change over `n` ticks held constant.
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

/// Terminal glide for a constant pitch: iterate the velocity map to its fixed point.
pub fn equilibrium(p: f64) -> Vec3 {
    let mut v = Vec3::new(0.0, -0.5, 1.0);
    for _ in 0..40000 { v = update_fall_flying_movement(v, rot(p)) }
    v
}

// ---------------------------------------------------------------- subcommands

fn cmd_profiles() {
    use elytrasim::replay_pitches as rp;
    let named: [(&str, &[f32]); 5] = [
        ("FORTY_FORTY", rp::FORTY_FORTY), ("FOUR_LINES_300", rp::FOUR_LINES_300),
        ("REPLAY_PITCHES_200", rp::REPLAY_PITCHES_200), ("REPLAY_PITCHES_300", rp::REPLAY_PITCHES_300),
        ("REPLAY_PITCHES_400", rp::REPLAY_PITCHES_400),
    ];
    println!("{:<20} {:>6} {:>10} {:>9} {:>9} {:>12}", "profile", "ticks", "|v_N - v_0|", "climb b/s", "dist b/s", "closes?");
    for (name, ps) in named {
        let ps: Vec<f64> = ps.iter().map(|&p| p as f64).collect();
        if ps.is_empty() { continue }
        let st = replay(&ps);
        let end = st.last().unwrap();
        let dv = (end.vel - V0).length();
        let n = ps.len() as f64;
        println!("{name:<20} {:>6} {dv:>10.2e} {:>9.4} {:>9.3} {:>12}",
                 ps.len(), end.pos.y / n * 20.0, end.pos.z / n * 20.0,
                 if dv < 1e-5 { "yes" } else { "no" });
    }
}

fn cmd_eq() {
    let (mut bz, mut bzp) = (f64::NEG_INFINITY, 0.0);
    for i in 0..=1800 {
        let p = 0.05 * i as f64;
        let e = equilibrium(p);
        if e.z > bz { bz = e.z; bzp = p }
    }
    let e = equilibrium(bzp);
    println!("fastest steady glide: pitch {bzp:.3}  v_z {bz:.5}  v_y {:.5}  gamma {:.4}", e.y, gamma(e));
    println!("  -- this angle is the floor the dive's flight-path angle settles onto.\n");
    println!("{:>6} {:>10} {:>10} {:>9} {:>8}", "pitch", "eq v_y", "eq v_z", "|eq|", "gamma");
    for p in [0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 53.35, 60.0, 75.0, 90.0] {
        let e = equilibrium(p);
        println!("{p:>6.2} {:>10.5} {:>10.5} {:>9.5} {:>8.4}", e.y, e.z, e.length(), gamma(e));
    }
}

fn cmd_polish(path: &str, passes: usize, w: f64) {
    let mut pitches = read_pitches(path);
    let n = pitches.len();
    let obj = |s: &State| s.total_energy() + w * s.pos.z;
    let tail = |s: &State, ps: &[f64], i: usize| {
        let mut s = s.clone();
        for k in i..ps.len() { s = ticked(&s, ps[k]) }
        obj(&s)
    };
    eprintln!("start: J = {:.6}  y = {:.4}  (w = {w})", tail(&replay(&pitches)[n], &pitches, n), replay(&pitches)[n].pos.y);
    for pass in 0..passes {
        let mut states = replay(&pitches);
        let mut improved = 0.0;
        for t in 0..n {
            let s = states[t].clone();
            let f = |p: f64| tail(&ticked(&s, p), &pitches, t + 1);
            let cur = pitches[t];
            // every fourth pass sweeps the whole range, in case a tick is in the wrong basin
            let (lo, hi, step) = if pass % 4 == 0 { (-90.0, 90.0, 0.25) }
                                 else { ((cur - 8.0).max(-90.0), (cur + 8.0).min(90.0), 0.05) };
            let (mut bp, mut bs) = (cur, f(cur));
            let steps = ((hi - lo) / step).round() as i64;
            for i in 0..=steps { let p = lo + step * i as f64; let v = f(p); if v > bs { bs = v; bp = p } }
            let (mut a, mut b) = ((bp - step).max(-90.0), (bp + step).min(90.0));
            for _ in 0..70 { let (m1, m2) = (a+(b-a)/3.0, b-(b-a)/3.0); if f(m1) < f(m2) { a = m1 } else { b = m2 } }
            let pr = 0.5 * (a + b);
            let (np, ns) = if f(pr) > bs { (pr, f(pr)) } else { (bp, bs) };
            let np = np as f32 as f64;                       // the sim casts pitch to f32 anyway
            let base = f(cur);
            if ns > base { improved += ns - base; pitches[t] = np }
            // the prefix must stay consistent with the pitches just changed, or the next
            // tick's line search optimises against a stale state and the schedule diverges
            states[t + 1] = ticked(&states[t], pitches[t]);
        }
        let st = replay(&pitches);
        if pass % 5 == 4 || improved < 1e-12 {
            eprintln!("pass {pass:>3}: J = {:.6}  y = {:.4}  z = {:.2}  |v_end| = {:.5}  (+{improved:.2e})",
                      tail(&st[n], &pitches, n), st[n].pos.y, st[n].pos.z, st[n].vel.length());
        }
        if improved < 1e-12 { break }
    }
    for p in &pitches { println!("{p}") }
}

fn cmd_cycle(path: &str, off: usize) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    println!("rel,pitch,vy,vz,speed,gamma,te,dte,hold_gamma,dte1,dte8,dte20");
    for rel in 0..300.min(ps.len() - off) {
        let t = off + rel;
        let s = &st[t];
        println!("{rel},{:.4},{:.6},{:.6},{:.6},{:.4},{:.5},{:.6},{:.4},{:.4},{:.4},{:.4}",
                 ps[t], s.vel.y, s.vel.z, s.vel.length(), gamma(s.vel), s.total_energy(),
                 st[t + 1].total_energy() - s.total_energy(),
                 bug_gamma_to(s, gamma(s.vel)), bug_dte_n(s, 1), bug_dte_n(s, 8), bug_dte_n(s, 20));
    }
}

fn cmd_score(path: &str, off: usize, lo: usize, hi: usize) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    let rms = |f: &dyn Fn(&State) -> f64| {
        let (mut acc, mut n) = (0.0, 0);
        for rel in lo..hi { let d = f(&st[off + rel]) - ps[off + rel]; acc += d * d; n += 1 }
        (acc / n as f64).sqrt()
    };
    let med = |f: &dyn Fn(&State) -> f64| {
        let mut v: Vec<f64> = (lo..hi).map(|rel| (f(&st[off + rel]) - ps[off + rel]).abs()).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    println!("ticks {lo}..{hi} of the cycle at offset {off}");
    println!("{:<22} {:>8} {:>8}", "rule", "RMS", "median");
    let row = |name: String, f: &dyn Fn(&State) -> f64| println!("{name:<22} {:>8.2} {:>8.2}", rms(f), med(f));
    for n in [1usize, 2, 4, 8, 12, 16, 20, 24, 32, 48] {
        row(format!("dTE / {n} ticks"), &|s| bug_dte_n(s, n));
    }
    row("hold gamma".into(), &|s| bug_gamma_to(s, gamma(s.vel)));
    row("gamma -> 16.577".into(), &|s| bug_gamma_to(s, 16.577));
    row("max next speed".into(), &|s| argmax(|p| ticked(s, p).vel.length(), 0.125));
    row("min next |v_y|".into(), &|s| argmax(|p| -ticked(s, p).vel.y.abs(), 0.125));
}

fn cmd_probe(path: &str, off: usize, lo: usize, hi: usize) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    println!("rel,pitch,n_star,err_dte1,err_dte20,err_dte48,gamma");
    for rel in lo..hi {
        let s = &st[off + rel];
        let opt = ps[off + rel];
        let (mut bn, mut bd) = (0usize, f64::INFINITY);
        for n in 1..=90 { let d = (bug_dte_n(s, n) - opt).abs(); if d < bd { bd = d; bn = n } }
        println!("{rel},{opt:.3},{bn},{:.3},{:.3},{:.3},{:.2}",
                 bug_dte_n(s, 1) - opt, bug_dte_n(s, 20) - opt, bug_dte_n(s, 48) - opt, gamma(s.vel));
    }
}

/// Find the cycle in a schedule without being told where it is, then report how well the
/// dive and gain rules fit it. Used to check the rules across a family of optimal cycles.
fn cmd_family(path: &str, tag: &str) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    let apex: Vec<usize> = (1..ps.len() - 1).filter(|&t| st[t].vel.y > 0.0 && st[t + 1].vel.y <= 0.0).collect();
    if apex.len() < 3 { eprintln!("{tag}: only {} apexes, need 3", apex.len()); return }
    let (a0, a1) = (apex[1], apex[2]);                       // middle cycle, apex to apex
    let period = a1 - a0;
    // the dive ends where the nose comes down and stays down (a lone spike is polish noise)
    let t_snap = (a0 + 30..a1 - 3).find(|&t| ps[t] < 5.0 && ps[t + 1] < 5.0 && ps[t + 2] < 5.0).unwrap_or(a1);
    let t_gain = (t_snap..a1).find(|&t| st[t].vel.y > 0.0).unwrap_or(a1);
    let t_gend = (t_gain + 10..a1).find(|&t| ps[t] > 0.0).unwrap_or(a1);
    let (dy, dz) = (st[a1].pos.y - st[a0].pos.y, st[a1].pos.z - st[a0].pos.z);

    let (mut acc, mut n, mut gmin, mut gmax) = (0.0, 0, f64::MAX, f64::MIN);
    let mut habs = vec![];
    for t in a0 + period / 8..t_snap {
        let d = bug_gamma_to(&st[t], gamma(st[t].vel)) - ps[t];
        acc += d * d; n += 1; habs.push(d.abs());
        let g = gamma(st[t].vel); gmin = gmin.min(g); gmax = gmax.max(g);
    }
    habs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (hold_rms, hold_med) = ((acc / n as f64).sqrt(), habs[habs.len() / 2]);

    let (mut best, mut r1, mut r20) = ((0usize, f64::MAX), 0.0, 0.0);
    for k in 1..=60usize {
        let (mut acc, mut m) = (0.0, 0);
        for t in t_gain..t_gend { let d = bug_dte_n(&st[t], k) - ps[t]; acc += d * d; m += 1 }
        let rms = (acc / m as f64).sqrt();
        if k == 1 { r1 = rms } else if k == 20 { r20 = rms }
        if rms < best.1 { best = (k, rms) }
    }
    let mut ns: Vec<usize> = (t_gain..t_gend).map(|t| {
        let (mut bn, mut bd) = (0usize, f64::INFINITY);
        for k in 1..=90 { let d = (bug_dte_n(&st[t], k) - ps[t]).abs(); if d < bd { bd = d; bn = k } }
        bn
    }).collect();
    ns.sort();
    println!("{tag:>8} | period {period:>4} | climb {:>6.4} | dist {:>6.3} | dive {:>3}t gamma {:>5.2}..{:<5.2} \
              hold RMS {:>5.2} med {:>4.2} | gain {:>3}t best n={:<3} rms {:>5.2} | n=1 {:>6.2} | n=20 {:>5.2} | n* med {:>2}",
             dy / period as f64 * 20.0, dz / period as f64 * 20.0, t_snap - a0, gmin, gmax,
             hold_rms, hold_med, t_gend - t_gain, best.0, best.1, r1, r20, ns[ns.len() / 2]);
}

// ---------------------------------------------------------------- the policy

#[derive(Clone, Copy, Debug)]
struct P { g_star: f64, k: f64, s_switch: f64, vy_flick: f64, s_exit: f64, slew: f64, p_push: f64, p_flick: f64, n_gain: usize }

/// Fly the four bugs, switching on state rather than on the clock, with a pitch rate limit.
fn fly(par: P, ticks: usize) -> (Vec<f64>, Vec<u8>, Vec<State>) {
    let mut s = State { pos: Vec3::ZERO, vel: V0 };
    let (mut ps, mut ph, mut st) = (vec![], vec![], vec![s.clone()]);
    let (mut phase, mut last) = (0u8, 0.0f64);
    for _ in 0..ticks {
        phase = match phase {
            0 if s.vel.length() >= par.s_switch => 1,     // dive  -> snap
            1 if s.vel.y >= par.vy_flick        => 2,     // snap  -> flick
            2 if last <= par.p_flick + 1e-9     => 3,     // flick -> gain, once the ramp lands
            3 if s.vel.length() <= par.s_exit   => 0,     // gain  -> dive
            p => p,
        };
        let want = match phase {
            0 => if gamma(s.vel) < 0.0 { par.p_push } else { bug_dive(&s, par.g_star, par.k) },
            1 => 0.0,
            2 => par.p_flick,
            _ => bug_dte_n(&s, par.n_gain),
        };
        let p = want.clamp(last - par.slew, last + par.slew);
        s = ticked(&s, p);
        last = p;
        ps.push(p); ph.push(phase); st.push(s.clone());
    }
    (ps, ph, st)
}
fn rate_of(par: P, t: usize) -> f64 { fly(par, t).2[t].pos.y / t as f64 * 20.0 }

fn cmd_policy(optimise: bool) {
    let ticks = 1500;
    let ng: usize = std::env::var("NGAIN").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    let mut par = P { g_star: 17.73, k: 0.055, s_switch: 2.40, vy_flick: -0.260,
                      s_exit: 0.21, slew: 12.7, p_push: 23.0, p_flick: -88.5, n_gain: ng };
    if optimise {
        let mut step = [3.0, 0.03, 0.30, 0.10, 0.08, 8.0, 6.0, 12.0];
        for _ in 0..26 {
            for j in 0..8 {
                let cur = rate_of(par, ticks);
                let (mut bv, mut bp) = (cur, par);
                for d in [-1.0f64, 1.0] {
                    let mut q = par;
                    let s = d * step[j];
                    match j {
                        0 => q.g_star += s, 1 => q.k = (q.k + s).clamp(0.002, 1.0),
                        2 => q.s_switch += s, 3 => q.vy_flick += s,
                        4 => q.s_exit = (q.s_exit + s).max(0.05), 5 => q.slew = (q.slew + s).clamp(2.0, 90.0),
                        6 => q.p_push += s, _ => q.p_flick = (q.p_flick + s).clamp(-90.0, -10.0),
                    }
                    let v = rate_of(q, ticks);
                    if v > bv { bv = v; bp = q }
                }
                if bv > cur { par = bp } else { step[j] *= 0.55 }
            }
        }
    }
    let (ps, ph, st) = fly(par, ticks);
    let r = rate_of(par, ticks);
    eprintln!("n_gain {:>2}  {r:.5} b/s  {:>5.1}% of the optimal cycle   {par:?}", par.n_gain, r / OPTIMAL_CYCLE_RATE * 100.0);
    println!("tick,phase,pitch,vy,vz,speed,gamma,te");
    for t in 0..ticks {
        println!("{t},{},{:.5},{:.6},{:.6},{:.6},{:.4},{:.6}", ph[t], ps[t],
                 st[t].vel.y, st[t].vel.z, st[t].vel.length(), gamma(st[t].vel), st[t].total_energy());
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n = |i: usize| a[i].parse().unwrap();
    match a.get(1).map(String::as_str) {
        Some("profiles") => cmd_profiles(),
        Some("eq") => cmd_eq(),
        Some("polish") => cmd_polish(&a[2], a.get(3).map_or(40, |s| s.parse().unwrap()),
                                     a.get(4).map_or(0.0, |s| s.parse().unwrap())),
        Some("cycle") => cmd_cycle(&a[2], n(3)),
        Some("score") => cmd_score(&a[2], n(3), n(4), n(5)),
        Some("probe") => cmd_probe(&a[2], n(3), n(4), n(5)),
        Some("family") => cmd_family(&a[2], &a[3]),
        Some("policy") => cmd_policy(a.get(2).map(String::as_str) == Some("opt")),
        _ => eprintln!("{}", "usage: myopic <profiles|eq|polish|cycle|score|probe|family|policy> ...\n\
                              see the module docs at the top of src/bin/myopic.rs"),
    }
}
