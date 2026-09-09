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
    if let Some(i) = a.iter().position(|x| x == "--trig") { set_trig_mode(a[i + 1].parse().unwrap()) }
    let forced = a.iter().position(|x| x == "--trig").map(|i| a[i + 1].parse().unwrap());

    print!("{:<24}", "file");
    for k in 1..=reps { print!("  {:>7}", format!("dy#{k}")) }
    println!("   {:>8} {:>8}", "|dv| 1st", "|dv| last");
    for f in &files {
        let text = std::fs::read_to_string(f.as_str()).unwrap();
        let (obj, ps) = match Profile::parse(&text) {
            Ok(p) => { set_trig_mode(p.trig); (p.obj, p.pitches) }
            Err(_) => (Objective { v0: V0, n: 0, lambda: 0.0 }, read_pitches(f)),
        };
        if let Some(m) = forced { set_trig_mode(m) }
        let v0 = obj.v0;
        let mut s = State { pos: Vec3::ZERO, vel: v0 };
        print!("{:<24}", std::path::Path::new(f.as_str()).file_name().unwrap().to_string_lossy());
        let (mut d1, mut dl) = (0.0, 0.0);
        for k in 0..reps {
            let (y0, vin) = (s.pos.y, s.vel);
            for &p in &ps { s = ticked(&s, p) }
            print!("  {:>7.3}", s.pos.y - y0);
            let d = (s.vel.y - vin.y).abs() + (s.vel.z - vin.z).abs();
            if k == 0 { d1 = d }
            dl = d;
        }
        println!("   {d1:>8.4} {dl:>8.4}");
    }
}
