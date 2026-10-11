use crate::sim::*;

/// assume xv and yaw is zero.
/// parent is [`crate::nocast::tick`].
pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    assert_eq!(vel.x, 0.0);
    assert_eq!(rot.y, 0.0);
    let mut vy = vel.y;
    let mut vz = vel.z;
    let pitch: f64 = rot.x as f64;
    let vel = ();
    let rot = ();

    // renamed from `lean_angle`
    let pitch_rad: f64 = pitch * (std::f64::consts::PI / 180.0);

    // renamed from `look_angle.z`
    let pitch_cos: f64 = pitch_rad.cos();

    let look_hor_length: f64 = (0.0 * 0.0 + pitch_cos * pitch_cos).sqrt();
    let move_hor_length: f64 = (0.0 * 0.0 + vz * vz).sqrt();
    let lift_force: f64 = {
        let t: f64 = pitch_rad.cos();
        t * t
    };
    vy += GRAVITY * (-1.0 + lift_force * 0.75);

    // converts v_down to v_forward
    if vy < 0.0 && look_hor_length > 0.0 {
        let convert: f64 = vy * -0.1 * lift_force;
        assert_eq!(0.0 * convert / look_hor_length, 0.0);
        vy += convert;
        vz += pitch_cos * convert / look_hor_length;
    }

    // converts v_forward to v_up if looking up
    if pitch_rad < 0.0 && look_hor_length > 0.0 {
        let convert: f64 = move_hor_length * -pitch_rad.sin() * 0.04;
        assert_eq!(-0.0 * convert / look_hor_length, 0.0);
        vy += convert * 3.2;
        vz += -pitch_cos * convert / look_hor_length;
    }

    // turning
    if look_hor_length > 0.0 {
        assert_eq!(
            (0.0 / look_hor_length * move_hor_length - 0.0) * 0.1,
            0.0
        );
        vy += 0.0;
        vz += (pitch_cos / look_hor_length * move_hor_length - vz) * 0.1;
    }

    Vec3 {
        x: 0.0,
        y: vy * 0.98,
        z: vz * 0.99,
    }
}
