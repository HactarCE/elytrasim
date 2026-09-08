//! Which *myopic* metrics does the globally optimal climb cycle agree with, phase by phase?
//!
//! The optimal cycle splits into five phases. Four of them follow a rule that needs only the
//! current velocity; the entry does not, and is the open problem:
//!
//! | phase | ticks | rule                                                  |
//! |-------|-------|-------------------------------------------------------|
//! | entry | ~40   | **no rule known** -- pitches hard down, unexplained    |
//! | dive  | ~150  | hold the flight-path angle exactly; no constants       |
//! | snap  | ~14   | pitch 0                                               |
//! | flick | ~6    | ramp to about -88 deg; the values do not matter       |
//! | gain  | ~86   | argmax over pitch of delta TE over ~20 ticks          |
//!
//! The dive was long described as leaking toward ~16.6 deg at a rate `k`. That was an artifact
//! of asking one rule to cover the entry as well: hand the first ~60 ticks to the optimum and
//! the exact hold beats every leaking variant (97.9% of the cycle). `k`, `g_star` and the floor
//! clamp are all entry corrections, kept here only because they still fly the whole cycle
//! open-loop. See "The leak was an entry correction" in README-myopic.md.
//!
//! Everything here is measured against `sim`'s physics with yaw pinned to zero, so the whole
//! problem lives in the (v_y, v_z) plane. See README-myopic.md for the numbers.
//!
//! Subcommands:
//!   profiles                             cycle closure and climb rate of the built-in profiles
//!   eq                                   terminal-glide table, and the fastest steady glide
//!   eqrate                               steady glide maximizing the objective rate, per w
//!   glide    [step] [lo] [hi]             steady-glide curves vs pitch, as CSV
//!   crit                                 the three critical points of the steady glide, both sides
//!   polish   <file> [passes] [w]         coordinate-ascent polish of a schedule; maximizes TE + w*z
//!   cycle    <file> <off>                per-tick dump: pitch, gamma, and each rule's answer
//!   score    <file> <off> <lo> <hi>      RMS pitch error of a menu of rules, per phase
//!   probe    <file> <off> <lo> <hi>      implied lookahead n*(t) through the gain phase
//!   family   <file> <tag>                auto-detect the phases and summarize the fit
//!   floor    <file> <tag>                fit the dive's first-order gamma decay and its asymptote
//!   prices   <file> <tag>                shadow prices from the optimum, and the glide they pick
//!   gprofile <file> <tag>                flight-path angle at ten points through the dive
//!   adjoint  <file> [w] [dump]           solve the periodic price vector and test the one-tick rule
//!   sweepn   <file> <off> <lo> <hi> <N>  argmax pitch for every lookahead 1..N, per tick
//!   policy   [opt] [leak|floor|hold]     fly the four bugs; NGAIN=<n> sets the gain lookahead
//!
//! `--trig libm|mth_lut` picks the trig implementation for any subcommand. `mth_lut` is
//! Minecraft's own 65536-entry sine table; `libm` (the default) is the platform's, which is
//! what every number in README-myopic.md was measured with.
//!
//! A schedule file is whitespace-separated pitches in degrees. `<off>` is the tick offset of
//! the cycle to read, so a 3x-tiled 900-tick flight is read horizon-free at offset 300.

use elytrasim::opt::*;
use elytrasim::sim::*;
use rayon::prelude::*;

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
    let (bzp, _) = ceiling();
    let e = equilibrium(bzp);
    println!("fastest steady glide: pitch {bzp:.3}  v_z {:.5}  v_y {:.5}  gamma {:.4}", e.z, e.y, gamma(e));
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
            // tick's line search optimizes against a stale state and the schedule diverges
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
    row("hold gamma, floored".into(), &bug_dive_floor);
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

/// Dump the whole tick x lookahead matrix of argmax pitches, so the spread across horizons
/// can be read directly. A narrow spread means the implied lookahead is barely determined.
fn cmd_sweepn(path: &str, off: usize, lo: usize, hi: usize, nmax: usize) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    print!("rel,pitch");
    for n in 1..=nmax { print!(",n{n}") }
    println!();
    for rel in lo..hi {
        let s = &st[off + rel];
        print!("{rel},{:.4}", ps[off + rel]);
        for n in 1..=nmax { print!(",{:.4}", bug_dte_n(s, n)) }
        println!();
    }
}

/// Find the cycle in a schedule without being told where it is, then report how well the
/// dive and gain rules fit it. Used to check the rules across a family of optimal cycles.
/// Middle cycle of a 3x-tiled schedule, split into its four phases: `(a0, t_snap, t_gain,
/// t_gend, a1)`. Apex to apex, so the dive is `a0..t_snap` and the gain is `t_gain..t_gend`.
fn cmd_family(path: &str, tag: &str) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    let Some((a0, t_snap, t_gain, t_gend, a1)) = segment(&ps, &st, tag) else { return };
    let period = a1 - a0;
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

/// The steady glide that maximizes the *objective rate* `v_y + w*v_z`.
///
/// Turnpike candidate for the dive's flight-path-angle floor. Maximizing `TE + w*z` over a
/// fixed number of ticks is maximizing the time-average of `d/dt (TE + w*z) = v_y + w*v_z`,
/// so if the dive were asymptoting to the best available *steady* state for the objective,
/// the floor would track this angle. Note what it reduces to at w = 0: the minimum-sink
/// glide, not the fastest one.
/// Steady-glide curves against pitch, as CSV, for `tools/plot_glide.py`.
///
/// The horizontal axis is `sim`'s `z` (yaw is pinned to zero); it is reported as `vx` because
/// that is the convention everywhere outside the sim.
fn cmd_glide(step: f64, lo: f64, hi: f64) {
    println!("# steady glide vs pitch, trig={}", trig_mode());
    println!("pitch,vy,vx,speed,glide,gamma");
    let n = ((hi - lo) / step).round() as i64;
    let rows: Vec<String> = (0..=n).into_par_iter().map(|i| {
        let p = lo + step * i as f64;
        let e = equilibrium(p);
        format!("{p:.6},{:.12},{:.12},{:.12},{:.9},{:.6}",
                e.y, e.z, e.length(), e.z / -e.y, gamma(e))
    }).collect();
    for r in rows { println!("{r}") }
}

