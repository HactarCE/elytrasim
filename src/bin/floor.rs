//! Flight over a floor: how long, and how far, before the trajectory first goes under `y = 0`.
//!
//! The polish only knows a fixed horizon, so endurance is a search over it: a horizon `n` is
//! *feasible* when some schedule keeps every one of its `n` post-tick states at or above the
//! floor, and the endurance is the largest feasible `n`. Inside one horizon the objective is the
//! usual `J = TE(s_n) + w*z_n` less the floor's price (see `Floor` in `opt.rs`); `J` only picks
//! which feasible schedule to hand the next horizon, and feasibility is read off the replay.
//!
//! Subcommands:
//!   probe                          constant-pitch baselines: glides, and ticks to the floor
//!   solve  --y0 <h> --n <n> [--init <spec>] [--out <file>]
//!   safety --y0 <h> --n <n> [--init <spec>]   max soft-min clearance: is this horizon feasible?
//!   endure --y0 <h> [--nmax <n>] [--misses <k>] [--out <file>]   the longest feasible horizon
//!   depth  --file <pitches> [--vy --vz]   how far a schedule dips below its start
//!   exit   --y0 <h> [--mode time|dist] [--n <cap>] [--nmax <cap>] [--init <spec>]
//!                                  the interpolated first exit; the cap doubles while the
//!                                  answer survives it
//!
//! Common options: --vy, --vz (initial velocity, default 0), --lambda (default 0),
//!   --margin, --weight, --wall (the floor's price), --mu, --limit, --passes, --tol.
//! An init spec is `hold:<p>`, `pump:<p_down>,<k>,<p_up>` (p_down for k ticks, then p_up),
//! `tile:<file>` (a cycle repeated), or a pitch file (last pitch repeated). `exit` also takes
//! `minipump[:<d>[,<k>]]`, see `minipump`.
//!
//! Physics is `mth_lut` trig and `reference` flight, fixed: vanilla's table, and the kernel
//! that is fastest on arm64 (README-sweep.md).

use elytrasim::opt::*;
use elytrasim::sim::*;
use rayon::prelude::*;

struct Args(Vec<String>);

impl Args {
    fn new() -> Args { Args(std::env::args().collect()) }
    fn get(&self, k: &str) -> Option<&str> {
        self.0.iter().position(|a| a == k).and_then(|i| self.0.get(i + 1)).map(String::as_str)
    }
    fn num<T: std::str::FromStr>(&self, k: &str, d: T) -> T where T::Err: std::fmt::Debug {
        self.get(k).map_or(d, |v| v.parse().unwrap_or_else(|e| panic!("bad {k}: {e:?}")))
    }
    fn obj(&self, n: usize) -> Objective {
        Objective { v0: Vec3::new(0.0, self.num("--vy", 0.0), self.num("--vz", 0.0)),
                    n, lambda: self.num("--lambda", 0.0) }
    }
    fn floor(&self) -> Floor {
        let d = Floor::default();
        Floor { depth: self.num("--y0", 4.0), margin: self.num("--margin", d.margin),
                weight: self.num("--weight", d.weight), wall: self.num("--wall", d.wall) }
    }
    fn opts(&self) -> PolishOpts {
        PolishOpts {
            max_passes: self.num("--passes", 200usize),
            tol: self.num("--tol", 1e-3),
            rough: Rough { mu: self.num("--mu", 1e-4), limit: self.num("--limit", 85.0),
                           ..Default::default() },
            floor: self.floor(),
            ..Default::default()
        }
    }
}

fn init(spec: &str, n: usize) -> Vec<f64> {
    if let Some(p) = spec.strip_prefix("hold:") { return vec![p.parse().unwrap(); n] }
    if let Some(r) = spec.strip_prefix("pump:") {
        let v: Vec<f64> = r.split(',').map(|x| x.parse().unwrap()).collect();
        let k = v[1] as usize;
        return (0..n).map(|t| if t < k { v[0] } else { v[2] }).collect();
    }
    if let Some(f) = spec.strip_prefix("tile:") {
        let c = read_pitches(f);
        return (0..n).map(|t| c[t % c.len()]).collect();
    }
    init_from(&read_pitches(spec), n)
}

