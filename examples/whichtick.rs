//! Which single tick is the schedule fragile at?
//!
//! For each tick, nudge only that pitch by +-delta and report the worst loss in `dJ`. A schedule
//! that is honestly robust shows a smooth, small profile; a schedule sitting on a cliff shows one
//! or two ticks with a loss of blocks. usage: whichtick <file> <delta> [--trig m] [--top k]
use elytrasim::opt::*;
use elytrasim::sim::*;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // --trig, if given, overrides every profile header for every file in this invocation.
    let forced: Option<TrigMode> =
        a.iter().position(|x| x == "--trig").map(|i| a[i + 1].parse().unwrap());
    let f = &a[1];
    let d: f64 = a[2].parse().unwrap();
    let top: usize = a.iter().position(|x| x == "--top").map(|i| a[i + 1].parse().unwrap()).unwrap_or(12);
    let text = std::fs::read_to_string(f).unwrap();
    let (obj, ps) = {
        // Each file picks its own mode from its own header, and an explicit --trig overrides
        // every header. Setting the mode inside the loop without resetting it let a
        // headerless file silently inherit the previous profile's physics.
        let parsed = Profile::parse(&text);
        set_trig_mode(forced.unwrap_or_else(|| parsed.as_ref().map(|p| p.trig).unwrap_or_default()));
        match parsed {
            Ok(p) => (p.obj, p.pitches),
            Err(_) => (Objective { v0: V0, n: 0, lambda: 0.0 }, read_pitches(f)),
        }
    };
    let obj = Objective { n: ps.len(), ..obj };
    let base = obj.eval(&ps);
    let mut worst: Vec<(f64, usize, f64)> = (0..ps.len()).map(|t| {
        let mut q = ps.clone();
        let mut w = 0.0f64;
        for s in [-1.0, 1.0] {
            q[t] = (ps[t] + s * d).clamp(-90.0, 90.0);
            w = w.max(base - obj.eval(&q));
        }
        (w, t, ps[t])
    }).collect();
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("# {} delta {d} trig {}  dJ {:.4}", f.rsplit('/').next().unwrap(), trig_mode(),
             base - obj.j(&State { pos: Vec3::ZERO, vel: obj.v0 }));
    println!("# worst single-tick losses, blocks");
    for (w, t, p) in worst.iter().take(top) { println!("  tick {t:>4}  pitch {p:>10.5}  loses {w:>9.4}") }
    let s: f64 = worst.iter().map(|x| x.0.max(0.0)).sum();
    println!("  total over all ticks: {s:.3}");
}
