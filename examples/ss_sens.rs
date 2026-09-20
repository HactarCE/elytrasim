//! What does a residual `dv0` cost in blocks? Turns the steady-state stop threshold into a number.
use elytrasim::opt::*;
use elytrasim::sim::*;
fn main() {
    println!("{:<20} {:>10} {:>12} {:>12} {:>12}", "file", "dv0", "dTE_change", "per_unit", "blocks@1e-3");
    for f in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&f).unwrap();
        let pr = Profile::parse(&text).unwrap();
        set_trig_mode(pr.trig); set_flight_mode(pr.flight);
        let v = steady_vel(&pr.pitches, pr.obj.v0);
        let te = |v: Vec3| {
            let mut s = State { pos: Vec3::ZERO, vel: v };
            let t0 = s.total_energy();
            for &p in &pr.pitches { s = ticked(&s, p) }
            s.total_energy() - t0
        };
        let base = te(v);
        for d in [1e-3, 1e-2] {
            // split the L1 budget across both components, worst case same sign
            let p = Vec3::new(0.0, v.y + d / 2.0, v.z + d / 2.0);
            let ch = (te(p) - base).abs();
            if d == 1e-3 {
                println!("{:<20} {d:>10.0e} {ch:>12.6} {:>12.3} {ch:>12.5}",
                         std::path::Path::new(&f).file_stem().unwrap().to_string_lossy(),
                         ch / d);
            } else {
                println!("{:<20} {d:>10.0e} {ch:>12.6} {:>12.3} {:>12}", "", ch / d, "");
            }
        }
    }
}