/// What a schedule does over the floor, from its replay alone.
struct Flown { survived: usize, clearance: f64, z: f64, te: f64, cost: f64 }

fn flown(obj: &Objective, floor: &Floor, p: &[f64]) -> Flown {
    let st = obj.replay(p);
    let s = st.last().unwrap();
    Flown { survived: floor.survived(&st), clearance: floor.clearance(&st), z: s.pos.z,
            te: s.total_energy() + floor.depth, cost: floor.cost(&st) }
}

/// Each pitch as the shortest decimal that round-trips its `f32`. Not a display rounding: an
/// optimum here skims the floor at zero margin, and writing pitches to four places moved one
/// replay 0.004 blocks through it.
fn fmt_pitches(p: &[f64]) -> String {
    p.chunks(10).map(|c| c.iter().map(|&x| format!("{}", x as f32)).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>().join("\n")
}

/// Soft minimum of the clearance over `states[1..]`: `-tau * ln sum exp(-h/tau)`, shifted by the
/// hard minimum so it cannot overflow. Never above the hard minimum, and within `tau * ln n` of it.
fn softmin(hs: impl Iterator<Item = f64> + Clone, tau: f64) -> f64 {
    let m = hs.clone().fold(f64::INFINITY, f64::min);
    m - tau * hs.map(|h| (-(h - m) / tau).exp()).sum::<f64>().ln()
}

/// The feasibility oracle: coordinate ascent on the soft-minimum clearance, the discrete HJ
/// safety value `max_u min_t h_t`. A horizon is feasible exactly when this can be pushed to 0 or
/// above, which the energy objective does not ask for -- it only prices the floor -- so a horizon
/// the energy polish reports infeasible is re-tried here before it is written off.
///
/// Returns the schedule and its hard clearance. Stops early once the hard clearance is
/// non-negative: past that point the oracle has answered its question.
fn safety(obj: &Objective, floor: &Floor, init: &[f64], limit: f64, passes: usize) -> (Vec<f64>, f64) {
    let tau = 0.01;
    let mut p = init.to_vec();
    let step = 0.25;
    let cands: Vec<f64> = (0..=((2.0 * limit / step) as i64)).map(|i| -limit + step * i as f64).collect();
    let hard = |p: &[f64]| floor.clearance(&obj.replay(p));
    let mut best = hard(&p);
    for _ in 0..passes {
        if best >= 0.0 { break }
        for t in 0..p.len() {
            let st = obj.replay(&p);
            let pre: Vec<f64> = st[1..=t].iter().map(|s| s.pos.y + floor.depth).collect();
            let tail: Vec<PitchTrig> = p[t + 1..].iter().map(|&q| PitchTrig::new(q as f32)).collect();
            let score = |x: f64| -> f64 {
                let mut s = st[t].clone().ticked_cached(PitchTrig::new(x as f32));
                let mut hs = pre.clone();
                hs.push(s.pos.y + floor.depth);
                for &q in &tail { s = s.ticked_cached(q); hs.push(s.pos.y + floor.depth) }
                softmin(hs.iter().copied(), tau)
            };
            let (mut bp, mut bs) = (p[t], score(p[t]));
            let (gp, gs) = cands.par_iter().map(|&x| (x, score(x)))
                .reduce(|| (bp, f64::NEG_INFINITY), |a, b| if b.1 > a.1 { b } else { a });
            if gs > bs { bp = gp; bs = gs }
            let (mut a, mut b) = ((bp - step).max(-limit), (bp + step).min(limit));
            for _ in 0..40 {
                let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
                if score(m1) < score(m2) { a = m1 } else { b = m2 }
            }
            let r = (0.5 * (a + b)) as f32 as f64;
            if score(r) > bs { bp = r }
            p[t] = bp as f32 as f64;
        }
        let now = hard(&p);
        if now <= best + 1e-9 { best = best.max(now); break }
        best = now;
    }
    (p, best)
}

/// Ticks a constant pitch survives, and the best such pitch on a 0.1-degree grid.
fn best_hold(obj: &Objective, floor: &Floor, limit: f64) -> (usize, f64) {
    let mut best = (0usize, 0.0);
    for i in (-(limit * 10.0) as i64)..=((limit * 10.0) as i64) {
        let q = 0.1 * i as f64;
        let t = floor.survived(&obj.replay(&vec![q; 400]));
        if t > best.0 { best = (t, q) }
    }
    best
}

const COLD: &[&str] = &["hold:-13", "hold:0", "pump:30,6,-20", "pump:60,4,-30"];

/// Endurance: the largest horizon with a feasible schedule, by walking `n` upward from the best
/// constant pitch's survival.
///
/// Each horizon is polished from the previous horizon's answer (last pitch repeated) and from
/// every `COLD` start; the feasible result with the best priced objective is kept and seeds the
/// next horizon. A horizon nobody made feasible goes to `safety` before it counts as a miss, and
/// the walk stops after `--misses` consecutive misses. A miss is evidence, not proof: the inner
/// search is local.
fn endure(a: &Args) {
    let floor = a.floor();
    let opts = a.opts();
    let nmax: usize = a.num("--nmax", 150);
    let misses_max: usize = a.num("--misses", 3);
    let (n0, q0) = best_hold(&a.obj(1), &floor, opts.rough.limit);
    let mut best: Option<(usize, Vec<f64>)> = None;
    let mut far: Option<(usize, Vec<f64>, f64)> = None;         // the feasible horizon reaching furthest
    let mut prev = vec![q0; n0];
    let mut misses = 0;
    let mut n = n0;
    while n <= nmax && misses < misses_max {
        let obj = a.obj(n);
        let mut starts: Vec<(String, Vec<f64>)> = vec![("warm".into(), init_from(&prev, n))];
        for c in COLD { starts.push((c.to_string(), init(c, n))) }
        let mut found: Option<(String, Vec<f64>, f64)> = None;
        let mut near: Vec<(String, Vec<f64>)> = Vec::new();
        for (tag, s) in starts {
            let p = polish_annealed(a, &obj, &s);
            let st = obj.replay(&p);
            // Compared on `J` alone once feasible: the stages ran at different bubbles, so their
            // priced objectives are not on one scale, and feasibility is already settled.
            let score = obj.eval(&p);
            if floor.clearance(&st) >= 0.0 {
                if found.as_ref().map_or(true, |f| score > f.2) { found = Some((tag, p, score)) }
            } else { near.push((tag, p)) }
        }
        if found.is_none() {
            for (tag, s) in near {
                let (p, c) = safety(&obj, &floor, &s, opts.rough.limit, 200);
                if c >= 0.0 {
                    // Feasible but not energy-optimal: polish it once more, and keep whichever
                    // of the two is still feasible.
                    let r = polish(&obj, &p, opts);
                    let keep = if floor.clearance(&obj.replay(&r.pitches)) >= 0.0 { r.pitches } else { p };
                    found = Some((format!("{tag}+safety"), keep, f64::NAN));
                    break;
                }
            }
        }
        match found {
            Some((tag, p, _)) => {
                let f = flown(&obj, &floor, &p);
                eprintln!("  n {n:>3} feasible via {tag:<16} clearance {:+.4} z {:7.3}", f.clearance, f.z);
                misses = 0;
                if far.as_ref().map_or(true, |b| f.z > b.2) { far = Some((n, p.clone(), f.z)) }
                prev = p.clone();
                best = Some((n, p));
            }
            None => { eprintln!("  n {n:>3} infeasible"); misses += 1 }
        }
        n += 1;
    }
    let (n, p) = best.expect("the best constant pitch is feasible at its own survival");
    let obj = a.obj(n);
    let f = flown(&obj, &floor, &p);
    let capped = if n == nmax { " (hit --nmax)" } else { "" };
    let text = format!(
        "# endure y0 {} v0 ({}, {}) lambda {} margin {} weight {} wall {} mu {} limit {}\n\
         # hold: {n0} ticks at pitch {q0:.1};  optimized: {n} ticks{capped}, clearance {:+.4}, z {:.4}\n{}\n",
        floor.depth, obj.v0.y, obj.v0.z, obj.lambda, floor.margin, floor.weight, floor.wall,
        opts.rough.mu, opts.rough.limit, f.clearance, f.z, fmt_pitches(&p));
    let (fn_, fp, fz) = far.unwrap();
    let text = format!("{text}# furthest: {fn_} ticks, z {fz:.4}\n{}\n", fmt_pitches(&fp));
    match a.get("--out") {
        Some(o) => { std::fs::write(o, &text).unwrap();
                     print!("{}", text.lines().filter(|l| l.starts_with('#')).collect::<Vec<_>>().join("\n") + "\n") }
        None => print!("{text}"),
    }
}

/// Polish, then shrink the bubble and polish again, `--anneal` times: margin and weight each
/// times `--shrink`, the wall untouched. Keeps the last stage that is still feasible, or the first
/// stage's answer if none is.
///
/// The bubble is conservative by design, and how conservative is not a detail: a range optimum
/// skims the floor for many ticks and pays the bubble on every one of them, so at a fixed bubble
/// the distance reached is set by the ratio of `weight` to `lambda`, not by `lambda`. Annealing is
/// the interior-point schedule: each stage starts from a schedule the previous one kept off the
/// floor, so it only has to give up margin, never find it.
fn polish_annealed(a: &Args, obj: &Objective, init: &[f64]) -> Vec<f64> {
    let mut opts = a.opts();
    let (stages, shrink): (usize, f64) = (a.num("--anneal", 0), a.num("--shrink", 0.3));
    let mut p = polish(obj, init, opts).pitches;
    for _ in 0..stages {
        opts.floor.margin *= shrink;
        opts.floor.weight *= shrink;
        let q = polish(obj, &p, opts).pitches;
        if a.floor().clearance(&obj.replay(&q)) >= 0.0 { p = q } else { break }
    }
    p
}

/// A seed of the right length from a schedule of another: truncate, or repeat the last pitch.
fn init_from(p: &[f64], n: usize) -> Vec<f64> {
    let mut v = p.to_vec();
    let last = *v.last().unwrap_or(&0.0);
    v.resize(n, last);
    v
}

/// The two steady-glide constants, from `probe`.
const MIN_SINK: f64 = 0.0708;    // blocks/tick, at pitch -13.0
const BEST_GLIDE: f64 = 10.10;   // blocks of z per block of height, at pitch 0

/// What a first-exit schedule is scored on. Every variant reads only the replay up to the first
/// state under the floor, so the schedule's later ticks are dead and the horizon is just a cap.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Exit {
    /// The fractional tick at which the replay crosses the floor.
    Time,
    /// `z` at that fractional tick.
    Dist,
}