/// The three critical points of the steady glide, each read from both sides.
///
/// The interesting one is pitch 0. For `p >= 0` the tick map depends on pitch *only* through
/// `lift_force = cos^2(p)`: yaw is zero, so `look_angle.x` is zero and every use of
/// `look_angle.z / look_hor_length` collapses to 1, cancelling `cos(p)` out of the direction
/// terms, and the `lean_angle < 0` branch is off. `cos^2` is even, so every curve here is
/// even-and-flat to second order on the right of 0. For `p < 0` that branch switches on with a
/// term linear in `sin(p)`, which is a nonzero one-sided slope. So 0 is a *corner*, not a jump.
fn cmd_crit() {
    let crit = |name: &str, p0: f64, key: &dyn Fn(Vec3) -> f64| {
        // refine on a fine sweep; the tops are flat to ~1e-9, so a ternary search wanders
        let (mut bp, mut bs) = (p0, f64::NEG_INFINITY);
        for i in -2000..=2000 {
            let p = p0 + 0.001 * i as f64;
            if !(-90.0..=90.0).contains(&p) { continue }
            let v = key(equilibrium(p));
            if v > bs { bs = v; bp = p }
        }
        println!("\n{name}: argmax at pitch {bp:.4}");
        println!("{:>12} {:>16} {:>16} {:>14} {:>12}", "pitch", "vy", "vx", "glide", "gamma");
        let mut ps = vec![bp];
        for e in [1e-1, 1e-2, 1e-3, 1e-5, 1e-7] { ps.push(bp - e); ps.push(bp + e) }
        ps.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for p in ps {
            let e = equilibrium(p);
            let mark = if p == bp { " <-" } else { "" };
            println!("{p:>12.7} {:>16.12} {:>16.12} {:>14.9} {:>12.6}{mark}",
                     e.y, e.z, e.z / -e.y, gamma(e));
        }
    };
    println!("steady glide critical points, trig={}", trig_mode());
    crit("max glide ratio (blocks forward per block fallen)", 0.0, &|v| v.z / -v.y);
    crit("min sink (max vy)", -13.0, &|v| v.y);
    crit("max forward speed (max vx)", 53.0, &|v| v.z);
}

fn cmd_eqrate() {
    // TE is in blocks, so potential energy is exactly height and v_y carries weight 1.
    const G: f64 = 1.0;
    // The locus is a curve in the (v_z, v_y) plane parameterized by pitch. Build it once;
    // each point is 40k iterations of the velocity map.
    let step = 0.05;
    let tab: Vec<(f64, Vec3)> = (0..=(180.0 / step) as i64)
        .map(|i| { let p = -90.0 + step * i as f64; (p, equilibrium(p)) })
        .collect();
    let pick = |f: &dyn Fn(Vec3) -> f64| -> (f64, Vec3) {
        let (mut bp, mut bs) = (0.0, f64::NEG_INFINITY);
        for &(p, e) in &tab { let v = f(e); if v > bs { bs = v; bp = p } }
        let (mut a, mut b) = ((bp - step).max(-90.0), (bp + step).min(90.0));
        for _ in 0..80 {
            let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
            if f(equilibrium(m1)) < f(equilibrium(m2)) { a = m1 } else { b = m2 }
        }
        let p = 0.5 * (a + b);
        if f(equilibrium(p)) > bs { (p, equilibrium(p)) } else { (bp, equilibrium(bp)) }
    };

    println!("reference points on the equilibrium locus");
    println!("{:<28} {:>8} {:>10} {:>10} {:>9} {:>9}", "", "pitch", "eq v_y", "eq v_z", "|eq|", "gamma");
    let refs: [(&str, &dyn Fn(Vec3) -> f64); 3] = [
        ("fastest glide  (max v_z)", &|e: Vec3| e.z),
        ("min-sink glide (max v_y)", &|e: Vec3| e.y),
        ("best glide ratio (min y)", &|e: Vec3| -gamma(e)),
    ];
    for (name, f) in refs {
        let (p, e) = pick(f);
        println!("{name:<28} {p:>8.3} {:>10.5} {:>10.5} {:>9.5} {:>9.4}", e.y, e.z, e.length(), gamma(e));
    }

    // The observed floors, from README-myopic.md's family table.
    // w is in the block convention; README-myopic.md's family table predates it, so its
    // w column is these divided by GRAVITY. The gammas are unchanged.
    let obs: &[(f64, f64)] = &[(-0.125, 12.81), (-0.0625, 14.62), (-0.025, 16.80), (0.0, 16.72),
                               (0.025, 16.32), (0.0625, 15.94), (0.125, 15.36), (0.250, 14.45)];
    println!("\nturnpike glide for the objective TE + w*z, vs the observed dive floor");
    println!("{:>7} {:>8} {:>10} {:>10} {:>9} {:>9} {:>12} {:>9}",
             "w", "pitch", "eq v_y", "eq v_z", "|eq|", "gamma_eq", "rate", "observed");
    for &(w, g_obs) in obs {
        let (p, e) = pick(&|e: Vec3| G * e.y + w * e.z);
        println!("{w:>7.4} {p:>8.3} {:>10.5} {:>10.5} {:>9.5} {:>9.4} {:>12.6} {g_obs:>9.2}",
                 e.y, e.z, e.length(), gamma(e), G * e.y + w * e.z);
    }
    if std::env::args().any(|a| a == "locus") {
        println!("\npitch,eq_vy,eq_vz,speed,gamma");
        for i in 0..=3600 { let p = -90.0 + 0.05 * i as f64; let e = equilibrium(p);
            println!("{p:.2},{:.6},{:.6},{:.6},{:.4}", e.y, e.z, e.length(), gamma(e)) }
        return;
    }
    println!("\nthe same sweep, wider, to show which way the turnpike actually moves");
    println!("{:>9} {:>8} {:>10} {:>10} {:>9}", "w", "pitch", "eq v_y", "eq v_z", "gamma_eq");
    for w in [-12.5, -2.5, -1.0, -0.5, -0.25, -0.125, 0.0, 0.125, 0.25, 0.5, 1.0, 2.5, 12.5, 125.0] {
        let (p, e) = pick(&|e: Vec3| G * e.y + w * e.z);
        println!("{w:>9.3} {p:>8.3} {:>10.5} {:>10.5} {:>9.4}", e.y, e.z, gamma(e));
    }
}

