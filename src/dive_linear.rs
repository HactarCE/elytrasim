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

    let pitch_cos2 = -pitch_rad + (PI + 2.0) / 4.0;
    assert!((0.0..=1.0).contains(&pitch_cos2));

    let grav = -1.0 + pitch_cos2 * 0.75;
    assert!((-1.0..=-0.25).contains(&grav));
    let vy_grav = vy0 + GRAVITY * grav;

    // converts v_down to v_forward if falling
    let convert = vy_grav * -0.1 * pitch_cos2;
    assert!(convert >= 0.0);
    // let vy_fall = vy_grav + convert;
    let vy_fall = vy_grav * (1.0 - 0.1 * pitch_cos2);

    // turning
    let vz_turn = vz0 + convert * 0.9;

    Vec3 {
        x: 0.0,
        y: vy_fall * 0.98,
        z: vz_turn * 0.99,
    }
}
