use std::f64::consts::PI;

use crate::sim::*;

/// assume we're in the dive.
/// so `vy <= 0.0`, `vz >= 0.0`, `pitch >= 0.0`.
/// except we use a linear approximation for cos2.
/// parent is [`crate::dive::tick`].
pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    assert_eq!(vel.x, 0.0);
    assert_eq!(rot.y, 0.0);
    let vy0 = vel.y;
    let vz0 = vel.z;
    let pitch = rot.x as f64;
    let vel = ();
    let rot = ();

    assert!(vy0 <= 0.0);
    assert!(vz0 >= 0.0);
    assert!(pitch >= 0.0);

    let pitch_rad = pitch * (PI / 180.0);

    // converts v_down to v_forward if falling
    const Y_PITCH2: f64 = -0.075 * GRAVITY;
    const Y_PITCH: f64 = GRAVITY * (0.0375 * PI - 0.775);
    const Y_VY_PITCH: f64 = 0.1;
    const Y_VY: f64 = 0.95 - 0.025 * PI;
    const Y_OFFSET: f64 = GRAVITY * (-0.0046875 * PI * PI + 0.19375 * PI - 0.59375);
    let vy_fall = Y_PITCH2 * pitch_rad.powi(2)
        + Y_PITCH * pitch_rad
        + Y_VY_PITCH * vy0 * pitch_rad
        + Y_VY * vy0
        + Y_OFFSET;

    // turning
    const Z_PITCH2: f64 = -0.0675 * GRAVITY;
    const Z_PITCH: f64 = GRAVITY * (0.03375 * PI - 0.0225);
    const Z_VY_PITCH: f64 = 0.09;
    const Z_VY: f64 = -0.0225 * PI - 0.045;
    const Z_OFFSET: f64 = GRAVITY * (-0.00421875 * PI * PI + 0.005625 * PI + 0.028125);
    let vz_turn = vz0
        + Z_PITCH2 * pitch_rad.powi(2)
        + Z_PITCH * pitch_rad
        + Z_VY_PITCH * vy0 * pitch_rad
        + Z_VY * vy0
        + Z_OFFSET;

    Vec3 {
        x: 0.0,
        y: vy_fall * 0.98,
        z: vz_turn * 0.99,
    }
}
