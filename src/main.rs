mod frange;
mod inlined;
mod nocast;
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
    let vxs_zero = vec![-0.0, 0.0];

    let vxs_nonzero = {
        let vxs_normal = Frange::from_n(-2.0, 2.0, 5);
        let vxs_near_zero = Frange::from_n(-0.05, 0.05, 2);
        let mut vxs = [vxs_normal, vxs_near_zero].iter().flatten().collect_vec();
        vxs.extend_from_slice(&[-0.0, 0.0]);
        vxs.sort_by(f64::total_cmp);
        vxs.dedup_by(exact_eq);
        vxs
    };

    let vys = {
        let vys_big = Frange::from_stride(-2.0, 4.0, 0.2);
        let vys_normal = Frange::from_stride(-1.0, 2.0, 0.1);
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
        let vzs_big = Frange::from_stride(-2.0, 4.0, 0.2);
        let vzs_normal = Frange::from_stride(-0.25, 3.0, 0.1);
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
        let pitches_normal = Frange::from_stride(-89.0, 89.0, 4.0);
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

    let yaws_zero = vec![-0.0, 0.0];

    // note yaws get wrapped to [-180.0, 180) on the server,
    // but physics happens on the client.
    let yaws_nonzero = {
        let yaws_big = Frange::from_n(-400.0, 400.0, 5);
        let yaws_normal = Frange::from_n(-190.0, 190.0, 3);
        let yaws_near_zero = Frange::from_n(-0.05, 0.05, 2);
        let mut yaws = [yaws_big, yaws_normal, yaws_near_zero]
            .iter()
            .flatten()
            .collect_vec();
        yaws.extend_from_slice(&[-0.0, 0.0]);
        yaws.sort_by(f64::total_cmp);
        yaws.dedup_by(exact_eq);
        yaws
    };

    // assert they contain +-0.0
    for it in [
        &vxs_zero,
        &vxs_nonzero,
        &vys,
        &vzs,
        &pitches,
        &yaws_zero,
        &yaws_nonzero,
    ] {
        assert!(it.contains(&-0.0));
        assert!(it.contains(&0.0));
    }

    // print parameter counts
    {
        println!();
        println!("vxs_zero:     {}", vxs_zero.len());
        println!("vxs_nonzero:  {}", vxs_nonzero.len());
        println!("vys:          {}", vys.len());
        println!("vzs:          {}", vzs.len());
        println!("pitches:      {}", pitches.len());
        println!("yaws_zero:    {}", yaws_zero.len());
        println!("yaws_nonzero: {}", yaws_nonzero.len());
    }

    let dom_zero = Domain {
        vxs: vxs_zero,
        vys: vys.clone(),
        vzs: vzs.clone(),
        pitches: pitches.clone(),
        yaws: yaws_zero,
    };

    let dom_nonzero = Domain {
        vxs: vxs_nonzero,
        vys,
        vzs,
        pitches,
        yaws: yaws_nonzero,
    };

    // print dom counts
    {
        println!();
        println!("dom_zero:    {}", dom_zero.count());
        println!("dom_nonzero: {}", dom_nonzero.count());
        println!(
            "ratio:       {}",
            dom_nonzero.count() as f64 / dom_zero.count() as f64
        );
    }

    let tol = 1e-6;

    // `reference` and `inlined` have exact equality in release,
    // but not in debug.
    // assert!(
    //     disagreements(reference::tick, inlined::tick, tol, &dom_nonzero)
    //         .cases
    //         .is_empty()
    // );
    // assert_eq!(
    //     disagreements(reference::tick, inlined::tick, tol, &dom_nonzero).max_dv,
    //     Vec3::ZERO,
    //     "inlining is exact"
    // );

    assert!(
        disagreements(reference::tick, nocast::tick, tol, &dom_nonzero)
            .cases
            .is_empty()
    );

    println!();
    dbg!(disagreements(
        inlined::tick,
        nocast::tick,
        tol,
        &dom_nonzero
    ));
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
impl Domain {
    fn count(&self) -> usize {
        self.vxs.len() * self.vys.len() * self.vzs.len() * self.pitches.len() * self.yaws.len()
    }
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
