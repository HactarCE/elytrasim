//! The price-aware gain-phase lookahead, `argmax dJ over n held ticks`.
//!
//! Two properties the write-up in README-myopic.md leans on, and neither is obvious from the
//! code: that `w = 0` recovers `bug_dte_n` *exactly* (it is the identity that catches a price
//! accidentally supplied in a lookahead's argument slot), and that the batched
//! `dj_argmax_all` -- which shares one held rollout across every horizon -- agrees with the
//! one-horizon-at-a-time `bug_dj_n` it stands in for.

use elytrasim::opt::*;
use elytrasim::sim::*;

/// A spread of climbing states: nose-up arcs at the forward speeds the gain phase actually sees.
fn states() -> Vec<State> {
    [(0.4, 2.4), (1.0, 1.5), (1.8, 0.9), (2.2, 0.5), (0.2, 3.0)]
        .iter()
        .map(|&(vy, vz)| State { pos: Vec3::ZERO, vel: Vec3::new(0.0, vy, vz) })
        .collect()
}

#[test]
fn at_zero_price_dj_is_dte() {
    for s in states() {
        for n in [1usize, 5, 20, 48] {
            assert_eq!(bug_dj_n(&s, n, 0.0, 90.0), bug_dte_n(&s, n),
                       "w = 0 must be the same expression, at n = {n}");
        }
    }
}

#[test]
fn batched_argmax_matches_the_one_at_a_time_rule() {
    const NMAX: usize = 48;
    for s in states() {
        for &w in &[-0.130303, 0.0, 0.130303, 0.260606] {
            let all = dj_argmax_all(&s, NMAX, w, 85.0);
            for n in 1..=NMAX {
                let one = bug_dj_n(&s, n, w, 85.0);
                // Both refine inside the same 0.125 deg cell of the same grid, so they can only
                // differ by the ternary search's own tolerance.
                assert!((all[n - 1] - one).abs() < 1e-3,
                        "w {w}, n {n}: batched {} vs one-at-a-time {one}", all[n - 1]);
            }
        }
    }
}

#[test]
fn the_pitch_bound_is_respected() {
    for s in states() {
        for n in [1usize, 20] {
            let p = bug_dj_n(&s, n, 0.260606, 85.0);
            assert!(p >= -85.0 && p <= 85.0, "n = {n} left the control set at {p}");
        }
    }
}