/// What angle is the dive's flight-path angle actually heading for?
///
/// Near w = 0 gamma dips to a plateau and comes back up as the snap approaches, so the trough
/// is a real floor. On the min-distance side it climbs monotonically and there is no floor at
/// all. `min gamma over the whole dive` cannot tell those apart -- on the climbing shapes it
/// returns the early transient instead, which is what produced the spurious non-monotonicity
/// in the family table. So take the minimum over the middle of the dive only, and say whether
/// it is interior; if it is not, there is no floor to report.
///
/// (A first-order decay `gamma_{t+1} - gamma_t = k (c - gamma_t)` fits badly -- the shape is
/// dip-then-rise, not decay -- and on a polished schedule the tick-scale jitter enters
/// regressor and response with opposite signs and inflates k. Not worth reporting.)
fn cmd_floor(path: &str, tag: &str) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    let Some((a0, t_snap, _, _, a1)) = segment(&ps, &st, tag) else { return };
    let (lo, hi) = (a0 + (a1 - a0) / 8, t_snap);             // same window family scores the hold on
    let g: Vec<f64> = (lo..hi).map(|t| gamma(st[t].vel)).collect();
    let m = g.len() - 1;
    let (i, &trough) = g[m / 5..=4 * m / 5].iter().enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap();
    let at = (m / 5 + i) as f64 / m as f64;
    let interior = at > 0.22 && at < 0.78;
    // How much forward speed does that angle buy? Invert it onto the nose-down branch of the
    // equilibrium locus and read the steady speed there against the ceiling.
    let (mut bp, mut bd) = (0.0, f64::MAX);
    for i in 400..=1800 {
        let q = 0.05 * i as f64;
        let d = (gamma(equilibrium(q)) - trough).abs();
        if d < bd { bd = d; bp = q }
    }
    let vz = equilibrium(bp).z;
    const VZ_MAX: f64 = 3.3887937;                           // fastest steady glide, from `eq`
    println!("{tag:>9} | dive {:>3}t | gamma {:>5.2} -> {:>5.2} | trough {:>5.2} at {:>3.0}% {} \
              | that glide: pitch {bp:>5.2} v_z {vz:>7.5} = {:>6.3}% of the ceiling",
             m + 1, g[0], g[m], trough, 100.0 * at,
             if interior { "        " } else { "(no floor)" }, 100.0 * vz / VZ_MAX);
}

/// The costate direction, read off the optimum itself.
///
/// The optimal pitch satisfies the stationary condition `lambda_{t+1} . df/dp = 0`. With yaw
/// pinned the state is two-dimensional, so that one equation pins `lambda` up to sign and
/// scale: it is the normal to the reachable curve's tangent. Sign is fixed by requiring height
/// to be worth something. What comes back is the cycle's *actual* price of distance in units of
/// height, `lambda_z / lambda_y`, which is the number the objective weight `w` was
/// standing in for.
fn costate_dir(v: Vec3, p: f64) -> (f64, f64) {
    let h = 1e-3;
    let (a, m, b) = (update_fall_flying_movement(v, rot(p - h)), update_fall_flying_movement(v, rot(p)),
                     update_fall_flying_movement(v, rot(p + h)));
    let (dy, dz) = ((b.y - a.y) / (2.0 * h), (b.z - a.z) / (2.0 * h));
    let (mut ny, mut nz) = (-dz, dy);
    let n = ny.hypot(nz);
    ny /= n; nz /= n;
    // Sign from the second-order condition: at a maximum of lambda.f the curvature must be
    // non-positive. Do NOT assume lambda_y > 0 -- in a dive, upward velocity is worth less
    // than nothing, and forcing the other branch picks out a nose-up glide that makes no sense.
    let (fyy, fzz) = ((b.y - 2.0 * m.y + a.y) / (h * h), (b.z - 2.0 * m.z + a.z) / (h * h));
    if ny * fyy + nz * fzz > 0.0 { ny = -ny; nz = -nz }
    (ny, nz)
}

/// Does the dive's floor sit where the *measured* prices say a steady glide should?
///
/// `eqrate` asks which equilibrium maximizes `v_y + w*v_z` and gets the wrong answer.
/// This asks the same question with the shadow prices the optimum is actually using, recovered
/// from its own stationary condition, which is the only version of the turnpike claim that
/// has a chance of being true.
fn cmd_prices(path: &str, tag: &str) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    let Some((a0, t_snap, _, _, a1)) = segment(&ps, &st, tag) else { return };
    let step = 0.05;
    let tab: Vec<(f64, Vec3)> = (0..=(180.0 / step) as i64)
        .map(|i| { let p = -90.0 + step * i as f64; (p, equilibrium(p)) }).collect();
    let best_eq = |ly: f64, lz: f64| -> (f64, Vec3) {
        let (mut bp, mut be, mut bs) = (0.0, Vec3::ZERO, f64::NEG_INFINITY);
        for &(p, e) in &tab { let v = ly * e.y + lz * e.z; if v > bs { bs = v; bp = p; be = e } }
        (bp, be)
    };
    println!("{tag}: costate through the dive (w is what eqrate assumed)");
    println!("{:>5} {:>8} {:>8} {:>8} {:>8} {:>9} {:>10} {:>9} {:>9} {:>9}",
             "rel", "pitch", "gamma", "l_y", "l_z", "lz/ly", "eq pitch", "eq gamma", "eq v_z", "argmax-p");
    println!("{:>88}   (last column: global argmax of lambda.f minus the optimum's pitch)", "");
    let (lo, hi) = (a0 + (a1 - a0) / 8, t_snap);
    for t in (lo..hi).step_by(((hi - lo) / 12).max(1)) {
        let (ly, lz) = costate_dir(st[t].vel, ps[t]);
        let (ep, ee) = best_eq(ly, lz);
        // not circular: the tangency fixes lambda locally, so recovering p_t as the *global*
        // argmax of lambda.f is a real check that the extracted lambda is the optimum's own
        let chk = argmax(|q| { let f = update_fall_flying_movement(st[t].vel, rot(q)); ly * f.y + lz * f.z }, 0.125);
        println!("{:>5} {:>8.3} {:>8.3} {ly:>8.4} {lz:>8.4} {:>9.4} {ep:>10.3} {:>9.4} {:>9.5} {:>9.3}",
                 t - a0, ps[t], gamma(st[t].vel), lz / ly, gamma(ee), ee.z, chk - ps[t]);
    }
    // the price the cycle puts on distance, averaged over the settled half of the dive
    let mid = (lo + hi) / 2;
    let (mut sy, mut sz) = (0.0, 0.0);
    for t in mid..hi { let (ly, lz) = costate_dir(st[t].vel, ps[t]); sy += ly; sz += lz }
    let (n, ep_pair) = ((hi - mid) as f64, ());
    let _ = ep_pair;
    let (my, mz) = (sy / n, sz / n);
    let (ep, ee) = best_eq(my, mz);
    println!("settled dive: mean lambda ({my:.4}, {mz:.4}), lz/ly {:.4}  ->  equilibrium pitch {ep:.3}, \
              gamma {:.4}, v_z {:.5}", mz / my, gamma(ee), ee.z);
}

