use crate::sim::*;

/// assume we're in the dive.
/// so `vy <= 0.0`, `vz >= 0.0`, `pitch >= 0.0`.
/// parent is [`crate::nocast::tick`].
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

    let pitch_rad = pitch * (std::f64::consts::PI / 180.0);

    let pitch_cos = pitch_rad.cos();
    assert!(pitch_cos >= 0.0);

    // renamed from `lift_force`
    let pitch_cos2 = pitch_cos * pitch_cos;

    let grav = -1.0 + pitch_cos2 * 0.75;
    let vy_grav = vy0 + GRAVITY * grav;
    
    assert!((-1.0..=-0.25).contains(&grav));

    // converts v_down to v_forward if falling
    let convert = vy_grav * -0.1 * pitch_cos2;
    let vy_fall = vy_grav + convert;
    let vz_turn = vz0 + convert * 0.9;

    assert!(convert >= 0.0);

    Vec3 {
        x: 0.0,
        y: vy_fall * 0.98,
        z: vz_turn * 0.99,
    }
}

/// for `theta` in `[0, pi/2]`
/// agrees with `cos` and `cos'` at `0` and `pi/2`.
/// `cubic_cos(0) = 1`
/// `cubic_cos'(0) = 0`
/// `cubic_cos(pi/2) = 0`
/// `cubic_cos'(pi/2) = -1`
/// (-4 (π - 4) / π^3) x^3 + (2 (π - 6) / π^2) x^2 + 1
fn cubic_cos(theta: f64) -> f64 {
    use std::f64::consts::PI;
    assert!((0.0..=PI / 2.0).contains(&theta));
    const A: f64 = -4.0 * (PI - 4.0) / (PI * PI * PI);
    const B: f64 = 2.0 * (PI - 6.0) / (PI * PI);
    A * theta.powi(3) + B * theta.powi(2) + 1.0
}

/// for `theta` in `[0, pi/2]`
/// agrees with `cos^2` and `(cos^2)'` at `0` and `pi/2`.
/// `cubic_cos(0) = 1`
/// `cubic_cos'(0) = 0`
/// `cubic_cos(pi/2) = 0`
/// `cubic_cos'(pi/2) = -1`
/// (16 / π^3) x^3 - (12 / π^2) x^2 + 1
fn cubic_cos2(theta: f64) -> f64 {
    use std::f64::consts::PI;
    assert!((0.0..=PI / 2.0).contains(&theta));
    const A: f64 = 16.0 / (PI * PI * PI);
    const B: f64 = -12.0 / (PI * PI);
    A * theta.powi(3) + B * theta.powi(2) + 1.0
}
