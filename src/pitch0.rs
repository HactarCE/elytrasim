use crate::sim::*;

/// assume that pitch is 0, and that we're moving forwards.
/// parent is [`crate::yaw0::tick`].
pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    todo!();
    // assert_eq!(vel.x, 0.0);
    // assert_eq!(rot.y, 0.0);
    // let vy0: f64 = vel.y;
    // let vz0: f64 = vel.z;
    // let pitch: f64 = rot.x as f64;
    // let vel = ();
    // let rot = ();

    // assert!(vz0 >= 0.0);
    // assert_eq!(pitch, 0.0);

    // // renamed from `lean_angle`
    // let pitch_rad: f64 = pitch * (std::f64::consts::PI / 180.0);

    // let pitch_sin: f64 = pitch_rad.sin();

    // // renamed from `look_angle.z`
    // // also renamed from `look_hor_length`
    // let pitch_cos: f64 = pitch_rad.cos();
    // assert!(pitch_cos >= 0.0);

    // // renamed from `lift_force`
    // let pitch_cos2: f64 = pitch_cos * pitch_cos;

    // let grav = -1.0 + pitch_cos2 * 0.75;
    // assert!((-1.0..=-0.25).contains(&grav));
    // let vy_grav = vy0 + GRAVITY * grav;

    // // converts v_down to v_forward if falling
    // if vy < 0.0 {
    //     let convert: f64 = vy * -0.1 * pitch_cos2;
    //     assert!(convert >= 0.0);
    //     vy += convert;
    //     vz += convert;
    // }

    // // converts v_forward to v_up if looking up
    // if pitch_rad < 0.0 {
    //     assert!(pitch_sin <= 0.0);
    //     let convert: f64 = vz0 * -pitch_sin * 0.04;
    //     assert!(convert >= 0.0);
    //     vy += convert * 3.2;
    //     vz += -convert;
    // }

    // // turning
    // {
    //     vz += (vz0 - vz) * 0.1;
    // }

    // Vec3 {
    //     x: 0.0,
    //     y: vy * 0.98,
    //     z: vz * 0.99,
    // }
}
