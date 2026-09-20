//! Steady-state polish against the single-cycle optimum, on one common yardstick.
//!
//! The two schedules are optimal for different `v0`, so their headline `dy` are not comparable.
//! The yardstick that is: fly each schedule at *its own* fixed point and measure one lap there.
//! That is the number a steady-state flier actually gets, and both modes can be scored on it.
//!
//! The `--control` column re-polishes the seed with `steady` off and the same pass budget, so a
//! gain cannot be confused with simply having spent more passes.
//!
//! usage: ss_compare <file>... [--passes 200] [--out DIR]
use elytrasim::opt::*;
use elytrasim::sim::*;

/// One lap from `v`, as (dTE, dy, dz).
fn lap(v: Vec3, ps: &[f64]) -> (f64, f64, f64) {
    let mut s = State { pos: Vec3::ZERO, vel: v };
    let te0 = s.total_energy();
    for &p in ps { s = ticked(&s, p) }
    (s.total_energy() - te0, s.pos.y, s.pos.z)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let flag = |k: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone());
    let passes: usize = flag("--passes").map(|v| v.parse().unwrap()).unwrap_or(200);
    let out = flag("--out");
    let mut files: Vec<&String> = Vec::new();
    let mut i = 1;
    while i < a.len() { if a[i].starts_with("--") { i += 2 } else { files.push(&a[i]); i += 1 } }

    println!("file,n,lambda,base_dte_v0,base_dte_ss,ctrl_dte_ss,ss_dte_ss,ss_dy,ss_dz,\
              ss_v0y,ss_v0z,base_ssy,base_ssz,passes,dv0,lag1_base,lag1_ss,\
              base_seam,ss_seam,ss_maxd");
    for f in &files {
        let text = std::fs::read_to_string(f.as_str()).unwrap();
        let pr = Profile::parse(&text).unwrap();
        set_trig_mode(pr.trig); set_flight_mode(pr.flight);
        let obj = pr.obj;
        let opts = PolishOpts { max_passes: passes, jitter: pr.jitter, rough: pr.rough,
                                ..Default::default() };

        // Baseline: the corpus optimum, scored at its declared v0 and at its own fixed point.
        let base_ss = steady_vel(&pr.pitches, obj.v0);
        let base_dte_v0 = lap(obj.v0, &pr.pitches).0;
        let base_dte_ss = lap(base_ss, &pr.pitches).0;

        // Control: same seed, same budget, steady off.
        let ctrl = polish(&obj, &pr.pitches, opts);
        let ctrl_ss = steady_vel(&ctrl.pitches, obj.v0);
        let ctrl_dte_ss = lap(ctrl_ss, &ctrl.pitches).0;

        // Steady: same seed, v0 re-solved after every pass.
        let sp = polish(&obj, &pr.pitches, PolishOpts { steady: true, ..opts });
        let (dte, dy, dz) = lap(sp.v0, &sp.pitches);

        // The seam is `p[n-1] -> p[0]`, the move a flier would have to make to fly this
        // schedule again. Nothing in the objective prices it -- the corpus optimizes one cycle
        // -- so this is a diagnostic, not a term. Reported next to the sharpest interior move,
        // because "98 degrees" only means something against what the schedule already does.
        let seam = |v: &[f64]| (v[0] - v[v.len() - 1]).abs();
        let maxd = |v: &[f64]| v.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f64::max);

        let name = std::path::Path::new(f.as_str()).file_name().unwrap().to_string_lossy();
        println!("{name},{},{},{base_dte_v0:.6},{base_dte_ss:.6},{ctrl_dte_ss:.6},{dte:.6},\
                  {dy:.6},{dz:.6},{:.9},{:.9},{:.9},{:.9},{},{:.3e},{:.4},{:.4},\
                  {:.3},{:.3},{:.3}",
                 obj.n, obj.lambda, sp.v0.y, sp.v0.z, base_ss.y, base_ss.z,
                 sp.passes, sp.dv0, lag1(&pr.pitches), lag1(&sp.pitches),
                 seam(&pr.pitches), seam(&sp.pitches), maxd(&sp.pitches));

        if let Some(d) = &out {
            std::fs::create_dir_all(d).unwrap();
            let stem = name.trim_end_matches(".pitches");
            let dump = |tag: &str, ps: &[f64], v: Vec3| {
                let st = replay_from(v, ps);
                let mut o = String::from("tick,pitch,vy,vz,y,z\n");
                for (t, p) in ps.iter().enumerate() {
                    o += &format!("{t},{p:.6},{:.6},{:.6},{:.6},{:.6}\n",
                                  st[t].vel.y, st[t].vel.z, st[t].pos.y, st[t].pos.z);
                }
                std::fs::write(format!("{d}/{stem}.{tag}.csv"), o).unwrap();
            };
            dump("base", &pr.pitches, base_ss);
            dump("steady", &sp.pitches, sp.v0);
        }
    }
}
