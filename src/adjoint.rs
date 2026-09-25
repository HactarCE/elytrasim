//! Reverse-mode gradients of a replay's outcome with respect to every pitch: one forward replay,
//! one backward sweep, O(n).
//!
//! A replay is `s_{t+1} = f(s_t, p_t)` over the state `(y, z, v_y, v_z)`, with
//! `v_{t+1} = F(v_t, p_t)` (`fall_flying_partials`) and `pos_{t+1} = pos_t + v_{t+1}`. Given a
//! scalar `U` of the states and its partials `dU/ds_t` (the *seeds*), the costate
//! `lam_t = dU/ds_t + (ds_{t+1}/ds_t)^T lam_{t+1}` gives `dU/dp_t = (ds_{t+1}/dp_t)^T lam_{t+1}`.
//!
//! What "the gradient" means here is set by `fall_flying_partials`: trig is differentiated as
//! if it were smooth, and each branch of the flight kernel contributes only where the replay
//! took it. Under `mth_lut` this is the gradient of the physics the table approximates; check it
//! against finite differences only under `libm`, where the physics is smooth between branches.

use crate::opt::Objective;
use crate::sim::*;

/// A cotangent on one state: `dU/d(y, z, v_y, v_z)`.
pub type Cot = [f64; 4];

/// `dU/dp` for every pitch, from the replay `states` of `pitches` (`states.len() ==
/// pitches.len() + 1`, as `replay_from` returns) and the seeds `(t, dU/ds_t)`. A pitch after
/// the last seeded state gets 0: nothing downstream of it is scored.
pub fn backprop(pitches: &[f64], states: &[State], seeds: &[(usize, Cot)]) -> Vec<f64> {
    backprop_costates(pitches, states, seeds).0
}

/// `backprop`, also returning each tick's velocity costate `dU/dv_{t+1}` (the total one, through
/// the position too), so a caller can price a different `dv_{t+1}/dp_t` at that tick -- the
/// other side of a branch, say -- without another sweep: `dU/dp_t = a_t . dv_{t+1}/dp_t`.
pub fn backprop_costates(pitches: &[f64], states: &[State], seeds: &[(usize, Cot)]) -> (Vec<f64>, Vec<[f64; 2]>) {
    assert_eq!(states.len(), pitches.len() + 1, "backprop wants the replay of these pitches");
    let mut g = vec![0.0; pitches.len()];
    let mut av = vec![[0.0; 2]; pitches.len()];
    let Some(last) = seeds.iter().map(|s| s.0).max() else { return (g, av) };
    let mut lam: Cot = [0.0; 4];
    for t in (0..=last).rev() {
        for (i, c) in seeds { if *i == t { for j in 0..4 { lam[j] += c[j] } } }
        if t == 0 { break }
        // s_t = f(s_{t-1}, p_{t-1}).
        let (_, d) = fall_flying_partials(states[t - 1].vel, pitches[t - 1]);
        // pos_t = pos_{t-1} + v_t, so v_t's total cotangent picks up the position's.
        let (avy, avz) = (lam[2] + lam[0], lam[3] + lam[1]);
        g[t - 1] = avy * d[0][2] + avz * d[1][2];
        av[t - 1] = [avy, avz];
        lam = [lam[0], lam[1], avy * d[0][0] + avz * d[1][0], avy * d[0][1] + avz * d[1][1]];
    }
    (g, av)
}

/// `d TE / d(y, z, v_y, v_z)`, with `TE = y + |v|^2 / 2g` in blocks (`State::total_energy`).
pub fn te_cot(s: &State) -> Cot { [1.0, 0.0, s.vel.y / GRAVITY, s.vel.z / GRAVITY] }