impl Exit {
    /// Score units per block of energy: what one block of `TE` buys at min sink or at the best
    /// glide ratio. `mu` is quoted in blocks of energy, as in `polish`; charged unconverted in
    /// ticks or in blocks of `z` it was 14x or 10x weaker than there, and the schedules showed it
    /// -- one-tick spikes to 75 degrees where the pitch barely matters.
    fn per_block(self) -> f64 { match self { Exit::Time => 1.0 / MIN_SINK, Exit::Dist => BEST_GLIDE } }
}

/// Where the replay exits, interpolated: `(t*, z(t*))`, from the first pair of states that
/// straddles the floor. `h` is affine along the secant between the two, so
/// `t* = (k-1) + h[k-1] / (h[k-1] - h[k])`, and `z` is read off the same secant.
///
/// Why interpolate. The integer exit tick is a step function of every pitch, so a coordinate
/// search sees a plateau with cliffs; the secant makes it continuous as the crossing slides from
/// one tick to the next -- at `h[k] = 0` both readings give `t* = k`. What it does *not* smooth is
/// a touch-and-go: a dip that goes under and comes back makes an earlier pair the first one, and
/// `t*` jumps down.
///
/// A replay that never exits scores its horizon plus a value-to-go for the energy it still holds:
/// that energy spent at min sink (`Time`) or at the best glide ratio (`Dist`), the two steady-glide
/// constants from `probe`. Crude, but it only has to rank schedules that all survive the cap.
fn exit_score(states: &[State], depth: f64, mode: Exit) -> f64 {
    for k in 1..states.len() {
        let (a, b) = (states[k - 1].pos.y + depth, states[k].pos.y + depth);
        if b < 0.0 {
            let f = a / (a - b);
            return match mode {
                Exit::Time => (k - 1) as f64 + f,
                Exit::Dist => states[k - 1].pos.z + f * (states[k].pos.z - states[k - 1].pos.z),
            };
        }
    }
    let s = states.last().unwrap();
    let e = s.total_energy() + depth;
    match mode {
        Exit::Time => (states.len() - 1) as f64 + e / MIN_SINK,
        Exit::Dist => s.pos.z + e * BEST_GLIDE,
    }
}

