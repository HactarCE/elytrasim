use crate::sim::*;

/// new vels rather than mut vel.
/// parent is [`crate::yaw0::tick`].
pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    assert_eq!(vel.x, 0.0);
    assert_eq!(rot.y, 0.0);
    let vy0 = vel.y;
    let vz0 = vel.z;
    let pitch = rot.x as f64;
    let vel = ();
    let rot = ();

    // renamed from `lean_angle`
    let pitch_rad = pitch * (std::f64::consts::PI / 180.0);

    let pitch_sin = pitch_rad.sin();

    // renamed from `look_angle.z`
    // also renamed from `look_hor_length`
    let pitch_cos = pitch_rad.cos();
    assert!(pitch_cos >= 0.0);

    let move_hor_length = vz0.abs();
    // renamed from `lift_force`
    let pitch_cos2 = pitch_cos * pitch_cos;

    let grav = -1.0 + pitch_cos2 * 0.75;
    assert!((-1.0..=-0.25).contains(&grav));
    let vy_grav = vy0 + GRAVITY * grav;

    // converts v_down to v_forward if falling
    let (vy_fall, vz_fall) = if vy_grav < 0.0 {
        let convert = vy_grav * -0.1 * pitch_cos2;
        assert!(convert >= 0.0);
        let vy_fall = vy_grav + convert;
        let vz_fall = vz0 + convert;
        (vy_fall, vz_fall)
    } else {
        (vy_grav, vz0)
    };

    // converts v_forward to v_up if looking up
    let (vy_up, vz_up) = if pitch_rad < 0.0 {
        assert!(pitch_sin <= 0.0);
        let convert = move_hor_length * -pitch_sin * 0.04;
        assert!(convert >= 0.0);
        let vy_up = vy_fall + convert * 3.2;
        let vz_up = vz_fall - convert;
        (vy_up, vz_up)
    } else {
        (vy_fall, vz_fall)
    };

    // turning
    let vz_turn = {
        // let vz_turn = vz_up + (move_hor_length - vz_up) * 0.1;
        let vz_turn = vz_up * 0.9 + move_hor_length * 0.1;
        vz_turn
    };

    Vec3 {
        x: 0.0,
        y: vy_up * 0.98,
        z: vz_turn * 0.99,
    }
}
