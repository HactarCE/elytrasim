//! The per-tick objective curve: J of the whole schedule as a function of the pitch at one
//! tick, everything else held. That is exactly the discrete Hamiltonian -- the exact tail
//! value of the state this tick lands in -- so its shape says what kind of optimum this is.
//!
//! A normal arc has one interior peak. A *singular* arc has two peaks of nearly equal height,
//! and then the best thing a discrete-time controller can do is alternate between them at the
//! tick rate, which is precisely the chatter. Printing the curve settles which one this is.
//!
//! usage: hamiltonian <file> <t0> <t1> [--smooth k]
//!   --smooth k   first replace the schedule with its k-tick box-filtered version, so the
//!                question is whether chatter is *created* from a smooth schedule rather than
//!                merely self-consistent once present.
use elytrasim::opt::*;
use elytrasim::sim::*;

fn box_filter(p: &[f64], k: usize) -> Vec<f64> {
    if k <= 1 { return p.to_vec() }
    let n = p.len() as isize;
    let h = (k / 2) as isize;
    (0..n).map(|i| {
        let (mut s, mut c) = (0.0, 0.0);
        for d in -h..=h {
            let j = (i + d).clamp(0, n - 1) as usize;
            s += p[j]; c += 1.0;
        }
        s / c
    }).collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    // --trig, if given, overrides every profile header for every file in this invocation.
    let forced: Option<TrigMode> =
        a.iter().position(|x| x == "--trig").map(|i| a[i + 1].parse().unwrap());
    let f = &a[1];
    let (t0, t1): (usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let smooth: usize = a.iter().position(|x| x == "--smooth")
        .map(|i| a[i + 1].parse().unwrap()).unwrap_or(1);

    let text = std::fs::read_to_string(f).unwrap();
    let (obj, ps) = {
        // Each file picks its own mode from its own header, and an explicit --trig overrides
        // every header. Setting the mode inside the loop without resetting it let a
        // headerless file silently inherit the previous profile's physics.
        let parsed = Profile::parse(&text);
        set_trig_mode(forced.unwrap_or_else(|| parsed.as_ref().map(|p| p.trig).unwrap_or_default()));
        set_flight_mode(parsed.as_ref().map(|p| p.flight).unwrap_or_default());
        match parsed {
            Ok(p) => (p.obj, p.pitches),
            Err(_) => (Objective { v0: V0, n: 0, lambda: 0.0 }, read_pitches(f)),
        }
    };
    let ps = box_filter(&ps, smooth);
    let obj = Objective { n: ps.len(), ..obj };
    let st = obj.replay(&ps);

    // one row per pitch, one column per tick: J(schedule with pitches[t] := p)
    print!("pitch");
    for t in t0..t1 { print!(",t{t}") }
    println!();
    let mut p = -90.0;
    while p <= 90.0 + 1e-9 {
        print!("{p:.2}");
        for t in t0..t1 {
            let mut s = ticked(&st[t], p);
            for &q in &ps[t + 1..] { s = ticked(&s, q) }
            print!(",{:.6}", obj.j(&s));
        }
        println!();
        p += 0.25;
    }
    eprintln!("schedule pitches at those ticks:");
    eprint!("cur");
    for t in t0..t1 { eprint!(",{:.2}", ps[t]) }
    eprintln!();
}