/// Coordinate ascent on a first-exit score over a fixed cap. The same sweep as `polish` -- a
/// global scan of every tick's pitch on a 0.25-degree grid, a ternary refine, the `f32` round,
/// Gauss-Seidel -- but scored on the whole replay rather than on a terminal state, and every pass
/// global. The l1 curvature price is charged as in `polish`, converted to score units by
/// `Exit::per_block`. Ticks after the exit are dead: they are not searched, and after each pass
/// they are set to the last live pitch, so the curvature price never sees the seed's leftover
/// tail.
fn ascend(obj: &Objective, depth: f64, init: &[f64], mode: Exit, rough: Rough, passes: usize)
    -> (Vec<f64>, f64) {
    let rough = Rough { mu: rough.mu * mode.per_block(), ..rough };
    let step = 0.25;
    let lim = rough.limit;
    let cands: Vec<f64> = (0..=((2.0 * lim / step) as i64)).map(|i| (-lim + step * i as f64).min(lim)).collect();
    let mut p = init.to_vec();
    let flatten = |p: &mut Vec<f64>| {
        let st = obj.replay(p);
        if let Some(k) = (1..st.len()).find(|&k| st[k].pos.y + depth < 0.0) {
            let last = p[k - 1];
            p[k..].iter_mut().for_each(|q| *q = last);
        }
    };
    flatten(&mut p);
    let total = |p: &[f64]| exit_score(&obj.replay(p), depth, mode) - rough.cost(p);
    let mut best = total(&p);
    let mut quiet = 0;
    for _ in 0..passes {
        let before = best;
        for t in 0..p.len() {
            let st = obj.replay(&p);
            // Dead tick: the replay is already under the floor by the state this pitch produces.
            if (1..=t).any(|k| st[k].pos.y + depth < 0.0) { break }
            let tail: Vec<PitchTrig> = p[t + 1..].iter().map(|&q| PitchTrig::new(q as f32)).collect();
            let w = win5(&p, t);
            let score = |x: f64| -> f64 {
                let mut v = st[..=t].to_vec();
                let mut s = st[t].ticked_cached(PitchTrig::new(x as f32));
                v.push(s.clone());
                for &q in &tail {
                    if s.pos.y + depth < 0.0 { break }
                    s = s.ticked_cached(q);
                    v.push(s.clone());
                }
                exit_score(&v, depth, mode) - rough.local(&w, x)
            };
            let (mut bp, mut bs) = (p[t], score(p[t]));
            let (gp, gs) = cands.par_iter().map(|&x| (x, score(x)))
                .reduce(|| (bp, f64::NEG_INFINITY), |a, b| if b.1 > a.1 { b } else { a });
            if gs > bs { bp = gp; bs = gs }
            let (mut a, mut b) = ((bp - step).max(-lim), (bp + step).min(lim));
            for _ in 0..40 {
                let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
                if score(m1) < score(m2) { a = m1 } else { b = m2 }
            }
            let r = (0.5 * (a + b)).clamp(-lim, lim) as f32 as f64;
            if score(r) > bs { bp = r }
            p[t] = bp as f32 as f64;
        }
        flatten(&mut p);
        best = total(&p);
        if best - before < 1e-6 { quiet += 1; if quiet >= 3 { break } } else { quiet = 0 }
    }
    (p, best)
}

