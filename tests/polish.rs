//! What the optimizer and its certificate do and do not promise.

use elytrasim::opt::*;
use elytrasim::sim::*;

fn obj(n: usize, lambda: f64) -> Objective {
    Objective { v0: Vec3::new(0.0, 0.167467, 0.200887), n, lambda }
}

#[test]
fn lambda_one_balances_height_against_distance() {
    // w is an exchange rate: at lambda = 1 the reference cycle's 21.5 blocks of climb and its
    // 330 blocks of z are worth the same.
    assert!((w_of_lambda(1.0) * Z_REF - Y_REF).abs() < 1e-12);
    assert_eq!(w_of_lambda(0.0), 0.0);
    assert_eq!(w_of_lambda(-1.0), -w_of_lambda(1.0));
}

#[test]
fn polish_never_decreases_j() {
    let o = obj(60, 0.0);
    let seed = seed_from_policy(&o);
    let before = o.eval(&seed);
    let r = polish(&o, &seed, PolishOpts { max_passes: 6, ..Default::default() });
    assert!(r.j >= before - 1e-12, "J fell from {before} to {}", r.j);
    assert_eq!(r.pitches.len(), o.n);
}

/// Every pitch written must survive the `f32` round trip the sim performs, or a profile scores
/// differently than it certifies.
#[test]
fn pitches_are_exactly_representable_as_f32() {
    let o = obj(40, 0.5);
    let r = polish(&o, &seed_from_policy(&o), PolishOpts { max_passes: 3, ..Default::default() });
    for (t, &p) in r.pitches.iter().enumerate() {
        assert_eq!(p, p as f32 as f64, "pitch {t} = {p} is not an f32");
        assert!((-90.0..=90.0).contains(&p), "pitch {t} = {p} out of range");
    }
}

/// The certificate is *local*. Coordinate ascent from a flat schedule converges hard -- to a
/// cycle that dives instead of climbing. A small residual says the schedule is a coordinate
/// optimum, and says nothing about whether it is the right one; that is what the seed is for.
#[test]
fn a_converged_certificate_does_not_mean_a_good_profile() {
    let o = obj(300, 0.0);
    let bad = polish(&o, &vec![0.0; o.n], PolishOpts { max_passes: 40, ..Default::default() });
    let good = polish(&o, &seed_from_policy(&o), PolishOpts { max_passes: 40, ..Default::default() });
    assert!(bad.residual < 1e-5, "the flat seed should converge, residual {:.2e}", bad.residual);
    assert!(bad.j < 0.0 && good.j > 20.0, "expected a huge J gap, got {} vs {}", bad.j, good.j);
}

/// A profile is a self-contained claim: header in, header out, and the pitches unchanged.
#[test]
fn profiles_round_trip_through_the_file_format() {
    let o = obj(50, -1.25);
    let r = polish(&o, &seed_from_policy(&o), PolishOpts { max_passes: 3, ..Default::default() });
    let p = Profile { obj: o, trig: trig_mode(), jitter: Jitter::default(), rough: Rough::default(),
                      commit: commit_hash().into(),
                      pitches: r.pitches.clone(), residual: r.residual, passes: r.passes };
    let back = Profile::parse(&p.to_string()).expect("must parse");
    assert_eq!(back.obj, o);
    assert_eq!(back.pitches, r.pitches);
    assert_eq!(back.trig, trig_mode());
}

/// `certify` must actually see a schedule that is not optimal. Ticks in the gain phase are the
/// sensitive ones; the entry is flat enough that a couple of degrees there is genuinely free.
#[test]
fn certify_catches_a_damaged_schedule() {
    let o = obj(300, 0.0);
    let r = polish(&o, &seed_from_policy(&o), PolishOpts { max_passes: 20, ..Default::default() });
    let mut hurt = r.pitches.clone();
    hurt[250] += 30.0;
    let res = certify(&o, &hurt, 0.25, Jitter::default());
    assert!(res > 10.0 * r.residual.max(1e-9),
            "damage went unnoticed: clean {:.2e}, damaged {res:.2e}", r.residual);
}

/// Jitter is part of the utility function, so it has to survive the file and it has to be
/// reproducible: the same sigma and seed must give the same perturbations on any machine.
#[test]
fn jitter_is_reproducible_and_round_trips() {
    let j = Jitter { sigma: 0.1, draws: 400, resample: true, seed: 12345 };
    let a = j.draws_at_0();
    assert_eq!(a.len(), 400);
    assert_eq!(a, j.draws_at_0(), "same seed must give the same draws");
    assert_ne!(a, Jitter { seed: 999, ..j }.draws_at_0(), "a different seed must differ");
    // Only v_y and v_z are perturbed; yaw is pinned to 0, so v_x must stay exactly zero.
    assert!(a.iter().all(|d| d.x == 0.0), "jitter must not touch v_x");
    let sq: f64 = a.iter().map(|d| d.y * d.y + d.z * d.z).sum();
    let sd = (sq / 800.0).sqrt();
    assert!((sd - 0.1).abs() < 0.02, "sigma should be about 0.1, measured {sd:.4}");
    assert_eq!(Jitter { sigma: 0.0, ..j }.draws_at_0(), vec![Vec3::ZERO]);

    let o = obj(40, 0.0);
    let r = polish(&o, &seed_from_policy(&o), PolishOpts { max_passes: 2, jitter: j, ..Default::default() });
    let p = Profile { obj: o, trig: trig_mode(), jitter: j, rough: Rough::default(), commit: commit_hash().into(),
                      pitches: r.pitches, residual: r.residual, passes: r.passes };
    assert_eq!(Profile::parse(&p.to_string()).unwrap().jitter, j);
}

