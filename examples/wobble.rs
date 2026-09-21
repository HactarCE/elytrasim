//! Write a schedule out as a *closed* profile, optionally perturbed first.
//!
//!     wobble <file> <amp deg> [w]   > out.pitches
//!
//! Adds one cycle of a sinusoid of amplitude `amp` to the pitches, then iterates the schedule
//! until it sits on its own limit cycle and states that velocity in the header. `amp = 0` is
//! the useful degenerate case: it turns a bare list of pitches -- a `cyclecut`, say -- into a
//! profile that closes, which is what `myopic gain` and `myopic adjoint` need before the
//! periodic costate means anything. `w` only lands in the header; it does not change the
//! schedule.
//!
//! Note the header states `w` directly; `Profile` derives its own `w` from `lambda`, so pass
//! the price on the subcommand's own argument rather than trusting this header to carry it.
use elytrasim::opt::*;
use elytrasim::sim::*;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let base = read_pitches(&a[1]);
    let amp: f64 = a[2].parse().unwrap();
    let n = base.len();
    let ps: Vec<f64> = base.iter().enumerate()
        .map(|(t, &p)| p + amp * (std::f64::consts::TAU * t as f64 / n as f64).sin()).collect();
    let mut v = V0;
    for _ in 0..200 { for &p in &ps { v = update_fall_flying_movement(v, rot(p)) } }
    println!("# elytrasim optimal pitch schedule");
    println!("# lambda 0\n# w {}\n# v0 {:.9} {:.9}   # vy vz\n# n {n}\n# trig libm\n\
              # flight reference\n# structure cyclic",
             a.get(3).map_or("0", |s| s.as_str()), v.y, v.z);
    for p in ps { println!("{p}") }
}
