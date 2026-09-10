//! Which schedules survive the mistakes a person makes while flying them?
//!
//! Pitch noise is cheap on these optima, but a person also misses cues, moves the mouse over
//! several ticks, and carries a persistent aim error. This table reports signed changes in dJ:
//! negative numbers are blocks lost, while a positive number means the mistake happened to help.
//!
//! usage: human <file>... [--trig m] [--draws k] [--shifts 1,2,5,...]
use elytrasim::opt::*;
use elytrasim::sim::*;

const KS: [usize; 5] = [1, 2, 5, 10, 20];

/// The mistimings the table prices, in ticks, overridable with `--shifts 1,2,3,...`. The default
/// five points sketch the shape; a dense scan is what you want to price "missed the cue by up to
/// half a second" at every tick in between, and that is a different question from the shape.
static SHIFTS: std::sync::OnceLock<Vec<usize>> = std::sync::OnceLock::new();

fn ks() -> &'static [usize] {
    SHIFTS.get_or_init(|| KS.to_vec())
}
const SIGMAS: [f64; 3] = [0.5, 1.0, 2.0];

/// SplitMix64 is deliberately local and seeded, so rerunning a table reproduces every draw.
fn rng(seed: u64) -> impl FnMut() -> f64 {
    let mut st = seed;
    move || {
        st = st.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = st;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
}

/// Store generated controls the same way the game does, without rounding back over the limit.
fn pitch32(x: f64, limit: f64) -> f64 {
    let bound = limit.abs().min(90.0);
    let mut p = x.clamp(-bound, bound) as f32;
    while (p as f64).abs() > bound {
        p = f32::from_bits(p.to_bits() - 1);
    }
    p as f64
}

fn shift(p: &[f64], k: usize, late: bool) -> Vec<f64> {
    assert!(
        k <= p.len(),
        "shift {k} exceeds a {}-tick schedule",
        p.len()
    );
    let mut q = Vec::with_capacity(p.len());
    if late {
        q.extend(std::iter::repeat_n(p[0], k));
        q.extend_from_slice(&p[..p.len() - k]);
    } else {
        q.extend_from_slice(&p[k..]);
        q.extend(std::iter::repeat_n(*p.last().unwrap(), k));
    }
    assert_eq!(q.len(), p.len());
    q
}

/// Where the manoeuvre begins: the first tick of the snap, including the steep descent into it.
///
/// This, not the flick tick, is the splice point for a mistimed cue. Splicing at the flick takes
/// its ticks out of the snap, so "flick 20 ticks early" silently DELETES the hold-0 -- and
/// removing the snap is independently worth about -74 blocks, which then shows up misattributed
/// as a timing cost. Taking the ticks out of the dive instead moves *when* the manoeuvre happens
/// while leaving every phase's shape alone, which is what missing a cue actually looks like.
fn manoeuvre_start(p: &[f64]) -> Option<usize> {
    let (flick, &lo) = p.iter().enumerate().min_by(|a, b| a.1.total_cmp(b.1))?;
    if lo > -30.0 {
        return None;                 // a steady glide has no manoeuvre to mistime
    }
    let mut t = flick;
    while t > 0 && p[t] < 5.0 {
        t -= 1;
    }
    let mut s = t + 1;
    while s > 1 && p[s - 1] - p[s] > 2.0 {
        s -= 1;
    }
    Some(s)
}

/// Run the whole manoeuvre (snap, flick, gain, ending) `k` ticks late or early, absorbing the
/// difference in the length of the dive. `None` when the schedule cannot give up the ticks.
fn cue(p: &[f64], start: usize, k: usize, late: bool) -> Option<Vec<f64>> {
    if k > p.len() || start == 0 || start + k > p.len() {
        return None;
    }
    if !late && start < k {
        return None;
    }
    let mut q = Vec::with_capacity(p.len());
    if late {
        q.extend_from_slice(&p[..start]);
        q.extend(std::iter::repeat_n(p[start - 1], k));   // hold the dive a little longer
        q.extend_from_slice(&p[start..p.len() - k]);
    } else {
        q.extend_from_slice(&p[..start - k]);             // cut the dive short
        q.extend_from_slice(&p[start..]);
        q.extend(std::iter::repeat_n(*p.last().unwrap(), k));
    }
    assert_eq!(q.len(), p.len());
    Some(q)
}

fn tremor(p: &[f64], sigma: f64, draw: usize, limit: f64) -> Vec<f64> {
    let mut u = rng(0x4855_4d41_4e_u64 ^ draw as u64);
    let modes = 4 + (u() * 5.0) as usize;
    let mut noise = vec![0.0; p.len()];
    for _ in 0..modes {
        // One to eight cycles across the flight keeps the error sustained rather than tickwise.
        let frequency = 1 + (u() * 8.0) as usize;
        let phase = std::f64::consts::TAU * u();
        let amplitude = (-2.0 * u().ln()).sqrt() * (std::f64::consts::TAU * u()).cos();
        for (t, x) in noise.iter_mut().enumerate() {
            let angle = std::f64::consts::TAU * frequency as f64 * t as f64 / p.len() as f64;
            *x += amplitude * (angle + phase).cos();
        }
    }
    let mean = noise.iter().sum::<f64>() / noise.len() as f64;
    let sd = (noise.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / noise.len() as f64).sqrt();
    let q: Vec<f64> = p
        .iter()
        .zip(noise)
        .map(|(&x, e)| pitch32(x + sigma * (e - mean) / sd, limit))
        .collect();
    assert_eq!(q.len(), p.len());
    q
}

fn lag(p: &[f64], tau: f64, limit: f64) -> Vec<f64> {
    let alpha = 1.0 - (-1.0 / tau).exp();
    let mut q = Vec::with_capacity(p.len());
    q.push(p[0]);
    for &want in &p[1..] {
        q.push(pitch32(
            q.last().unwrap() + (want - q.last().unwrap()) * alpha,
            limit,
        ));
    }
    assert_eq!(q.len(), p.len());
    q
}

fn print_header() {
    print!("{:<24} {:>8} |", "file", "dJ");
    for k in ks().iter().copied() {
        print!(
            " {:>8} {:>8} {:>8}",
            format!("shL{k}"),
            format!("shE{k}"),
            format!("shW{k}")
        );
    }
    print!(" |");
    for k in ks().iter().copied() {
        print!(
            " {:>8} {:>8} {:>8}",
            format!("cuL{k}"),
            format!("cuE{k}"),
            format!("cuW{k}")
        );
    }
    print!(" |");
    for sigma in SIGMAS {
        print!(" {:>8} {:>8}", format!("trM{sigma}"), format!("trP{sigma}"));
    }
    println!(
        " | {:>8} {:>8} {:>8} | {:>8} {:>8} {:>8} {:>8} {:>8}",
        "lag1", "lag2", "lag4", "b+1", "b-1", "b+2", "b-2", "bW"
    );
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let mut forced: Option<TrigMode> = None;
    let mut draws = 200usize;
    let mut files = Vec::new();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--trig" => {
                i += 1;
                forced = Some(argv.get(i).expect("--trig needs a mode").parse().unwrap());
            }
            "--shifts" => {
                i += 1;
                let v: Vec<usize> = argv.get(i).expect("--shifts needs a list")
                    .split(',').map(|x| x.trim().parse()
                        .unwrap_or_else(|e| panic!("bad --shifts entry {x:?}: {e:?}")))
                    .collect();
                assert!(!v.is_empty() && v.iter().all(|&k| k > 0),
                        "--shifts must be a non-empty list of positive tick counts");
                SHIFTS.set(v).expect("--shifts given twice");
            }
            "--draws" => {
                i += 1;
                draws = argv.get(i).expect("--draws needs a count").parse().unwrap();
            }
            a if a.starts_with("--") => panic!("unknown option {a}"),
            _ => files.push(argv[i].clone()),
        }
        i += 1;
    }
    assert!(draws > 0, "--draws must be positive");

    print_header();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
        let parsed = Profile::parse(&text);
        // A missing header must select the default physics anew, not inherit the previous row.
        set_trig_mode(forced.unwrap_or_else(|| {
            parsed
                .as_ref()
                .map(|profile| profile.trig)
                .unwrap_or_default()
        }));
        let (obj, ps, limit) = match parsed {
            Ok(profile) => (profile.obj, profile.pitches, profile.rough.limit),
            Err(_) => (
                Objective {
                    v0: V0,
                    n: 0,
                    lambda: 0.0,
                },
                read_pitches(f),
                90.0,
            ),
        };
        assert!(!ps.is_empty(), "{f}: empty schedule");
        let obj = Objective { n: ps.len(), ..obj };
        let j0 = obj.j(&State {
            pos: Vec3::ZERO,
            vel: obj.v0,
        });
        let base = obj.eval(&ps) - j0;
        let loss = |q: &[f64]| {
            assert_eq!(q.len(), ps.len());
            obj.eval(q) - j0 - base
        };

        let name = std::path::Path::new(f)
            .file_name()
            .unwrap()
            .to_string_lossy();
        print!("{:<24} {:>8.3} |", name, base);
        for k in ks().iter().copied() {
            let late = loss(&shift(&ps, k, true));
            let early = loss(&shift(&ps, k, false));
            print!(" {:>8.3} {:>8.3} {:>8.3}", late, early, late.min(early));
        }

        print!(" |");
        if let Some(flick) = manoeuvre_start(&ps) {
            for k in ks().iter().copied() {
                let late = cue(&ps, flick, k, true).map(|q| loss(&q));
                let early = cue(&ps, flick, k, false).map(|q| loss(&q));
                let cell = |v: Option<f64>| match v {
                    Some(x) => format!("{x:>8.3}"),
                    None => format!("{:>8}", "-"),
                };
                // The worst case is over the shifts that exist, not over a silent zero.
                let worst = match (late, early) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
                print!(" {} {} {}", cell(late), cell(early), cell(worst));
            }
        } else {
            for _ in 0..ks().len() {
                print!(" {:>8} {:>8} {:>8}", "-", "-", "-");
            }
        }

        print!(" |");
        for sigma in SIGMAS {
            let mut values: Vec<f64> = (0..draws)
                .map(|draw| loss(&tremor(&ps, sigma, draw, limit)))
                .collect();
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            values.sort_by(f64::total_cmp);
            print!(" {:>8.3} {:>8.3}", mean, values[values.len() / 20]);
        }

        print!(" |");
        for tau in [1.0, 2.0, 4.0] {
            print!(" {:>8.3}", loss(&lag(&ps, tau, limit)));
        }
        print!(" |");
        let biases = [1.0, -1.0, 2.0, -2.0];
        let mut bias_losses = Vec::new();
        for bias in biases {
            let q: Vec<f64> = ps.iter().map(|&p| pitch32(p + bias, limit)).collect();
            assert_eq!(q.len(), ps.len());
            let value = loss(&q);
            bias_losses.push(value);
            print!(" {:>8.3}", value);
        }
        println!(
            " {:>8.3}",
            bias_losses.into_iter().reduce(f64::min).unwrap()
        );
    }
}