/// The minipump seed: `d` degrees nose-down for `k` ticks, pitch 0 for 10, -40 while `v_y` is
/// still rising, then 0. The dive is the part a per-tick search cannot invent from a hold, since
/// no single tick of it pays on its own.
fn minipump(obj: &Objective, d: f64, k: usize) -> Vec<f64> {
    let mut p = Vec::with_capacity(obj.n);
    let mut s = State { pos: Vec3::ZERO, vel: obj.v0 };
    let mut climbing = true;
    while p.len() < obj.n {
        let t = p.len();
        let q = if t < k { d } else if t < k + 10 || !climbing { 0.0 } else { -40.0 };
        let next = s.ticked_cached(PitchTrig::new(q as f32));
        if q == -40.0 && next.vel.y < s.vel.y { climbing = false }
        p.push(q);
        s = next;
    }
    p
}

/// A minipump seed with the dive's length `k` fitted -- the `k` whose seed scores best -- and,
/// unless given, the dive angle `d` too, on a 2.5-degree grid.
fn fit_minipump(obj: &Objective, depth: f64, mode: Exit, spec: &str) -> (Vec<f64>, f64, usize, f64) {
    let v: Vec<f64> = spec.split(',').filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).collect();
    let ds: Vec<f64> = match v.first() { Some(&d) => vec![d], None => (0..=34).map(|i| 2.5 * i as f64).collect() };
    let ks: Vec<usize> = match v.get(1) { Some(&k) => vec![k as usize], None => (0..obj.n).collect() };
    ds.par_iter().flat_map(|&d| ks.par_iter().map(move |&k| (d, k)))
        .map(|(d, k)| { let p = minipump(obj, d, k); let sc = exit_score(&obj.replay(&p), depth, mode); (p, d, k, sc) })
        .reduce(|| (vec![], 0.0, 0, f64::NEG_INFINITY), |a, b| if b.3 > a.3 { b } else { a })
}

