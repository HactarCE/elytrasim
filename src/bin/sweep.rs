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
//! Cell options: --n, --lambda, --vy, --vz, --passes, --tol, --trig, --init <file>
//!
//! Jitter options: --jitter <sigma>, --draws <k>, --fixed-draws, --seed <s>
//!   `--jitter` is the standard deviation of the *initial velocity* error, in blocks/tick,
//!   applied independently to v_y and v_z. It is the regularizer: polished against one exact
//!   starting state the optimizer converges onto knife edges in the state and chatters between
//!   0 and 90 degrees; polished against a spread of starting states it cannot, because the
//!   draws are never on the edge at the same tick. Perturbing the pitches instead does not
//!   work and was measured -- see `Jitter` in `opt.rs`.

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
            // Zero, not the reference cycle's start: (0.167467, 0.200887) is the velocity
            // REPLAY_PITCHES_300 happens to close on, which makes it special for that one
            // profile and for nothing else.
            v0: Vec3::new(0.0, self.num("--vy", 0.0), self.num("--vz", 0.0)),
            n: self.num("--n", 300usize),
            lambda: self.num("--lambda", 0.0),
        }
    }
    fn opts(&self) -> PolishOpts {
        PolishOpts {
            max_passes: self.num("--passes", 200usize),
            tol: self.num("--tol", PolishOpts::default().tol),
            block: self.num("--block", PolishOpts::default().block),
            lag1_floor: self.num("--lag1-floor", PolishOpts::default().lag1_floor),
            jitter: Jitter {
                sigma: self.num("--jitter", 0.0),
                draws: self.num("--draws", 8usize),
                resample: !self.0.iter().any(|x| x == "--fixed-draws"),
                seed: self.num("--seed", Jitter::default().seed),
            },
            ..Default::default()
        }
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

/// Returns the profile and the polish record: the record carries *why* the polish stopped,
/// which the file deliberately does not (the file states the objective, not the provenance).
fn solve(obj: &Objective, init: &[f64], opts: PolishOpts) -> (Profile, Polished) {
    let r = polish(obj, init, opts);
    let profile = Profile {
        obj: *obj, trig: trig_mode(), jitter: opts.jitter,
        commit: commit_hash().to_string(),
        pitches: r.pitches.clone(), residual: r.residual, passes: r.passes,
    };
    (profile, r)
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
    let (p, r) = solve(&obj, &init, a.opts());
    eprintln!("n {:>4}  lambda {:+.4}  v0 ({:.6}, {:.6})  J {:.6}  residual {:.2e}  \
lag1 {:+.3}  {} passes{}  {:.1}s",
              obj.n, obj.lambda, obj.v0.y, obj.v0.z, obj.eval(&p.pitches), p.residual,
              lag1(&p.pitches), p.passes, if r.stopped_degenerate { " (stopped: degenerate)" } else { "" },
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
        let res = certify(&p.obj, &p.pitches, PolishOpts::default().global_step, p.jitter);
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
    // --vels gives explicit (vy:vz) points; --vys/--vzs give their outer product. The critical
    // points of the velocity box are its four corners, its centre, and the reference operating
    // point, which is not a corner and is the only one with a known answer.
    let vels: Vec<(f64, f64)> = match a.get("--vels") {
        Some(s) => s.split(',').map(|p| {
            let (y, z) = p.split_once(':').unwrap_or_else(|| panic!("--vels wants vy:vz, got {p:?}"));
            (y.parse().unwrap(), z.parse().unwrap())
        }).collect(),
        None => {
            let f = |k: &str| -> Vec<f64> {
                a.get(k).map_or(vec![0.0, 0.1, 0.2], |s| s.split(',').map(|x| x.parse().unwrap()).collect())
            };
            let (vys, vzs) = (f("--vys"), f("--vzs"));
            vys.iter().flat_map(|&y| vzs.iter().map(move |&z| (y, z))).collect()
        }
    };

    let mut cells: Vec<Objective> = vec![];
    for &n in &ns { for &lambda in &lams { for &(vy, vz) in &vels {
        cells.push(Objective { v0: Vec3::new(0.0, vy, vz), n, lambda });
    }}}
    eprintln!("pilot: {} cells, {passes} passes each, trig {}", cells.len(), trig_mode());

    println!("{:>5} {:>8} {:>8} {:>8} {:>10} {:>10} {:>9} {:>9} {:>10} {:>10} {:>8}",
             "n", "lambda", "vy0", "vz0", "J_seed", "J_opt", "dy", "dz", "residual", "close", "passes");
    let t0 = Instant::now();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let total = cells.len();
    let rows: Vec<String> = cells.par_iter().map(|obj| {
        let seed = seed_from_policy(obj);
        let j_seed = obj.eval(&seed);
        let (p, _) = solve(obj, &seed, PolishOpts { max_passes: passes, ..Default::default() });
        let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if k % 10 == 0 || k == total {
            eprintln!("  {k}/{total} cells, {:.0}s elapsed", t0.elapsed().as_secs_f64());
        }
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

// ---------------------------------------------------------------- the full sweep

/// The grid. Kept in one place so `run`, `manifest` and the resume check cannot disagree.
struct Grid { ns: Vec<usize>, lams: Vec<f64>, vys: Vec<f64>, vzs: Vec<f64> }

impl Grid {
    fn from(a: &Args) -> Grid {
        let f = |k: &str, d: Vec<f64>| -> Vec<f64> {
            a.get(k).map_or(d, |s| s.split(',').map(|x| x.parse().unwrap()).collect())
        };
        Grid {
            ns: a.get("--ns").map_or((100..=500).step_by(10).collect(),
                |s| s.split(',').map(|x| x.parse().unwrap()).collect()),
            lams: f("--lams", (-20..=20).map(|i| i as f64 * 0.05).collect()),
            vys: f("--vys", (0..=10).map(|i| i as f64 * 0.02).collect()),
            vzs: f("--vzs", (0..=10).map(|i| i as f64 * 0.02).collect()),
        }
    }
    /// Index of the value nearest `x`, for placing the anchor.
    fn nearest(v: &[f64], x: f64) -> usize {
        (0..v.len()).min_by(|&i, &j| {
            (v[i] - x).abs().partial_cmp(&(v[j] - x).abs()).unwrap()
        }).unwrap()
    }
}

/// Solve one `(vy0, vz0)` shard: anchor at the cell nearest the reference operating point, then
/// breadth-first outward over the `(n, lambda)` plane, each cell warm-started from the solved
/// neighbor that reached it. Continuation is the whole speedup, and it also keeps the heuristic
/// seed out of the parts of the grid where it has no reason to work -- it is asked once, at the
/// anchor, and never again.
fn run_shard(dir: &str, g: &Grid, vy: f64, vz: f64, opts: PolishOpts, force: bool,
             anchor: Option<&[f64]>) -> (usize, usize) {
    let ns_f: Vec<f64> = g.ns.iter().map(|&n| n as f64).collect();
    let (ni, li) = (Grid::nearest(&ns_f, 300.0), Grid::nearest(&g.lams, 0.0));
    let (nn, nl) = (g.ns.len(), g.lams.len());
    let cell = |i: usize, j: usize| Objective {
        v0: Vec3::new(0.0, vy, vz), n: g.ns[i], lambda: g.lams[j],
    };

    let mut solved: Vec<Option<Vec<f64>>> = vec![None; nn * nl];
    let mut queue = std::collections::VecDeque::from([(ni, li, None::<(usize, usize)>)]);
    let mut seen = vec![false; nn * nl];
    seen[ni * nl + li] = true;
    let (mut done, mut skipped) = (0, 0);

    while let Some((i, j, from)) = queue.pop_front() {
        let obj = cell(i, j);
        let path = cell_path(dir, &obj);
        // Warm start from whichever solved neighbor got here first; the anchor has none.
        // The anchor is the one cell with no solved neighbour. `seed_from_policy` was tuned
        // against the reference cycle's operating point, so away from it -- v0 = 0 especially --
        // an explicit anchor schedule is the difference between seeding the cyclic branch and
        // seeding a collapsed one, and the BFS propagates whichever it gets to the whole shard.
        // Two candidate seeds, and the cell takes whichever actually scores better. Both stay
        // inside the single-cycle family: the corpus sweeps one cycle across num_ticks, so a
        // seed that tiles the cycle is not a cheaper route to the answer, it is a different
        // and degenerate answer.
        //
        // The BFS parent carries the branch -- along lambda, and along n below the anchor where
        // it holds the cyclic regime a cold seed collapses out of. The anchor rescaled to
        // period n is the cold fallback, and wins where the chain has drifted.
        let parent = from.and_then(|(a, b)| solved[a * nl + b].clone())
            .map(|prev| stretch(&prev, obj.n));
        let mut cands: Vec<Vec<f64>> = parent.into_iter().collect();
        if let Some(a) = anchor {
            cands.push(rescale(a, obj.n));      // one cycle of period n
        }
        if cands.is_empty() { cands.push(seed_from_policy(&obj)) }
        // Score the candidates *after a short polish*, not before it. Judged cold the BFS
        // parent always wins -- it is a solved schedule and the others are not -- but going up
        // in n it then polishes straight into chatter, while a tiled seed that starts lower
        // ends far higher. Judged cold, the n axis came back with a band from 350 to 530 losing
        // 0.71 blocks per 10 ticks at lag-1 -0.87, with n = 530 scoring 6.94 where n = 540,
        // which the tiled seed reached, scored 16.56.
        let init = if cands.len() == 1 {
            cands.pop().unwrap()
        } else {
            let probe = PolishOpts { max_passes: 2, tol: 0.0, ..opts };
            cands.into_iter()
                .map(|c| { let r = polish(&obj, &c, probe); (r.j, r.pitches) })
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
                .unwrap().1
        };
        // Resume: a cell whose file already matches this objective and physics is not redone,
        // but its pitches still seed the neighbours, so a killed job costs one cell.
        let existing = (!force).then(|| std::fs::read_to_string(&path).ok()).flatten()
            .and_then(|t| Profile::parse(&t).ok())
            .filter(|p| p.obj == obj && p.trig == trig_mode() && p.pitches.len() == obj.n);
        let pitches = match existing {
            Some(p) => { skipped += 1; p.pitches }
            None => {
                let (prof, _) = solve(&obj, &init, opts);
                write_profile(&path, &prof);
                done += 1;
                if done % 25 == 0 { eprintln!("  vy{vy:+.3} vz{vz:+.3}: {done} solved, {skipped} resumed") }
                prof.pitches
            }
        };
        solved[i * nl + j] = Some(pitches);

        for (di, dj) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (a, b) = (i as i64 + di, j as i64 + dj);
            if a < 0 || b < 0 || a as usize >= nn || b as usize >= nl { continue }
            let (a, b) = (a as usize, b as usize);
            if std::mem::replace(&mut seen[a * nl + b], true) { continue }
            queue.push_back((a, b, Some((i, j))));
        }
    }
    (done, skipped)
}

fn write_manifest(dir: &str, g: &Grid, opts: PolishOpts) {
    let list = |v: &[f64]| v.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(", ");
    let text = format!(
"{{
  \"commit\": \"{}\",
  \"trig\": \"{}\",
  \"objective\": \"J = TE(s_n) + w*z_n, TE in blocks (KE = |v|^2/(2g), PE = y), v_n free\",
  \"y_ref\": {Y_REF}, \"z_ref\": {Z_REF},
  \"axes\": {{
    \"n\": [{}],
    \"lambda\": [{}],
    \"vy0\": [{}],
    \"vz0\": [{}]
  }},
  \"cells\": {},
  \"polish\": {{ \"max_passes\": {}, \"global_every\": {}, \"global_step\": {}, \"local_span\": {}, \"local_step\": {}, \"tol\": {} }},
  \"fingerprint\": \"{:016x}\"
}}
", commit_hash(), trig_mode(),
   g.ns.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "),
   list(&g.lams), list(&g.vys), list(&g.vzs),
   g.ns.len() * g.lams.len() * g.vys.len() * g.vzs.len(),
   opts.max_passes, opts.global_every, opts.global_step, opts.local_span, opts.local_step, opts.tol,
   physics_fingerprint());
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/manifest.json"), text).unwrap();
}

