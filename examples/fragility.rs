//! How much of a schedule's score survives the things you cannot control?
//!
//! Chatter is one symptom of overfitting; it is not the only one, and a smooth schedule can
//! still be tuned to a starting state nobody can reproduce. This is the battery:
//!
//!   dJ         the schedule's score, flown exactly as written from the exact v0
//!   E[v]s      mean dJ over initial-velocity error, sigma per component in blocks/tick
//!   p05[v]     the 5th percentile of the same -- the bad day, which is what actually matters
//!   phase      mean dJ over sub-tick phase: read the schedule as samples of a piecewise linear
//!              control and let the game sample it at t + phi instead of t
//!   late1      dJ having started the schedule one tick late (the first pitch is repeated)
//!   quant      dJ with every pitch rounded to a tenth of a degree
//!   hold2      dJ with pitch held over pairs of ticks at the pair's mean
//!
//! usage: fragility <file>...
use elytrasim::opt::*;
use elytrasim::sim::*;

fn resample(p: &[f64], phi: f64) -> Vec<f64> {
    let n = p.len();
    (0..n).map(|t| {
        let x = t as f64 + phi;
        let (i, f) = (x.floor(), x - x.floor());
        let i = i as isize;
        let at = |k: isize| p[k.clamp(0, n as isize - 1) as usize];
        at(i) * (1.0 - f) + at(i + 1) * f
    }).collect()
}

fn block_hold(p: &[f64], k: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(p.len());
    for c in p.chunks(k) {
        let m = c.iter().sum::<f64>() / c.len() as f64;
        for _ in c { out.push(m) }
    }
    out
}

/// Deterministic standard normals, so the table is reproducible.
fn normals(k: usize, seed: u64) -> Vec<(f64, f64)> {
    let mut st = seed;
    let mut next = || {
        st = st.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = st;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    };
    (0..k).map(|_| {
        let (u1, u2, u3, u4) = (next(), next(), next(), next());
        ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos(),
         (-2.0 * u3.ln()).sqrt() * (std::f64::consts::TAU * u4).cos())
    }).collect()
}

fn main() {
    let files: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with("--")).collect();
    let draws = normals(400, 0xC0FFEE);
    println!("{:<24} {:>7} {:>6} {:>5} {:>5} | {:>7} {:>7} {:>7} {:>7} | {:>7} {:>7} {:>7} {:>7}",
             "file", "dJ", "TV", "cvl1", "cvmx",
             "E[v.05]", "p05.05", "E[v.10]", "p05.10", "phase", "late1", "quant", "hold2");
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
        let (obj, ps) = match Profile::parse(&text) {
            Ok(p) => { set_trig_mode(p.trig); (p.obj, p.pitches) }
            Err(_) => (Objective { v0: V0, n: 0, lambda: 0.0 }, read_pitches(f)),
        };
        let obj = Objective { n: ps.len(), ..obj };
        let j0 = obj.j(&State { pos: Vec3::ZERO, vel: obj.v0 });
        let dj = |p: &[f64]| obj.eval(p) - j0;
        // dJ from a perturbed start: J is still measured against the *unperturbed* baseline, so
        // the number stays comparable across rows.
        let dj_v = |p: &[f64], dv: Vec3| {
            let mut s = State { pos: Vec3::ZERO, vel: obj.v0 + dv };
            for &q in p { s = ticked(&s, q) }
            obj.j(&s) - j0
        };
        let vel = |sigma: f64| -> (f64, f64) {
            let mut v: Vec<f64> = draws.iter()
                .map(|&(a, b)| dj_v(&ps, Vec3::new(0.0, sigma * a, sigma * b))).collect();
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            v.sort_by(f64::total_cmp);
            (mean, v[v.len() / 20])
        };
        let (m5, q5) = vel(0.05);
        let (m10, q10) = vel(0.10);
        let phase = { let m = 41;
            (0..m).map(|i| dj(&resample(&ps, i as f64 / (m - 1) as f64))).sum::<f64>() / m as f64 };
        let late = { let mut q = vec![ps[0]]; q.extend_from_slice(&ps[..ps.len() - 1]); dj(&q) };
        let quant: Vec<f64> = ps.iter().map(|p| (p * 10.0).round() / 10.0).collect();

        let name = std::path::Path::new(f).file_name().unwrap().to_string_lossy();
        println!("{:<24} {:>7.3} {:>6.0} {:>5.0} {:>5.0} | {:>7.3} {:>7.3} {:>7.3} {:>7.3} | \
{:>7.3} {:>7.3} {:>7.3} {:>7.3}",
                 name, dj(&ps), total_variation(&ps), curvature_l1(&ps), curvature_max(&ps),
                 m5, q5, m10, q10, phase, late, dj(&quant), dj(&block_hold(&ps, 2)));
    }
}
