use crate::sim::*;

/// assume xv and yaw is zero.
/// also assumes that `cos` is algebraic.
/// this breaks the LUT.
/// parent is [`crate::yaw0::tick`].
pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    assert_eq!(vel.x, 0.0);
    assert_eq!(rot.y, 0.0);
    let vy0 = vel.y;
    let vz0 = vel.z;
    let pitch = rot.x as f64;
    let vel = ();
    let rot = ();

    assert!(vy0 >= 0.0);
    assert!(vz0 >= 0.0);
    assert!(pitch <= 0.0);

    let pitch_rad = pitch * (std::f64::consts::PI / 180.0);

    let pitch_sin = pitch_rad.sin();
    assert!(pitch_sin <= 0.0);

    let pitch_cos = pitch_rad.cos();
    assert!(pitch_cos >= 0.0);

    // renamed from `lift_force`
    let pitch_cos2 = pitch_cos * pitch_cos;

    let grav = -1.0 + pitch_cos2 * 0.75;
    assert!((-1.0..=-0.25).contains(&grav));
    let vy_grav = vy0 + GRAVITY * grav;

    assert!(vy_grav >= 0.0);

    // converts v_forward to v_up if looking up
    assert!(pitch_sin <= 0.0);
    let convert = vz0 * -pitch_sin * 0.04;
    assert!(convert >= 0.0);
    let vy_up = vy_grav + convert * 3.2;
    let vz_up = vz0 - convert;

    // turning
    let vz_turn = vz_up + (vz0 - vz_up) * 0.1;

    Vec3 {
        x: 0.0,
        y: vy_up * 0.98,
        z: vz_turn * 0.99,
    }
}