/// First-exit ascent under a cap that doubles, up to `--nmax`, for as long as the answer
/// survives it. Why: a survivor is scored on a value-to-go guess rather than an exit, so a cap
/// that binds decides the answer. Doubling is a heuristic, not a bound -- a pump that gains
/// energy survives every cap, and says so in the header.
fn exit_cmd(a: &Args) {
    let mut n: usize = a.num("--n", 150);
    let nmax: usize = a.num("--nmax", 2400);
    let depth = a.num("--y0", 4.0);
    let mode = match a.get("--mode").unwrap_or("time") { "time" => Exit::Time, "dist" => Exit::Dist,
                                                         m => panic!("bad --mode {m}") };
    let spec = a.get("--init").unwrap_or("hold:-13");
    let mut seed_note = String::new();
    let mut p = match spec.strip_prefix("minipump") {
        Some(r) => {
            let (p, d, k, sc) = fit_minipump(&a.obj(n), depth, mode, r.trim_start_matches(':'));
            seed_note = format!("  minipump d {d} k {k} seed score {sc:.4}");
            p
        }
        None => init(spec, n),
    };
    let passes = a.num("--passes", 100usize);
    let (obj, sc) = loop {
        let obj = a.obj(n);
        let (q, sc) = ascend(&obj, depth, &init_from(&p, n), mode, a.opts().rough, passes);
        p = q;
        let fl = Floor { depth, ..Default::default() };
        if fl.survived(&obj.replay(&p)) < n || n >= nmax { break (obj, sc) }
        n = (2 * n).min(nmax);
    };
    let st = obj.replay(&p);
    let fl = Floor { depth, ..Default::default() };
    let k = fl.survived(&st);
    let verdict = if k >= n { "  SURVIVES the cap: score is a value-to-go guess" } else { "" };
    let text = format!("# exit {mode:?} y0 {depth} v0 ({}, {}) cap {n} init {spec} mu {} limit {}{seed_note}\n\
                        # score {sc:.4}  t* {:.4}  z(t*) {:.4}  survived {k}{verdict}\n{}\n",
                       obj.v0.y, obj.v0.z, a.opts().rough.mu, a.opts().rough.limit,
                       exit_score(&st, depth, Exit::Time), exit_score(&st, depth, Exit::Dist),
                       fmt_pitches(&p[..(k + 1).min(n)]));
    match a.get("--out") {
        Some(o) => { std::fs::write(o, &text).unwrap(); print!("{}", text.lines().take(2).collect::<Vec<_>>().join("\n") + "\n") }
        None => print!("{text}"),
    }
}