fn cmd_run(a: &Args) {
    let dir = a.get("--out").unwrap_or("sweep").to_string();
    let g = Grid::from(a);
    let opts = a.opts();
    let force = a.0.iter().any(|x| x == "--force");
    let anchor: Option<Vec<f64>> = a.get("--anchor").map(|f| {
        let t = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
        Profile::parse(&t).map(|p| p.pitches).unwrap_or_else(|_| {
            t.lines().flat_map(|l| l.split('#').next().unwrap_or("").split_whitespace())
             .map(|x| x.parse().unwrap()).collect()
        })
    });
    // A shard is one (vy0, vz0) cell, so it is a directory and an independent job.
    let mut shards: Vec<(f64, f64)> = vec![];
    for &vy in &g.vys { for &vz in &g.vzs { shards.push((vy, vz)) } }
    if let Some(sel) = a.get("--shard") {
        let pick: Vec<usize> = sel.split(',').map(|x| x.parse().unwrap()).collect();
        shards = pick.iter().map(|&i| shards[i]).collect();
    }
    write_manifest(&dir, &g, opts);
    eprintln!("sweep: {} shards x {} cells, {} passes, trig {}, commit {}",
              shards.len(), g.ns.len() * g.lams.len(), opts.max_passes, trig_mode(), commit_hash());
    let t0 = Instant::now();
    let totals: Vec<(usize, usize)> = shards.par_iter()
        .map(|&(vy, vz)| run_shard(&dir, &g, vy, vz, opts, force, anchor.as_deref())).collect();
    let (d, s): (usize, usize) = totals.iter().fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    eprintln!("sweep: {d} solved, {s} resumed, {:.0}s", t0.elapsed().as_secs_f64());
}

