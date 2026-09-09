//! Fly the same schedule several times in a row, each cycle starting where the last one ended.
//!
//! The corpus optimizes one cycle with the terminal velocity free, so nothing in the objective
//! asks a schedule to be *repeatable*. That is a place an answer can be quietly overfit without
//! chattering and without touching a gate: a schedule that ends 0.08 blocks/tick off its own
//! start is a fine single cycle and may be a bad second one. This measures it -- the climb per
//! repetition, the drift in the state it hands on, and whether it converges to a limit cycle or
//! falls apart.
//!
//! usage: repeat <file>... [--reps k]
use elytrasim::opt::*;
use elytrasim::sim::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let reps: usize = a.iter().position(|x| x == "--reps")
        .map(|i| a[i + 1].parse().unwrap()).unwrap_or(6);
    // Drop each flag *and the value after it*, or the value gets treated as a filename.
    let mut files: Vec<&String> = Vec::new();
    let mut i = 1;
    while i < a.len() {
        if a[i].starts_with("--") { i += 2 } else { files.push(&a[i]); i += 1 }
    }
    let forced = a.iter().position(|x| x == "--trig").map(|i| a[i + 1].parse().unwrap());

    print!("{:<24}", "file");
    for k in 1..=reps { print!("  {:>7}", format!("dy#{k}")) }
    // The sum of the two components' absolute changes, not a Euclidean norm -- it is a closure
    // *test* (zero or not), and calling it |dv| invited reading it as a speed difference.
    println!("   {:>9} {:>9}", "sum|dv|1", "sum|dv|N");
    for f in &files {
        let text = std::fs::read_to_string(f.as_str()).unwrap();
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
        let v0 = obj.v0;
        let mut s = State { pos: Vec3::ZERO, vel: v0 };
        print!("{:<24}", std::path::Path::new(f.as_str()).file_name().unwrap().to_string_lossy());
        let (mut d1, mut dl) = (0.0, 0.0);
        for k in 0..reps {
            let (y0, vin) = (s.pos.y, s.vel);
            for &p in &ps { s = ticked(&s, p) }
            print!("  {:>7.3}", s.pos.y - y0);
            let d = (s.vel.y - vin.y).abs() + (s.vel.z - vin.z).abs();  // L1, a closure test
            if k == 0 { d1 = d }
            dl = d;
        }
        println!("   {d1:>9.4} {dl:>9.4}");
    }
}