/// Flight-path angle at ten points through the dive, so the *shape* is visible.
///
/// This is what settles the question `min gamma` was getting wrong: near w = 0 the dive dips
/// and comes back up, so the minimum is a real plateau, but on the min-distance side gamma
/// climbs monotonically and there is no floor at all -- there the minimum is just the start of
/// the dive, and reading it as an asymptote invents a trend that is not there.
fn cmd_gprofile(path: &str, tag: &str) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    let Some((a0, t_snap, _, _, a1)) = segment(&ps, &st, tag) else { return };
    let (lo, hi) = (a0 + (a1 - a0) / 8, t_snap);
    let g: Vec<f64> = (lo..hi).map(|t| gamma(st[t].vel)).collect();
    let m = g.len() - 1;
    print!("{tag:>9} |");
    for i in 0..=10 { print!(" {:>5.2}", g[m * i / 10]) }
    println!(" | min {:>5.2} at {:>3.0}%",
             g.iter().cloned().fold(f64::MAX, f64::min),
             100.0 * g.iter().enumerate().min_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0 as f64 / m as f64);
}


/// Is the optimum the one-tick argmax of a linear score on next tick's velocity?
///
/// Over a closed cycle the objective is `sum_t c . v_{t+1}` with `c = (1, w)`, because the
/// kinetic terms cancel when the cycle closes. Pontryagin then says the optimum maximizes
/// `mu_{t+1} . f(v_t, p)` at every tick -- a genuinely myopic, one-tick, horizon-free score --
/// where the price vector obeys `mu_t = c + A_t^T mu_{t+1}`, `A_t = df/dv`.
///
/// That recursion plus periodicity (`mu_N = mu_0`) pins `mu` completely: solve `(I - M) mu_0 = b`
/// where `M` and `b` accumulate the recursion around the loop. **No free parameters.** So the
/// tangency at each of the N ticks is a separate falsifiable prediction, unlike reading `mu` off
/// the optimum's own pitch, which is stationary by construction. Reported as the gap in degrees
/// between the optimum's pitch and the argmax of the score.
fn cmd_adjoint(path: &str, w: f64) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    let n = ps.len();
    let close = (st[n].vel - st[0].vel).length();
    println!("{path}: {n} ticks, |v_N - v_0| = {close:.3e}, w = {w}");
    if close > 1e-3 { println!("  !! not a closed cycle; the periodic adjoint does not apply") }
    let c = (1.0, w);

    let (mut b, mut m) = ((0.0, 0.0), [1.0, 0.0, 0.0, 1.0]);
    for t in (0..n).rev() {
        let a = jac(st[t].vel, ps[t]);
        let ab = mt_vec(&a, b);
        b = (c.0 + ab.0, c.1 + ab.1);
        m = mt_mat(&a, &m);
    }
    // (I - M) mu_0 = b
    let d = [1.0 - m[0], -m[1], -m[2], 1.0 - m[3]];
    let det = d[0] * d[3] - d[1] * d[2];
    println!("  monodromy M = [{:.4} {:.4}; {:.4} {:.4}], det(I-M) = {det:.4e}", m[0], m[1], m[2], m[3]);
    if det.abs() < 1e-12 { println!("  !! singular; mu is not determined"); return }
    let mut mu = vec![(0.0, 0.0); n + 1];
    mu[n] = ((d[3] * b.0 - d[1] * b.1) / det, (-d[2] * b.0 + d[0] * b.1) / det);
    for t in (0..n).rev() {
        let a = jac(st[t].vel, ps[t]);
        let am = mt_vec(&a, mu[t + 1]);
        mu[t] = (c.0 + am.0, c.1 + am.1);
    }
    let drift = ((mu[0].0 - mu[n].0).hypot(mu[0].1 - mu[n].1)) / mu[0].0.hypot(mu[0].1);
    println!("  mu_0 = ({:.4}, {:.4}), periodicity residual {drift:.2e}", mu[0].0, mu[0].1);

    // the prediction: p_t is the global argmax over pitch of mu_{t+1} . f(v_t, p)
    let mut err: Vec<f64> = vec![];
    for t in 0..n {
        let (my, mz) = mu[t + 1];
        let q = argmax(|p| { let f = update_fall_flying_movement(st[t].vel, rot(p)); my * f.y + mz * f.z }, 0.125);
        err.push(q - ps[t]);
    }
    let rms = |a: &[f64]| (a.iter().map(|x| x * x).sum::<f64>() / a.len() as f64).sqrt();
    let mut srt: Vec<f64> = err.iter().map(|x| x.abs()).collect();
    srt.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("  argmax gap over all {n} ticks: RMS {:.4}deg  median {:.4}  p90 {:.4}  max {:.4}",
             rms(&err), srt[n / 2], srt[n * 9 / 10], srt[n - 1]);
    if let Some((a0, t_snap, t_gain, t_gend, a1)) = segment(&ps, &st, path) {
        for (name, lo, hi) in [("dive", a0, t_snap), ("snap", t_snap, t_gain),
                               ("flick", t_gain, t_gend), ("gain", t_gend, a1)] {
            if hi > lo { println!("    {name:<6} {:>3}t  RMS {:>8.4}  max {:>8.4}",
                                  hi - lo, rms(&err[lo..hi]), err[lo..hi].iter().fold(0.0f64, |m, x| m.max(x.abs()))) }
        }
    }
    if std::env::args().any(|a| a == "dump") {
        println!("t,pitch,vy,vz,gamma,mu_y,mu_z,ratio,gap");
        for t in 0..n {
            println!("{t},{:.4},{:.6},{:.6},{:.4},{:.6},{:.6},{:.6},{:.4}", ps[t], st[t].vel.y, st[t].vel.z,
                     gamma(st[t].vel), mu[t + 1].0, mu[t + 1].1, mu[t + 1].1 / mu[t + 1].0, err[t]);
        }
    }
}