fn probe() {
    let (mut ms, mut msp, mut bg, mut bgp) = (f64::INFINITY, 0.0, 0.0, 0.0);
    for i in -900..=900 {
        let p = 0.1 * i as f64;
        let e = equilibrium(p);
        if e.y < 0.0 && -e.y < ms { ms = -e.y; msp = p }
        if e.y < 0.0 && e.z / -e.y > bg { bg = e.z / -e.y; bgp = p }
    }
    println!("steady glide: min sink {ms:.5} b/tick at pitch {msp:.1}; best glide ratio {bg:.4} at {bgp:.1}");
    println!("constant pitch from v0 = 0, pitch step 0.1:");
    println!("{:>3} {:>6} {:>7} {:>8} {:>7}   {:>12}   {:>12}", "y0", "ticks", "at", "z", "at",
             "hold 0 t, z", "hold -13 t, z");
    for y0 in 1..=8 {
        let fl = Floor { depth: y0 as f64, ..Default::default() };
        let (mut bt, mut btp, mut bz, mut bzp) = (0usize, 0.0, 0.0f64, 0.0);
        for i in -900..=900 {
            let p = 0.1 * i as f64;
            let st = replay_from(Vec3::ZERO, &vec![p; 400]);
            let t = fl.survived(&st);
            if t > bt { bt = t; btp = p }
            if st[t].pos.z > bz { bz = st[t].pos.z; bzp = p }
        }
        let at = |q: f64| { let st = replay_from(Vec3::ZERO, &vec![q; 400]);
                            let t = fl.survived(&st); (t, st[t].pos.z) };
        let ((t0, z0), (t13, z13)) = (at(0.0), at(-13.0));
        println!("{y0:>3} {bt:>6} {btp:>7.1} {bz:>8.3} {bzp:>7.1}   {t0:>4} {z0:>7.3}   {t13:>4} {z13:>7.3}");
    }
}

