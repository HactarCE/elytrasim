//! Pitch sensitivity: what does a schedule score when every pitch is nudged by a little?
//!
//! Chatter is one symptom of overfitting. Hypersensitivity to pitch is another, and a schedule
//! can be perfectly smooth and still be tuned to a control precision no input device has. Minecraft
//! rotation arrives in steps of about 0.15 * sensitivity degrees, so anything that needs better
//! than a tenth of a degree is not flyable regardless of how smooth it looks.
//!
//! For each amplitude, one row: mean and 5th percentile of dJ over `k` draws of uniform noise
//! added to every pitch, plus the deterministic "round every pitch to this grid" number.
//!
//! usage: sens <file>...
use elytrasim::opt::*;
use elytrasim::sim::*;

fn rng(seed: u64) -> impl FnMut() -> f64 {
    let mut st = seed;
    move || {
        st = st.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = st;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let forced = argv.iter().position(|x| x == "--trig").map(|i| argv[i + 1].parse().unwrap());
    let files: Vec<String> = std::env::args().skip(1)
        .filter(|a| !a.starts_with("--")).collect();
    let files: Vec<String> = { let mut f = files; if let Some(i) = argv.iter().position(|x| x == "--trig") { f.retain(|x| *x != argv[i+1]); } f };
    let amps = [1e-4, 1e-3, 1e-2, 0.05, 0.15, 0.5];
    print!("{:<24} {:>7}", "file", "dJ");
    for a in amps { print!("  {:>8} {:>8} {:>8}", format!("m{a}"), format!("p05{a}"), format!("q{a}")) }
    println!();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
        let (obj, ps) = match Profile::parse(&text) {
            Ok(p) => { set_trig_mode(p.trig); set_flight_mode(p.flight); (p.obj, p.pitches) }
            Err(_) => (Objective { v0: V0, n: 0, lambda: 0.0 }, read_pitches(f)),
        };
        if let Some(m) = forced { set_trig_mode(m) }
        let obj = Objective { n: ps.len(), ..obj };
        let j0 = obj.j(&State { pos: Vec3::ZERO, vel: obj.v0 });
        let dj = |p: &[f64]| obj.eval(p) - j0;
        print!("{:<24} {:>7.3}", std::path::Path::new(f).file_name().unwrap().to_string_lossy(), dj(&ps));
        for a in amps {
            let mut u = rng(0xBEEF);
            let mut v: Vec<f64> = (0..200).map(|_| {
                let q: Vec<f64> = ps.iter().map(|&p| (p + a * u()).clamp(-90.0, 90.0)).collect();
                dj(&q)
            }).collect();
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            v.sort_by(f64::total_cmp);
            let q: Vec<f64> = ps.iter().map(|&p| (p / a).round() * a).collect();
            print!("  {:>8.3} {:>8.3} {:>8.3}", mean, v[v.len() / 20], dj(&q));
        }
        println!();
    }
}