/// How sharply does the one-tick score pick out the optimum's pitch?
///
/// Pontryagin says the optimum maximizes `mu . f(v, p)` over p every tick. That is exact, but it
/// only *determines* the pitch if the score has curvature at its maximum. Where the score is flat
/// the condition is satisfied by a whole range of pitches and the maximum principle says nothing
/// -- a singular arc -- and the control has to come from somewhere else, typically a feedback law
/// in the state. Which is exactly the shape of "hold the flight-path angle".
///
/// Uses the per-tick tangency direction, so `p_t` is stationary by construction and the curvature
/// there is clean. Reports `d^2 S/dp^2` in degrees^-2 and the half-width over which the score
/// stays within 1e-6 of its maximum.
fn cmd_singular(path: &str) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    let n = ps.len();
    let mut curv = vec![0.0; n];
    let mut half = vec![0.0; n];
    for t in 0..n {
        let (uy, uz) = costate_dir(st[t].vel, ps[t]);
        let sc = |p: f64| { let f = update_fall_flying_movement(st[t].vel, rot(p)); uy * f.y + uz * f.z };
        let (h, s0) = (0.5, sc(ps[t]));
        curv[t] = (sc(ps[t] + h) - 2.0 * s0 + sc(ps[t] - h)) / (h * h);
        let (mut lo, mut hi) = (0.0, 90.0);
        for _ in 0..40 {
            let m = 0.5 * (lo + hi);
            if s0 - sc(ps[t] + m).max(sc(ps[t] - m)) < 1e-6 { lo = m } else { hi = m }
        }
        half[t] = 0.5 * (lo + hi);
    }
    println!("{path}: curvature of the one-tick score at the optimum's own pitch");
    println!("{:>7} {:>5} {:>12} {:>12}", "phase", "t", "d2S/dp2", "half-width");
    let show = |name: &str, lo: usize, hi: usize| {
        let c: Vec<f64> = curv[lo..hi].iter().map(|x| x.abs()).collect();
        let w: Vec<f64> = half[lo..hi].to_vec();
        let med = |mut v: Vec<f64>| { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[v.len() / 2] };
        println!("{name:>7} {:>5} {:>12.2e} {:>10.2}deg", hi - lo, med(c), med(w));
    };
    if let Some((a0, t_snap, t_gain, t_gend, a1)) = segment(&ps, &st, path) {
        show("dive", a0, t_snap); show("snap", t_snap, t_gain);
        show("flick", t_gain, t_gend); show("gain", t_gend, a1);
    } else {
        show("all", 0, n);
        for (name, lo, hi) in [("entry", 1, 20), ("entry30", 1, 30), ("dive", 20, 190),
                               ("snap", 190, 205), ("flick", 205, 213), ("gain", 213, 295)] {
            show(name, lo, hi);
        }
    }
}

/// What does a pitch error at tick t actually cost the cycle?
///
/// The curvature of `mu . f` is a statement about a linear score in abstract units. This is the
/// operational version: nudge one tick's pitch, let the schedule re-converge to its own limit
/// cycle, and read the change in climb rate. Answers "does the pitch at this tick matter", in
/// blocks per second, with no theory in between.
fn cmd_sens(path: &str, delta: f64) {
    let base = read_pitches(path);
    let n = base.len();
    let rate = |ps: &[f64]| -> f64 {
        let mut v = V0;
        for _ in 0..40 { for &p in ps { v = update_fall_flying_movement(v, rot(p)) } }
        let mut s = State { pos: Vec3::ZERO, vel: v };
        for &p in ps { s = ticked(&s, p) }
        s.pos.y / n as f64 * 20.0
    };
    let base_rate = rate(&base);
    println!("{path}: climb {base_rate:.5} b/s; cost of a {delta}deg pitch error at one tick");
    let mut cost = vec![0.0; n];
    for t in 0..n {
        let mut up = base.clone(); up[t] += delta;
        let mut dn = base.clone(); dn[t] -= delta;
        // symmetric part: the second-order cost, which is what "flat" is really asking about
        cost[t] = base_rate - 0.5 * (rate(&up) + rate(&dn));
    }
    let show = |name: &str, lo: usize, hi: usize| {
        let mut v: Vec<f64> = cost[lo..hi].iter().cloned().collect();
        let sum: f64 = v.iter().sum();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let _ = sum;
        // a correlated shift of the whole phase is NOT the sum of the one-tick costs -- the cross
        // terms dominate -- so shift it for real and re-converge
        let corr = |d: f64| { let mut q = base.clone(); for t in lo..hi { q[t] += d } rate(&q) };
        println!("{name:>11} {:>4}t | one tick: median {:>9.2e} max {:>9.2e} | whole phase off by \
                  {delta}deg: {:>+8.4} / {:>+8.4} b/s",
                 hi - lo, v[v.len() / 2], v[v.len() - 1], corr(delta) - base_rate, corr(-delta) - base_rate);
    };
    let st = replay(&base);
    let v_top = (0..n).map(|t| st[t].vel.length()).fold(0.0, f64::max);
    let t_snap = (30..n - 3).find(|&t| base[t] < 5.0 && base[t + 1] < 5.0 && base[t + 2] < 5.0
                                    && st[t].vel.length() > 0.8 * v_top).unwrap_or(n * 2 / 3);
    let t_gain = (t_snap..n).find(|&t| st[t].vel.y > 0.0).unwrap_or(t_snap);
    let t_gend = (t_gain + 10..n).find(|&t| base[t] > 0.0).unwrap_or(n);
    show("dive", 20, t_snap); show("snap+flick", t_snap, t_gain); show("gain", t_gain, t_gend);
    if std::env::args().any(|a| a == "dump") {
        println!("t,pitch,gamma,cost");
        for t in 0..n { println!("{t},{:.4},{:.3},{:.6e}", base[t], gamma(st[t].vel), cost[t]) }
    }
}

/// One tick, spelled out: how next tick's velocity swings with pitch, and where the price sits.
fn cmd_swing(path: &str, t: usize) {
    let ps = read_pitches(path);
    let st = replay(&ps);
    let (v, p) = (st[t].vel, ps[t]);
    println!("tick {t}: v = ({:.5}, {:.5}), gamma {:.2}, pitch {p:.3}", v.y, v.z, gamma(v));
    println!("{:>8} {:>11} {:>11}", "pitch", "next v_y", "next v_z");
    for d in [-2.0, -1.0, 0.0, 1.0, 2.0] {
        let f = update_fall_flying_movement(v, rot(p + d));
        println!("{:>8.3} {:>11.6} {:>11.6}", p + d, f.y, f.z);
    }
    let h = 1e-3;
    let (a, b) = (update_fall_flying_movement(v, rot(p - h)), update_fall_flying_movement(v, rot(p + h)));
    let (dy, dz) = ((b.y - a.y) / (2.0 * h), (b.z - a.z) / (2.0 * h));
    println!("df/dp = ({dy:.6}, {dz:.6}) per degree  -- the direction next velocity slides as you pitch");
    let (uy, uz) = costate_dir(v, p);
    println!("price = ({uy:.6}, {uz:.6})  -- perpendicular to it: dot = {:.3e}", uy * dy + uz * dz);
}

