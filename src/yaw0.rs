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

    let look_angle: Vec3 = {
        let real_x_rot: f64 = pitch * (std::f64::consts::PI / 180.0);
        let real_y_rot: f64 = 0.0;
        let y_cos: f64 = real_y_rot.cos();
        let y_sin: f64 = real_y_rot.sin();
        let x_cos: f64 = real_x_rot.cos();
        let x_sin: f64 = real_x_rot.sin();
        Vec3 {
            x: y_sin * x_cos,
            y: x_sin,
            z: y_cos * x_cos,
        }
    };
    let lean_angle: f64 = pitch * (std::f64::consts::PI / 180.0);

    let look_hor_length: f64 = (look_angle.x * look_angle.x + look_angle.z * look_angle.z).sqrt();
    let move_hor_length: f64 = (0.0 * 0.0 + vz * vz).sqrt();
    let lift_force: f64 = {
        let t: f64 = lean_angle.cos();
        t * t
    };
    vy += GRAVITY * (-1.0 + lift_force * 0.75);

    // converts v_down to v_forward
    if vy < 0.0 && look_hor_length > 0.0 {
        let convert: f64 = vy * -0.1 * lift_force;
        assert_eq!(look_angle.x * convert / look_hor_length, 0.0);
        vy += convert;
        vz += look_angle.z * convert / look_hor_length;
    }

    // converts v_forward to v_up if looking up
    if lean_angle < 0.0 && look_hor_length > 0.0 {
        let convert: f64 = move_hor_length * -lean_angle.sin() * 0.04;
        assert_eq!(-look_angle.x * convert / look_hor_length, 0.0);
        vy += convert * 3.2;
        vz += -look_angle.z * convert / look_hor_length;
    }

    // turning
    if look_hor_length > 0.0 {
        assert_eq!(
            (look_angle.x / look_hor_length * move_hor_length - 0.0) * 0.1,
            0.0
        );
        vy += 0.0;
        vz += (look_angle.z / look_hor_length * move_hor_length - vz) * 0.1;
    }

    Vec3 {
        x: 0.0,
        y: vy * 0.98,
        z: vz * 0.99,
    }
}