/// The polish's terminal objective `J = TE(s_n) + w z_n` and its gradient in every pitch.
pub fn j_grad(obj: &Objective, pitches: &[f64]) -> (f64, Vec<f64>) {
    let st = obj.replay(pitches);
    let s = st.last().unwrap();
    let mut c = te_cot(s);
    c[1] += obj.w();
    (obj.j(s), backprop(pitches, &st, &[(pitches.len(), c)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    // These run under the default trig mode, `libm`, which no unit test in this crate changes:
    // smooth between branches, so finite differences are a fair check. Under `mth_lut` they
    // would be differences of a staircase.

    /// Pitches kept at least a degree from 0 (the forward-to-up switch) and dyadic, so that
    /// `p +- h` round-trips through `f32` exactly and the only noise is `f32` trig.
    fn schedule(n: usize, seed: u64) -> Vec<f64> {
        let mut x = seed;
        (0..n).map(|t| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r = (x >> 40) as f64 / (1u64 << 24) as f64;            // [0, 1)
            let base = if t % 40 < 25 { 12.0 } else { -35.0 };        // glide, then pull
            let q = base + 8.0 * (r - 0.5);
            (q * 64.0).round() / 64.0
        }).collect()
    }

    pub(crate) fn fd_check(f: &dyn Fn(&[f64]) -> f64, g: &[f64], p: &[f64], ticks: &[usize]) -> f64 {
        let h = 1.0 / 16.0;
        let mut worst: f64 = 0.0;
        for &t in ticks {
            let (mut a, mut b) = (p.to_vec(), p.to_vec());
            a[t] += h;
            b[t] -= h;
            let fd = (f(&a) - f(&b)) / (2.0 * h);
            let err = (fd - g[t]).abs() / g[t].abs().max(1e-3);
            worst = worst.max(err);
        }
        worst
    }

    #[test]
    fn partials_match_finite_differences() {
        // Each branch combination: dive on/off, climb on/off.
        for &(vy, vz, p) in &[(-0.5, 1.2, 20.0), (-0.5, 1.2, -20.0), (0.6, 1.0, -30.0), (0.6, 1.0, 10.0)] {
            let v = Vec3::new(0.0, vy, vz);
            let (_, d) = fall_flying_partials(v, p);
            let h = 1e-6;
            let at = |vy: f64, vz: f64| fall_flying_partials(Vec3::new(0.0, vy, vz), p).0;
            let (a, b) = (at(vy + h, vz), at(vy - h, vz));
            let (c, e) = (at(vy, vz + h), at(vy, vz - h));
            let fd = [[(a.y - b.y) / (2.0 * h), (c.y - e.y) / (2.0 * h)], [(a.z - b.z) / (2.0 * h), (c.z - e.z) / (2.0 * h)]];
            for r in 0..2 { for k in 0..2 {
                assert!((fd[r][k] - d[r][k]).abs() < 1e-7, "v ({vy}, {vz}) p {p}: [{r}][{k}] fd {} vs {}", fd[r][k], d[r][k]);
            } }
            let hp = 1.0 / 16.0;
            let (a, b) = (fall_flying_partials(v, p + hp).0, fall_flying_partials(v, p - hp).0);
            for (r, (x, y)) in [(a.y, b.y), (a.z, b.z)].into_iter().enumerate() {
                let fd = (x - y) / (2.0 * hp);
                assert!((fd - d[r][2]).abs() < 1e-4 * d[r][2].abs().max(1e-3), "p {p}: d/dp [{r}] fd {fd} vs {}", d[r][2]);
            }
        }
    }

    #[test]
    fn j_gradient_matches_finite_differences() {
        let obj = Objective { v0: Vec3::new(0.0, 0.0, 0.8), n: 120, lambda: 20.0 };
        let p = schedule(obj.n, 7);
        let (_, g) = j_grad(&obj, &p);
        let worst = fd_check(&|q| obj.eval(q), &g, &p, &[0, 1, 17, 30, 59, 60, 88, 119]);
        assert!(worst < 1e-4, "worst relative error {worst:.2e}");
    }
}
