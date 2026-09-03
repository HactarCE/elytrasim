//! Produce a corpus of optimal pitch schedules over a grid of `(n, lambda, vy0, vz0)`.
//!
//! The product is one file per cell: whitespace-separated pitches in degrees under a `#` header
//! that states the utility function and initial conditions completely. The point of a profile
//! is not where it came from -- it is that the pitches are observably a coordinate optimum of
//! the objective the header names. `sweep verify` re-checks exactly that, from the file alone.
//!
//! Subcommands:
//!   polish  <opts> [--out <file>]   solve one cell and print or write it
//!   verify  <file>...               re-certify profiles against their own headers
//!   bench   [--n <n>]               seconds per pass and per profile, cold and warm
//!   pilot   [--out <dir>]           the coarse grid: axis bounds, strides, seed quality
//!   run     --out <dir> [--shard vy=<i>,vz=<j>]      the full sweep
//!
//! Cell options: --n, --lambda, --vy, --vz, --passes, --trig, --init <file>

use elytrasim::opt::*;
use elytrasim::sim::*;
use rayon::prelude::*;
use std::time::Instant;

// ---------------------------------------------------------------- argument plumbing

struct Args(Vec<String>);

impl Args {
    fn new() -> Args { Args(std::env::args().collect()) }
    fn get(&self, k: &str) -> Option<&str> {
        self.0.iter().position(|a| a == k).and_then(|i| self.0.get(i + 1)).map(String::as_str)
    }
    fn num<T: std::str::FromStr>(&self, k: &str, d: T) -> T where T::Err: std::fmt::Debug {
        self.get(k).map_or(d, |v| v.parse().unwrap_or_else(|e| panic!("bad {k}: {e:?}")))
    }
    fn cell(&self) -> Objective {
        Objective {
            v0: Vec3::new(0.0, self.num("--vy", 0.167467), self.num("--vz", 0.200887)),
            n: self.num("--n", 300usize),
            lambda: self.num("--lambda", 0.0),
        }
    }
    fn opts(&self) -> PolishOpts {
        PolishOpts { max_passes: self.num("--passes", 200usize), ..Default::default() }
    }
}

fn cell_path(dir: &str, o: &Objective) -> String {
    format!("{dir}/vy{:+.4}_vz{:+.4}/n{:04}_lam{:+.6}.pitches", o.v0.y, o.v0.z, o.n, o.lambda)
}

fn write_profile(path: &str, p: &Profile) {
    if let Some(d) = std::path::Path::new(path).parent() { std::fs::create_dir_all(d).unwrap() }
    std::fs::write(path, p.to_string()).unwrap_or_else(|e| panic!("{path}: {e}"));
}

// ---------------------------------------------------------------- one cell

fn solve(obj: &Objective, init: &[f64], opts: PolishOpts) -> Profile {
    let r = polish(obj, init, opts);
    Profile {
        obj: *obj, trig: trig_mode(), commit: commit_hash().to_string(),
        pitches: r.pitches, residual: r.residual, passes: r.passes,
    }
}

fn cmd_polish_cell(a: &Args) {
    let obj = a.cell();
    let init = match a.get("--init") {
        Some(f) => {
            let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
            stretch(&Profile::parse(&text).map(|p| p.pitches).unwrap_or_else(|_| {
                text.lines().flat_map(|l| l.split('#').next().unwrap_or("").split_whitespace())
                    .map(|s| s.parse().unwrap()).collect()
            }), obj.n)
        }
        None => seed_from_policy(&obj),
    };
    let t = Instant::now();
    let p = solve(&obj, &init, a.opts());
    eprintln!("n {:>4}  lambda {:+.4}  v0 ({:.6}, {:.6})  J {:.6}  residual {:.2e}  {} passes  {:.1}s",
              obj.n, obj.lambda, obj.v0.y, obj.v0.z, obj.eval(&p.pitches), p.residual, p.passes,
              t.elapsed().as_secs_f64());
    match a.get("--out") {
        Some(f) => write_profile(f, &p),
        None => print!("{}", p.to_string()),
    }
}

fn cmd_verify(files: &[String]) {
    let mut bad = 0;
    for f in files {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
        let p = match Profile::parse(&text) { Ok(p) => p, Err(e) => { println!("{f}: BAD HEADER: {e}"); bad += 1; continue } };
        // the header is authoritative: replay under the physics it claims, not the shell's
        set_trig_mode(p.trig);
        let res = certify(&p.obj, &p.pitches, PolishOpts::default().global_step);
        let claimed = p.residual;
        let ok = res <= claimed.max(1e-6) * 1.5 + 1e-9;
        println!("{f}: n {:>4} lambda {:+.4} trig {} | claimed {:.2e}, found {:.2e}  {}",
                 p.obj.n, p.obj.lambda, p.trig, claimed, res, if ok { "ok" } else { "MISMATCH" });
        if !ok { bad += 1 }
    }
    if bad > 0 { eprintln!("{bad} of {} profiles failed", files.len()); std::process::exit(1) }
}

// ---------------------------------------------------------------- cost