fn cmd_fingerprint() {
    println!("{:016x}  trig {}  commit {}", physics_fingerprint(), trig_mode(), commit_hash());
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
        Some("run") => cmd_run(&a),
        Some("fingerprint") => cmd_fingerprint(),
        Some("residuals") => {
            let f = &a.0[2];
            let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
            let pr = Profile::parse(&text).unwrap_or_else(|e| panic!("{f}: {e}"));
            set_trig_mode(pr.trig);
            let r = residuals(&pr.obj, &pr.pitches, PolishOpts::default().global_step, pr.jitter);
            let (pos, neg) = (r.iter().filter(|x| x.0 > 1e-9).count(),
                              r.iter().filter(|x| x.0 < -1e-9).count());
            let sum: f64 = r.iter().map(|x| x.0).sum();
            let absum: f64 = r.iter().map(|x| x.0.abs()).sum();
            let gain: f64 = r.iter().map(|x| x.1).sum();
            eprintln!("{}: n {}, {} want more pitch, {} want less, {} still",
                      f.rsplit('/').next().unwrap(), r.len(), pos, neg, r.len() - pos - neg);
            eprintln!("  sum of moves {sum:+.3} deg, sum of |moves| {absum:.3} deg  ->  \
                       coherence {:.3}", if absum > 0.0 { sum / absum } else { 0.0 });
            eprintln!("  sum of per-tick gains {gain:.4} blocks, largest {:.2e}",
                      r.iter().map(|x| x.1).fold(0.0, f64::max));
            let deltas: Vec<f64> = r.iter().map(|x| x.0).collect();
            let (lag1, run_len, ks) = delta_structure(&deltas);
            eprintln!("  lag-1 correlation of the moves {lag1:+.3}, mean same-sign run {run_len:.1} ticks");
            eprint!("  energy in the first k cosine modes:");
            for (k, e) in &ks { eprint!("  k={k}: {:.0}%", 100.0 * e) }
            eprintln!();
            match jacobi_step(&pr.obj, &pr.pitches, PolishOpts::default().global_step, pr.jitter) {
                Some((_, alpha, g)) => eprintln!(
                    "  best whole-schedule step: alpha {alpha:.3} worth {g:.4} blocks \
                     ({:.1}% of the per-tick total)", 100.0 * g / gain.max(1e-12)),
                None => eprintln!("  no whole-schedule step improves J"),
            }
            println!("tick,pitch,delta,gain");
            for (t, (d, g)) in r.iter().enumerate() {
                println!("{t},{:.5},{d:+.5},{g:.3e}", pr.pitches[t]);
            }
        }
        Some("structure") => {
            for f in a.0[2..].iter().filter(|s| !s.starts_with("--")) {
                let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
                let ps = Profile::parse(&text).map(|p| p.pitches).unwrap_or_else(|_|
                    text.lines().flat_map(|l| l.split('#').next().unwrap_or("").split_whitespace())
                        .map(|s| s.parse().unwrap()).collect());
                let sh = shape(&ps);
                println!("{:<28} n {:>4}  {:>9}  pitch [{:>7.2}, {:>6.2}]  flat {:>3}",
                         f.rsplit('/').next().unwrap(), ps.len(), sh.structure.to_string(),
                         sh.pitch_min, sh.pitch_max, sh.flat_ticks);
            }
        }
        _ => eprintln!("{}", "usage: sweep <polish|verify|bench|pilot|run|structure|residuals|fingerprint> ...\n\
                             see the module docs at the top of src/bin/sweep.rs"),
    }
}
