mod frange;
mod inlined;
mod reference;
mod rot0;
mod sim;

use itertools::Itertools;

use crate::{frange::*, sim::*};

/// returns the new state
pub type Ticker = fn(vel: Vec3, pitch: Rot) -> Vec3;

// TODO: lut weirdness
// pub const LUT_STRIDE

fn main() {
    let vxs = vec![-0.0, 0.0];

    let vys = {
        let vys_big = Frange::from_stride(-2.0, 4.0, 0.1);
        let vys_normal = Frange::from_stride(-1.0, 2.0, 0.05);
        let vys_near_zero = Frange::from_stride(-0.1, 0.1, 0.01);
        let mut vys = [vys_big, vys_normal, vys_near_zero]
            .iter()
            .flatten()
            .collect_vec();
        vys.extend_from_slice(&[-0.0, 0.0]);
        vys.sort_by(f64::total_cmp);
        vys.dedup_by(exact_eq);
        vys
    };

    let vzs = {
        let vzs_big = Frange::from_stride(-2.0, 4.0, 0.1);
        let vzs_normal = Frange::from_stride(-0.25, 3.0, 0.05);
        let vzs_near_zero = Frange::from_stride(-0.1, 0.1, 0.01);
        let mut vzs = [vzs_big, vzs_normal, vzs_near_zero]
            .iter()
            .flatten()
            .collect_vec();
        vzs.extend_from_slice(&[-0.0, 0.0]);
        vzs.sort_by(f64::total_cmp);
        vzs.dedup_by(exact_eq);
        vzs
    };

    // TODO: +-90 pitch weirdness
    let pitches = {
        let pitches_normal = Frange::from_stride(-89.0, 89.0, 0.5);
        let pitches_near_zero = Frange::from_stride(-0.1, 0.1, 0.01);
        let mut pitches = [pitches_normal, pitches_near_zero]
            .iter()
            .flatten()
            .collect_vec();
        pitches.extend_from_slice(&[-0.0, 0.0]);
        pitches.sort_by(f64::total_cmp);
        pitches.dedup_by(exact_eq);
        pitches
    };

    let yaws = vec![-0.0, 0.0];

    for it in [&vxs, &vys, &vzs, &pitches, &yaws] {
        assert!(it.contains(&-0.0));
        assert!(it.contains(&0.0));
    }

    let dom = Domain {
        vxs,
        vys,
        vzs,
        pitches,
        yaws,
    };

    let tol = 1e-6;

    assert_eq!(
        disagreements(reference::tick, inlined::tick, tol, &dom).max_dv,
        Vec3::ZERO,
        "inlining is exact"
    );

    dbg!(disagreements(reference::tick, inlined::tick, tol, &dom));
}

/// to avoid `-0.0` and `+0.0` collapsing in `dedup`.
fn exact_eq(l: &mut f64, r: &mut f64) -> bool {
    l.to_bits() == r.to_bits()
}

fn disagreements(lhs: Ticker, rhs: Ticker, tol: f64, dom: &Domain) -> Report {
    let mut max_dv = Vec3::ZERO;
    let mut cases = Vec::new();
    for (&vx, &vy, &vz) in itertools::iproduct!(&dom.vxs, &dom.vys, &dom.vzs) {
        let vel = Vec3 {
            x: vx,
            y: vy,
            z: vz,
        };
        for (&pitch, &yaw) in itertools::iproduct!(&dom.pitches, &dom.yaws) {
            let rot = Rot {
                x: pitch as f32,
                y: yaw as f32,
            };
            let lhs = lhs(vel, rot);
            let rhs = rhs(vel, rot);
            max_dv = max_dv.max(lhs.abs_diff(rhs));
            if !lhs.abs_diff_eq(rhs, tol) {
                cases.push(Case { vel, rot, lhs, rhs });
            }
        }
    }

    Report { max_dv, cases }
}

#[derive(Debug, Clone)]
struct Domain {
    vxs: Vec<f64>,
    vys: Vec<f64>,
    vzs: Vec<f64>,
    pitches: Vec<f64>,
    yaws: Vec<f64>,
}

#[derive(Debug, Clone)]
struct Case {
    vel: Vec3,
    rot: Rot,
    lhs: Vec3,
    rhs: Vec3,
}

#[derive(Debug, Clone)]
struct Report {
    /// the max abs diff
    max_dv: Vec3,
    /// the cases that exceeded some tolerance
    cases: Vec<Case>,
}
