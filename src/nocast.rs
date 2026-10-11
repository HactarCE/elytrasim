use crate::sim::*;

/// do everything in `f64`.
/// parent is [`crate::inlined::tick`].
pub fn tick(mut vel: Vec3, rot: Rot) -> Vec3 {
    let pitch: f64 = rot.x as f64;
    let yaw: f64 = rot.y as f64;
    let rot = ();

    let look_angle: Vec3 = {
        let real_x_rot: f64 = pitch * (std::f64::consts::PI / 180.0);
        let real_y_rot: f64 = -yaw * (std::f64::consts::PI / 180.0);
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
    let move_hor_length: f64 = (vel.x * vel.x + vel.z * vel.z).sqrt();
    let lift_force: f64 = {
        let t: f64 = lean_angle.cos();
        t * t
    };
    vel.y += GRAVITY * (-1.0 + lift_force * 0.75);

    // converts v_down to v_forward
    if vel.y < 0.0 && look_hor_length > 0.0 {
        let convert: f64 = vel.y * -0.1 * lift_force;
        vel += Vec3 {
            x: look_angle.x * convert / look_hor_length,
            y: convert,
            z: look_angle.z * convert / look_hor_length,
        };
    }

    // converts v_forward to v_up if looking up
    if lean_angle < 0.0 && look_hor_length > 0.0 {
        let convert: f64 = move_hor_length * -lean_angle.sin() * 0.04;
        vel += Vec3 {
            x: -look_angle.x * convert / look_hor_length,
            y: convert * 3.2,
            z: -look_angle.z * convert / look_hor_length,
        };
    }

    // turning
    if look_hor_length > 0.0 {
        vel += Vec3 {
            x: (look_angle.x / look_hor_length * move_hor_length - vel.x) * 0.1,
            y: 0.0,
            z: (look_angle.z / look_hor_length * move_hor_length - vel.z) * 0.1,
        };
    }

    vel * Vec3 {
        x: 0.99,
        y: 0.98,
        z: 0.99,
    }
}