fn solve(a: &Args) {
    let n: usize = a.num("--n", 40);
    let obj = a.obj(n);
    let opts = a.opts();
    let spec = a.get("--init").unwrap_or("hold:-13");
    let r = polish(&obj, &init(spec, n), opts);
    let f = flown(&obj, &opts.floor, &r.pitches);
    let text = format!(
        "# floor y0 {} margin {} weight {} wall {}\n# n {n} lambda {} v0 ({}, {}) mu {} limit {}\n\
         # init {spec}  passes {}  residual {:.2e}  lag1 {:+.3}\n\
         # survived {}  clearance {:+.4}  z {:.4}  TE {:.4}  floor cost {:.4}  J {:.4}\n{}\n",
        opts.floor.depth, opts.floor.margin, opts.floor.weight, opts.floor.wall,
        obj.lambda, obj.v0.y, obj.v0.z, opts.rough.mu, opts.rough.limit,
        r.passes, r.residual, r.lag1, f.survived, f.clearance, f.z, f.te, f.cost, r.j,
        fmt_pitches(&r.pitches));
    match a.get("--out") {
        Some(o) => { std::fs::write(o, &text).unwrap(); print!("{}", text.lines().take(4).collect::<Vec<_>>().join("\n") + "\n") }
        None => print!("{text}"),
    }
}

fn main() {
    set_trig_mode(TrigMode::MthLut);
    set_flight_mode(FlightMode::Reference);
    let a = Args::new();
    match a.0.get(1).map(String::as_str) {
        Some("probe") => probe(),
        Some("solve") => solve(&a),
        Some("endure") => endure(&a),
        Some("exit") => exit_cmd(&a),
        Some("depth") => {
            // How far a schedule's replay goes below its start, from `--vy/--vz`: the floor
            // clearance a repeated cycle needs.
            let p = read_pitches(a.get("--file").expect("--file"));
            let st = a.obj(p.len()).replay(&p);
            let lo = st.iter().map(|s| s.pos.y).fold(f64::INFINITY, f64::min);
            let hi = st.iter().map(|s| s.pos.y).fold(f64::NEG_INFINITY, f64::max);
            let e = st.last().unwrap();
            println!("min y {lo:.3}  max y {hi:.3}  end y {:.3}  end v ({:.4}, {:.4})", e.pos.y, e.vel.y, e.vel.z);
            if let Some(every) = a.get("--every").map(|v| v.parse::<usize>().unwrap()) {
                for (t, s) in st.iter().enumerate().step_by(every) {
                    println!("  t {t:4}  y {:8.3}  TE {:8.3}  v ({:+.4}, {:.4})", s.pos.y, s.total_energy(), s.vel.y, s.vel.z);
                }
            }
        }
        Some("safety") => {
            let n: usize = a.num("--n", 40);
            let obj = a.obj(n);
            let (p, c) = safety(&obj, &a.floor(), &init(a.get("--init").unwrap_or("hold:-13"), n),
                                a.num("--limit", 85.0), a.num("--passes", 200usize));
            println!("# clearance {c:+.5}  survived {}\n{}", a.floor().survived(&obj.replay(&p)), fmt_pitches(&p));
        }
        _ => { eprintln!("usage: floor probe | solve --y0 <h> --n <n> [--init <spec>]"); std::process::exit(2) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: &[f64]) -> Vec<State> {
        y.iter().enumerate().map(|(t, &y)| State { pos: Vec3::new(0.0, y, t as f64), vel: Vec3::ZERO }).collect()
    }

    /// The point of interpolating: as the crossing slides from one tick to the next, `t*` and
    /// `z(t*)` do not jump.
    #[test]
    fn exit_is_continuous_across_a_tick() {
        let eps = 1e-9;
        let (a, b) = (at(&[0.0, -0.5, -1.0 + eps, -1.5]), at(&[0.0, -0.5, -1.0 - eps, -1.5]));
        for mode in [Exit::Time, Exit::Dist] {
            let (x, y) = (exit_score(&a, 1.0, mode), exit_score(&b, 1.0, mode));
            assert!((x - 2.0).abs() < 1e-6 && (y - 2.0).abs() < 1e-6, "{mode:?}: {x} {y}");
        }
    }
}
