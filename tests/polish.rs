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
    let p = Profile { obj: o, trig: trig_mode(), commit: commit_hash().into(),
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
    let res = certify(&o, &hurt, 0.25);
    assert!(res > 10.0 * r.residual.max(1e-9),
            "damage went unnoticed: clean {:.2e}, damaged {res:.2e}", r.residual);
}