/// Print one cycle of a schedule: the middle cycle of a 3x tiling, or the policy's limit cycle.
/// Both are needed as controls for `consist`, which requires a genuinely closed cycle.
fn cmd_cyclecut(what: &str) {
    let (ps, st, tag): (Vec<f64>, Vec<State>, &str) = if what == "policy" {
        let par = P { g_star: 17.73, k: 0.055, s_switch: 2.40, vy_flick: -0.260,
                      s_exit: 0.21, slew: 12.7, p_push: 23.0, p_flick: -88.5, n_gain: 20, dive: Dive::Leak };
        let (ps, _, st) = fly(par, 2000);
        (ps, st, "policy")
    } else {
        let ps = read_pitches(what);
        let st = replay(&ps);
        (ps, st, "file")
    };
    // last two apexes, so the policy has settled onto its limit cycle
    let apex: Vec<usize> = (1..ps.len() - 1).filter(|&t| st[t].vel.y > 0.0 && st[t + 1].vel.y <= 0.0).collect();
    let k = apex.len();
    if k < 3 { eprintln!("{tag}: {k} apexes, need 3"); return }
    let (a0, a1) = (apex[k - 2], apex[k - 1]);
    eprintln!("{tag}: cycle {a0}..{a1} ({} ticks)", a1 - a0);
    for t in a0..a1 { println!("{}", ps[t]) }
}