fn cmd_bench(a: &Args) {
    let obj = a.cell();
    let step = a.num("--every", 10usize);
    let total = a.num("--passes", 80usize);
    eprintln!("trig {}  n {}  lambda {:+.4}  v0 ({:.4}, {:.4})",
              trig_mode(), obj.n, obj.lambda, obj.v0.y, obj.v0.z);

    let seeds: Vec<(&str, Vec<f64>)> = vec![
        ("policy", seed_from_policy(&obj)),
        ("reference", seed_from_reference(obj.n)),
        ("flat 0", vec![0.0; obj.n]),
    ];
    println!("{:>10} {:>7} {:>10} {:>10} {:>9} {:>8}", "seed", "passes", "J", "residual", "dy", "cum s");
    for (name, seed) in seeds {
        let mut cur = seed;
        let (mut done, mut secs) = (0usize, 0.0);
        println!("{name:>10} {:>7} {:>10.5} {:>10} {:>9.3} {:>8}",
                 0, obj.eval(&cur), "-", obj.replay(&cur).last().unwrap().pos.y, "-");
        while done < total {
            let t = Instant::now();
            let r = polish(&obj, &cur, PolishOpts { max_passes: step, ..Default::default() });
            secs += t.elapsed().as_secs_f64();
            done += r.passes;
            cur = r.pitches;
            println!("{name:>10} {done:>7} {:>10.5} {:>10.2e} {:>9.3} {secs:>8.1}",
                     r.j, r.residual, obj.replay(&cur).last().unwrap().pos.y);
            if r.passes < step { break }        // converged inside the block
        }
    }
}

// ---------------------------------------------------------------- the coarse sweep

fn cmd_pilot(a: &Args) {
    let dir = a.get("--out").unwrap_or("sweep-pilot").to_string();
    let passes = a.num("--passes", 60usize);
    let ns: Vec<usize> = a.get("--ns").map_or(vec![100, 200, 300, 400, 500],
        |s| s.split(',').map(|x| x.parse().unwrap()).collect());
    let lams: Vec<f64> = a.get("--lams").map_or(vec![-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0],
        |s| s.split(',').map(|x| x.parse().unwrap()).collect());
    let vys: Vec<f64> = a.get("--vys").map_or(vec![0.0, 0.1, 0.2], |s| s.split(',').map(|x| x.parse().unwrap()).collect());
    let vzs: Vec<f64> = a.get("--vzs").map_or(vec![0.0, 0.1, 0.2], |s| s.split(',').map(|x| x.parse().unwrap()).collect());

    let mut cells: Vec<Objective> = vec![];
    for &n in &ns { for &lambda in &lams { for &vy in &vys { for &vz in &vzs {
        cells.push(Objective { v0: Vec3::new(0.0, vy, vz), n, lambda });
    }}}}
    eprintln!("pilot: {} cells, {passes} passes each, trig {}", cells.len(), trig_mode());

    println!("{:>5} {:>8} {:>8} {:>8} {:>10} {:>10} {:>9} {:>9} {:>10} {:>10} {:>8}",
             "n", "lambda", "vy0", "vz0", "J_seed", "J_opt", "dy", "dz", "residual", "close", "passes");
    let t0 = Instant::now();
    let rows: Vec<String> = cells.par_iter().map(|obj| {
        let seed = seed_from_policy(obj);
        let j_seed = obj.eval(&seed);
        let p = solve(obj, &seed, PolishOpts { max_passes: passes, ..Default::default() });
        let st = obj.replay(&p.pitches);
        let sn = st.last().unwrap();
        write_profile(&cell_path(&dir, obj), &p);
        format!("{:>5} {:>8.3} {:>8.4} {:>8.4} {j_seed:>10.4} {:>10.4} {:>9.3} {:>9.2} {:>10.2e} {:>10.2e} {:>8}",
                obj.n, obj.lambda, obj.v0.y, obj.v0.z, obj.eval(&p.pitches),
                sn.pos.y, sn.pos.z, p.residual, (sn.vel - obj.v0).length(), p.passes)
    }).collect();
    for r in rows { println!("{r}") }
    eprintln!("pilot: {:.1}s total", t0.elapsed().as_secs_f64());
}

// ---------------------------------------------------------------- main

fn main() {
    let mut a = Args::new();
    if let Some(i) = a.0.iter().position(|x| x == "--trig") {
        set_trig_mode(a.0[i + 1].parse().unwrap_or_else(|e: String| panic!("{e}")));
        a.0.drain(i..=i + 1);
    }
    match a.0.get(1).map(String::as_str) {
        Some("polish") => cmd_polish_cell(&a),
        Some("verify") => cmd_verify(&a.0[2..].iter().filter(|s| !s.starts_with("--")).cloned().collect::<Vec<_>>()),
        Some("bench") => cmd_bench(&a),
        Some("pilot") => cmd_pilot(&a),
        _ => eprintln!("{}", "usage: sweep <polish|verify|bench|pilot|run> ...\n\
                             see the module docs at the top of src/bin/sweep.rs"),
    }
}