/// Polishing under jitter maximizes the *smoothed* objective, and should be judged on it.
///
/// Note what is deliberately not asserted here. Jitter does not punish chatter in general --
/// perturbing an arbitrary chattering schedule *improves* it, because arbitrary chatter is not
/// at a maximum of anything. The claim is narrower: chatter that the optimizer converged to
/// sits on a knife edge in the *state*, and only that kind loses its value once the starting
/// velocity is uncertain. Testing it needs a schedule polished hard enough to have found the
/// singular arc, which is minutes of work, so it lives in the sweep's measurements rather
/// than here.
#[test]
fn polishing_under_jitter_improves_the_smoothed_objective() {
    let o = obj(80, 0.0);
    let jit = Jitter { sigma: 0.1, draws: 6, resample: false, seed: 11 };
    let dv = jit.draws_at_0();
    let seed = seed_from_policy(&o);
    let before = o.eval_jittered(&seed, &dv);
    let r = polish(&o, &seed, PolishOpts { max_passes: 6, tol: 0.0, jitter: jit, ..Default::default() });
    let after = o.eval_jittered(&r.pitches, &dv);
    assert!(after >= before - 1e-9, "smoothed J fell from {before} to {after}");
    for (t, &p) in r.pitches.iter().enumerate() {
        assert_eq!(p, p as f32 as f64, "pitch {t} is not an f32");
        assert!((-90.0..=90.0).contains(&p), "pitch {t} = {p} out of range");
    }
}

/// The projection step, and the trap in it. A median filter looks like the obvious way to strip
/// chatter while keeping the snap, and it is not: an alternation has runs of length one at fifty
/// percent duty, so every window holds a majority of its own center value and the median
/// reproduces the alternation. The local mean removes it. This pins both halves, because the
/// wrong one costs six blocks in practice.
#[test]
fn the_median_reproduces_chatter_and_the_mean_removes_it() {
    let alt: Vec<f64> = (0..40).map(|i| if i % 2 == 0 { 0.0 } else { 90.0 }).collect();
    let m = smooth_median(&alt, 5);
    assert!((m[10] - m[11]).abs() > 80.0, "the median should *keep* a 50% duty alternation");
    let b = smooth_box(&alt, 5);
    assert!((b[10] - b[11]).abs() < 25.0, "the mean should flatten it, got {} {}", b[10], b[11]);

    // and the thing the median is actually good at: a step survives it, but not the mean
    let step: Vec<f64> = (0..40).map(|i| if i < 20 { 10.0 } else { -80.0 }).collect();
    let ms = smooth_median(&step, 5);
    assert_eq!((1..40).filter(|&i| (ms[i] - ms[i - 1]).abs() > 1.0).count(), 1);
    assert!((1..40).filter(|&i| (smooth_box(&step, 5)[i] - smooth_box(&step, 5)[i - 1]).abs() > 1.0)
            .count() > 1);
}

/// `--limit` is a restriction of the admissible set, not a preference, so a seed that arrives
/// outside it must be brought in and must stay in. Both halves of that failed at first: the
/// coordinate search kept an out-of-range incumbent because it out-scored every candidate, and
/// the ternary refine re-expanded its bracket to +-90.
#[test]
fn a_pitch_limit_is_respected_from_a_seed_that_breaks_it() {
    let o = Objective { v0: V0, n: 60, lambda: 0.0 };
    let seed: Vec<f64> = (0..o.n).map(|i| if i % 2 == 0 { -90.0 } else { 90.0 }).collect();
    let rough = Rough { limit: 60.0, ..Rough::default() };
    let r = polish(&o, &seed, PolishOpts { max_passes: 3, tol: 0.0, rough, ..Default::default() });
    let worst = r.pitches.iter().fold(0.0f64, |a, &p| a.max(p.abs()));
    assert!(worst <= 60.0 + 1e-9, "polish left a pitch at {worst}, outside the limit");
}

/// A roughness price is part of the utility function, so a profile has to carry it or the
/// certificate in its header is a claim about a different objective than the one it was
/// written under.
#[test]
fn the_roughness_price_round_trips_through_the_header() {
    let o = Objective { v0: V0, n: 12, lambda: 0.25 };
    let rough = Rough { mu: 1e-3, mu_tv: 0.0, cap: 45.0, slew_cap: f64::INFINITY, limit: 85.0 };
    let p = Profile { obj: o, trig: trig_mode(), jitter: Jitter::default(), rough,
                      commit: commit_hash().into(), pitches: vec![1.0; 12],
                      residual: 1e-7, passes: 3 };
    let back = Profile::parse(&p.to_string()).expect("header should parse");
    assert_eq!(back.rough, rough);
    // and a file written before the field existed still reads, as an unpriced profile
    let older: String = p.to_string().lines().filter(|l| !l.starts_with("# rough"))
        .map(|l| format!("{l}\n")).collect();
    assert_eq!(Profile::parse(&older).expect("old header").rough, Rough::default());
}
