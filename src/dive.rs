use crate::sim::*;

/// assume we're in the dive.
/// so `vy <= 0.0`, `vz >= 0.0`, `pitch >= 0.0`.
/// parent is [`crate::nocast::tick`].
pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    assert_eq!(vel.x, 0.0);
    assert_eq!(rot.y, 0.0);
    let vy0: f64 = vel.y;
    let vz0: f64 = vel.z;
    let pitch: f64 = rot.x as f64;
    let vel = ();
    let rot = ();

    assert!(vy0 <= 0.0);
    assert!(vz0 >= 0.0);
    assert!(pitch >= 0.0);

    // renamed from `lean_angle`
    let pitch_rad: f64 = pitch * (std::f64::consts::PI / 180.0);

    // renamed from `look_angle.z`
    // also renamed from `look_hor_length`
    let pitch_cos: f64 = pitch_rad.cos();
    assert!(pitch_cos >= 0.0);

    // renamed from `lift_force`
    let pitch_cos2: f64 = pitch_cos * pitch_cos;

    let grav = -1.0 + pitch_cos2 * 0.75;
    assert!((-1.0..=-0.25).contains(&grav));
    let vy_grav = vy0 + GRAVITY * grav;

    // converts v_down to v_forward if falling
    let convert: f64 = vy_grav * -0.1 * pitch_cos2;
    assert!(convert >= 0.0);
    let vy_fall = vy_grav + convert;
    let vz_fall = vz0 + convert;

    // turning
    let vz_turn = vz_fall + (vz0 - vz_fall) * 0.1;

    Vec3 {
        x: 0.0,
        y: vy_fall * 0.98,
        z: vz_turn * 0.99,
    }
}
