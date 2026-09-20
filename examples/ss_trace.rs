//! Per-pass trace of the outer alternation: does `v0` walk in, or ring?
//!
//! Mirrors `cycle-optimizer`'s `optimization_step` literally -- one sweep at a frozen `v0`, then
//! re-solve `v0` to convergence -- so the trace is of the real outer loop, not an approximation.
use elytrasim::opt::*;
use elytrasim::sim::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let flag = |k: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone());
    let n: usize = flag("--passes").map(|v| v.parse().unwrap()).unwrap_or(400);
    let f = a[1].clone();
    let text = std::fs::read_to_string(&f).unwrap();
    let pr = Profile::parse(&text).unwrap();
    set_trig_mode(pr.trig); set_flight_mode(pr.flight);

    let mut obj = pr.obj;
    let mut ps = pr.pitches.clone();
    obj.v0 = steady_vel(&ps, obj.v0);
    // `polish` picks its draws by its own pass index, which is always 0 when it is called one
    // pass at a time. To reproduce the real loop's resampling the seed has to be walked here.
    let resample = a.iter().any(|x| x == "--resample");
    let opts = PolishOpts { max_passes: 1, tol: f64::NEG_INFINITY, stall_window: usize::MAX,
                            jitter: pr.jitter, rough: pr.rough, ..Default::default() };
    println!("pass,dv0,v0y,v0z,dte_ss,lag1");
    for pass in 0..n {
        let mut opts = opts;
        if resample { opts.jitter.seed = pr.jitter.seed.wrapping_add(pass as u64) }
        let r = polish(&obj, &ps, opts);
        ps = r.pitches;
        let next = steady_vel(&ps, obj.v0);
        let d = dv_l1(next, obj.v0);
        obj.v0 = next;
        let mut s = State { pos: Vec3::ZERO, vel: obj.v0 };
        let te0 = s.total_energy();
        for &p in &ps { s = ticked(&s, p) }
        println!("{pass},{d:.6e},{:.9},{:.9},{:.6},{:.4}",
                 obj.v0.y, obj.v0.z, s.total_energy() - te0, lag1(&ps));
    }
}