/// Is the price vector's consistency special to the optimum, or does any profile have it?
///
/// Every schedule has, at each tick, *some* direction making its pitch stationary -- just take
/// the normal to `df/dp`. That part is vacuous. What is not vacuous is whether the directions
/// found tick by tick are mutually consistent: the true costate must satisfy
/// `mu_t = c + A_t^T mu_{t+1}` and close on itself around the cycle, which leaves no freedom at
/// all. So solve for `mu` from the recursion alone and then measure, at every tick, the angle
/// between it and the perpendicular to `df/dp`. Zero for an optimum; the control is a
/// deliberately perturbed schedule, re-closed into its own limit cycle.
///
/// The angle is the right metric rather than the argmax gap, because the one-tick score is very
/// flat in the dive (see `singular`) and a fraction of a percent in `mu` throws the argmax by ten
/// degrees there without meaning anything.
fn cmd_consist(path: &str, amp: f64) {
    let tag = path.rsplit('/').next().unwrap().trim_end_matches(".txt");
    let base = read_pitches(path);
    let n = base.len();
    let ps: Vec<f64> = base.iter().enumerate()
        .map(|(t, &p)| p + amp * (std::f64::consts::TAU * t as f64 / n as f64).sin()).collect();
    // the cycle map is strongly contracting, so iterating the schedule lands on its limit cycle
    let mut v = V0;
    for _ in 0..60 { for &p in &ps { v = update_fall_flying_movement(v, rot(p)) } }
    let mut st = vec![State { pos: Vec3::ZERO, vel: v }];
    for &p in &ps { let s = ticked(st.last().unwrap(), p); st.push(s) }
    let close = (st[n].vel - st[0].vel).length();
    let c = (1.0, 0.0);

    let (mut b, mut m) = ((0.0, 0.0), [1.0, 0.0, 0.0, 1.0]);
    for t in (0..n).rev() {
        let a = jac(st[t].vel, ps[t]);
        let ab = mt_vec(&a, b);
        b = (c.0 + ab.0, c.1 + ab.1);
        m = mt_mat(&a, &m);
    }
    let d = [1.0 - m[0], -m[1], -m[2], 1.0 - m[3]];
    let det = d[0] * d[3] - d[1] * d[2];
    let mut mu = vec![(0.0, 0.0); n + 1];
    mu[n] = ((d[3] * b.0 - d[1] * b.1) / det, (-d[2] * b.0 + d[0] * b.1) / det);
    for t in (0..n).rev() {
        let a = jac(st[t].vel, ps[t]);
        let am = mt_vec(&a, mu[t + 1]);
        mu[t] = (c.0 + am.0, c.1 + am.1);
    }

    // angle between mu_{t+1} and the perpendicular to df/dp: the stationary residual
    let mut res = vec![0.0; n];
    for t in 0..n {
        let h = 1e-3;
        let (a, bb) = (update_fall_flying_movement(st[t].vel, rot(ps[t] - h)),
                       update_fall_flying_movement(st[t].vel, rot(ps[t] + h)));
        let (dy, dz) = ((bb.y - a.y) / (2.0 * h), (bb.z - a.z) / (2.0 * h));
        let (my, mz) = mu[t + 1];
        let cosang = (my * dy + mz * dz).abs() / (my.hypot(mz) * dy.hypot(dz));
        res[t] = cosang.min(1.0).asin().to_degrees();
    }
    let med = |lo: usize, hi: usize| {
        if hi <= lo { return f64::NAN }
        let mut v: Vec<f64> = res[lo..hi].to_vec();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    // the cycle starts at an apex, so segment it in place rather than needing a 3x tiling
    let v_top = (0..n).map(|t| st[t].vel.length()).fold(0.0, f64::max);
    let t_snap = (30..n - 3).find(|&t| ps[t] < 5.0 && ps[t + 1] < 5.0 && ps[t + 2] < 5.0
                                    && st[t].vel.length() > 0.8 * v_top).unwrap_or(n * 2 / 3);
    let t_gain = (t_snap..n).find(|&t| st[t].vel.y > 0.0).unwrap_or(t_snap);
    let t_gend = (t_gain + 10..n).find(|&t| ps[t] > 0.0).unwrap_or(n);
    println!("{tag:>11} amp {amp:>4.1} | climb {:>7.4} b/s | closure {close:>8.1e} | median degrees off perpendicular: \
              all {:>6.3} | dive({:>3}) {:>6.3} | snap+flick({:>2}) {:>6.3} | gain({:>3}) {:>6.3}",
             st[n].pos.y / n as f64 * 20.0, med(0, n),
             t_snap - 20, med(20, t_snap), t_gain - t_snap, med(t_snap, t_gain),
             t_gend - t_gain, med(t_gain, t_gend));
}

/// Two things the flatness result raises but does not answer.
///
/// `sides`: the one-tick sensitivity in `sens` is a symmetric second difference, which hides
/// whether the two directions cost the same. At the snap the pitch sits a tenth of a degree above
/// zero, and zero is a corner of the physics -- the forward-to-up conversion is gated on
/// `lean_angle < 0.0` -- so there is every reason to expect one side to be free and the other a
/// cliff.
///
/// `ksweep`: independent per-tick jitter in the dive costs nothing, but `k` is not per-tick
/// jitter, it sets the dive's whole trend. Sweeps it with the other constants fixed, and again
/// with the two that could absorb it re-tuned, which is the test of whether it is a real degree
/// of freedom or a redundant one.
fn cmd_sides(path: &str, delta: f64) {
    let base = read_pitches(path);
    let n = base.len();
    let rate = |ps: &[f64]| -> f64 {
        let mut v = V0;
        for _ in 0..40 { for &p in ps { v = update_fall_flying_movement(v, rot(p)) } }
        let mut s = State { pos: Vec3::ZERO, vel: v };
        for &p in ps { s = ticked(&s, p) }
        s.pos.y / n as f64 * 20.0
    };
    let r0 = rate(&base);
    let st = replay(&base);
    println!("{path}: climb {r0:.5} b/s; cost of {delta}deg at one tick, each side separately");
    println!("{:>5} {:>8} {:>8} {:>13} {:>13}", "t", "pitch", "gamma", "nose down", "nose up");
    for t in 0..n {
        let interesting = (190..215).contains(&t) || t % 30 == 0;
        if !interesting { continue }
        let (mut up, mut dn) = (base.clone(), base.clone());
        up[t] += delta; dn[t] -= delta;
        println!("{t:>5} {:>8.3} {:>8.2} {:>13.2e} {:>13.2e}",
                 base[t], gamma(st[t].vel), rate(&up) - r0, rate(&dn) - r0);
    }
}

/// Where in the dive does the leak actually earn its keep?
///
/// Josie's hypothesis: most of the difference between leak rates is spent early, around the
/// stretch where the flight-path angle is still being dragged down and the delta-TE family is
/// bimodal. If so, the leak is an *entry* correction rather than a rule for the whole dive, and
/// an exact hold -- no constant at all -- should be nearly free once something else handles the
/// first stretch.
///
/// Hands the first T ticks of every dive to the optimum's own pitches, then lets the rule take
/// over, and retunes the switch speed for each cell so no rule is judged on constants tuned for
/// another. `g_star` stays pinned to the derived ceiling throughout: it is the fastest steady
/// glide's flight-path angle, which the physics fixes, and letting a tuner move it would turn a
/// derived constant into a fitted one.
fn cmd_prefix(path: &str) {
    const TICKS: usize = 1500;
    let opt = read_pitches(path);
    let g_star = ceiling().1;
    let ts = [0usize, 5, 10, 20, 30, 40, 60, 80, 120];
    let rules: [(&str, Dive, f64); 4] = [("hold current", Dive::Hold, f64::NAN),
                                         ("target g*", Dive::Target, f64::NAN),
                                         ("floor clamp", Dive::Floor, f64::NAN),
                                         ("leak k=.04", Dive::Leak, 0.040)];
    let rate = |par: P, pre: &[f64]| fly_pre(par, TICKS, pre).2[TICKS].pos.y / TICKS as f64 * 20.0;
    // best climb over the switch speed: a coarse global grid, because a single seed gets stuck,
    // then a shrinking-step refine
    let best = |par: P, pre: &[f64]| -> (f64, f64) {
        let (mut bv, mut bs) = (f64::NEG_INFINITY, par.s_switch);
        for i in 0..=32 {
            let mut q = par; q.s_switch = 1.4 + 0.05 * i as f64;
            let v = rate(q, pre);
            if v > bv { bv = v; bs = q.s_switch }
        }
        let mut step = 0.05;
        for _ in 0..25 {
            let mut improved = false;
            for d in [-1.0f64, 1.0] {
                let mut q = par; q.s_switch = (bs + d * step).clamp(1.2, 3.2);
                let v = rate(q, pre);
                if v > bv { bv = v; bs = q.s_switch; improved = true }
            }
            if !improved { step *= 0.6 }
        }
        (bv, bs)
    };
    // the (prefix, rule) matrix is embarrassingly parallel
    let cells: Vec<(usize, usize)> = (0..ts.len()).flat_map(|a| (0..rules.len()).map(move |b| (a, b))).collect();
    let out: Vec<f64> = cells.par_iter()
        .map(|&(a, b)| {
            let (_, dive, k) = rules[b];
            let par = P { g_star, k, s_switch: 2.40, vy_flick: -0.260, s_exit: 0.21,
                          slew: 12.7, p_push: 23.0, p_flick: -88.5, n_gain: 20, dive };
            best(par, &opt[..ts[a]]).0
        })
        .collect();

    println!("{path}: dive prefix flown open-loop from the optimum, rule takes over after");
    println!("g_star pinned at {g_star:.4}; s_switch retuned for every cell");
    println!("hold / target / floor are all parameter-free; the leak is the reference");
    print!("{:>7}", "prefix");
    for (n, _, _) in rules { print!(" {n:>13}") }
    println!(" {:>12}", "leak - hold");
    for (a, t) in ts.iter().enumerate() {
        print!("{t:>7}");
        for b in 0..rules.len() { print!(" {:>13.4}", out[a * rules.len() + b]) }
        println!(" {:>12.4}", out[a * rules.len() + 3] - out[a * rules.len()]);
    }
}

fn cmd_ksweep() {
    let ticks = 1500;
    // g_star is NOT free: it is the flight-path angle of the fastest steady glide, which the
    // physics fixes. Letting a tuner move it turns a derived constant into a fitted one and
    // makes k look redundant when it is only being absorbed.
    let g_star = ceiling().1;
    let base = P { g_star, k: 0.055, s_switch: 2.40, vy_flick: -0.260,
                   s_exit: 0.21, slew: 12.7, p_push: 23.0, p_flick: -88.5, n_gain: 20, dive: Dive::Leak };
    println!("g_star pinned at the derived ceiling, {g_star:.4} deg; only s_switch is retuned");
    println!("{:>7} {:>13} {:>13} {:>9}", "k", "as tuned", "retuned", "s_switch");
    for k in [0.005, 0.01, 0.02, 0.03, 0.04, 0.055, 0.07, 0.10, 0.15, 0.25, 0.50, 1.00] {
        let mut p = base; p.k = k;
        let asis = rate_of(p, ticks);
        // Let the two constants that could stand in for the leak try to absorb the change.
        // Coordinate descent from a single seed is not enough: from the k = 0.055 constants it
        // gets stuck against the s_switch clamp for k = 0.15 and 0.25 while k = 0.5 finds 1.28,
        // which is an optimiser artefact and not the leak rate failing. Seed on a grid instead.
        let mut q = p;
        let mut best = f64::NEG_INFINITY;
        for si in 0..=32 {
            let mut r = p;
            r.s_switch = 1.4 + 0.05 * si as f64;
            let v = rate_of(r, ticks);
            if v > best { best = v; q = r }
        }
        let mut step = 0.05;
        for _ in 0..25 {
            let cur = rate_of(q, ticks);
            let (mut bv, mut bq) = (cur, q);
            for d in [-1.0f64, 1.0] {
                let mut r = q;
                r.s_switch = (r.s_switch + d * step).clamp(1.2, 3.2);
                let v = rate_of(r, ticks);
                if v > bv { bv = v; bq = r }
            }
            if bv > cur { q = bq } else { step *= 0.6 }
        }
        println!("{k:>7.3} {asis:>13.4} {:>13.4} {:>9.3}", rate_of(q, ticks), q.s_switch);
    }
}


fn cmd_policy(optimize: bool, dive: Dive) {
    let ticks = 1500;
    let ng: usize = std::env::var("NGAIN").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    // the shared constants, as tuned by `policy opt <rule>` for each dive rule in turn
    let mut par = match dive {
        Dive::Leak => P { g_star: 17.73, k: 0.055, s_switch: 2.40, vy_flick: -0.260,
                          s_exit: 0.21, slew: 12.7, p_push: 23.0, p_flick: -88.5, n_gain: ng, dive },
        Dive::Floor => P { g_star: f64::NAN, k: f64::NAN, s_switch: 2.177, vy_flick: -0.2685,
                           s_exit: 0.45, slew: 12.92, p_push: 24.01, p_flick: -79.44, n_gain: ng, dive },
        Dive::Hold => P { g_star: f64::NAN, k: f64::NAN, s_switch: 2.127, vy_flick: -0.260,
                          s_exit: 0.29, slew: 8.34, p_push: 47.0, p_flick: -41.51, n_gain: ng, dive },
        // steer straight at the derived floor rather than holding what you have; wants its own
        // constants, so `policy opt target` before reading anything into its score
        Dive::Target => P { g_star: f64::NAN, k: f64::NAN, s_switch: 2.177, vy_flick: -0.2685,
                            s_exit: 0.45, slew: 12.92, p_push: 24.01, p_flick: -79.44, n_gain: ng, dive },
    };
    if optimize {
        // g_star and k exist only for the leaking dive; the others are shared
        let js: Vec<usize> = if dive == Dive::Leak { (0..8).collect() } else { (2..8).collect() };
        let mut step = [3.0, 0.03, 0.30, 0.10, 0.08, 8.0, 6.0, 12.0];
        for _ in 0..26 {
            for &j in &js {
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
    if dive == Dive::Floor { eprintln!("dive floor: gamma {:.4}, the glide at pitch {:.3}", ceiling().1, ceiling().0) }
    println!("tick,phase,pitch,vy,vz,speed,gamma,te");
    for t in 0..ticks {
        println!("{t},{},{:.5},{:.6},{:.6},{:.6},{:.4},{:.6}", ph[t], ps[t],
                 st[t].vel.y, st[t].vel.z, st[t].vel.length(), gamma(st[t].vel), st[t].total_energy());
    }
}

fn main() {
    let mut a: Vec<String> = std::env::args().collect();
    // --trig <libm|mth_lut> anywhere in the line, stripped before positional parsing
    if let Some(i) = a.iter().position(|x| x == "--trig") {
        set_trig_mode(a.get(i + 1).unwrap_or_else(|| panic!("--trig needs a mode"))
                       .parse().unwrap_or_else(|e| panic!("{e}")));
        a.drain(i..=i + 1);
    }
    let a = a;
    let n = |i: usize| a[i].parse().unwrap();
    match a.get(1).map(String::as_str) {
        Some("profiles") => cmd_profiles(),
        Some("eq") => cmd_eq(),
        Some("eqrate") => cmd_eqrate(),
        Some("glide") => cmd_glide(a.get(2).map_or(0.05, |s| s.parse().unwrap()),
                                   a.get(3).map_or(-90.0, |s| s.parse().unwrap()),
                                   a.get(4).map_or(90.0, |s| s.parse().unwrap())),
        Some("crit") => cmd_crit(),
        Some("polish") => cmd_polish(&a[2], a.get(3).map_or(40, |s| s.parse().unwrap()),
                                     a.get(4).map_or(0.0, |s| s.parse().unwrap())),
        Some("cycle") => cmd_cycle(&a[2], n(3)),
        Some("score") => cmd_score(&a[2], n(3), n(4), n(5)),
        Some("probe") => cmd_probe(&a[2], n(3), n(4), n(5)),
        Some("family") => cmd_family(&a[2], &a[3]),
        Some("floor") => cmd_floor(&a[2], &a[3]),
        Some("prices") => cmd_prices(&a[2], &a[3]),
        Some("gprofile") => cmd_gprofile(&a[2], &a[3]),
        Some("sides") => cmd_sides(&a[2], a[3].parse().unwrap()),
        Some("ksweep") => cmd_ksweep(),
        Some("prefix") => cmd_prefix(&a[2]),
        Some("sens") => cmd_sens(&a[2], a[3].parse().unwrap()),
        Some("swing") => cmd_swing(&a[2], n(3)),
        Some("cyclecut") => cmd_cyclecut(&a[2]),
        Some("consist") => cmd_consist(&a[2], a[3].parse().unwrap()),
        Some("singular") => cmd_singular(&a[2]),
        Some("adjoint") => cmd_adjoint(&a[2], a.get(3).map_or(0.0, |s| s.parse().unwrap())),
        Some("sweepn") => cmd_sweepn(&a[2], n(3), n(4), n(5), n(6)),
        Some("policy") => cmd_policy(a.iter().any(|x| x == "opt"),
                                     match a.iter().find(|x| ["leak", "floor", "hold", "target"].contains(&x.as_str())) {
                                         Some(x) if x == "floor" => Dive::Floor,
                                         Some(x) if x == "hold" => Dive::Hold,
                                         Some(x) if x == "target" => Dive::Target,
                                         _ => Dive::Leak,
                                     }),
        _ => eprintln!("{}", "usage: myopic <profiles|eq|eqrate|glide|crit|polish|cycle|score|probe|family|floor|prices|gprofile|adjoint|singular|consist|cyclecut|sens|swing|prefix|sweepn|policy> ...\n\
                              see the module docs at the top of src/bin/myopic.rs"),
    }
}
