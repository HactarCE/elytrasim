use crate::sim::*;

/// inline methods.
/// parent is [`crate::reference::tick`]
pub fn tick(mut vel: Vec3, rot: Rot) -> Vec3 {
    let look_angle: Vec3 = {
        let real_x_rot: f32 = rot.x * (std::f64::consts::PI / 180.0) as f32;
        let real_y_rot: f32 = -rot.y * (std::f64::consts::PI / 180.0) as f32;
        let y_cos: f32 = real_y_rot.cos();
        let y_sin: f32 = real_y_rot.sin();
        let x_cos: f32 = real_x_rot.cos();
        let x_sin: f32 = real_x_rot.sin();
        Vec3 {
            x: (y_sin * x_cos) as f64,
            y: x_sin as f64,
            z: (y_cos * x_cos) as f64,
        }
    };
    let lean_angle: f32 = rot.x * (std::f64::consts::PI / 180.0) as f32;

    let look_hor_length: f64 = (look_angle.x * look_angle.x + look_angle.z * look_angle.z).sqrt();
    let move_hor_length: f64 = (vel.x * vel.x + vel.z * vel.z).sqrt();
    let gravity: f64 = GRAVITY;
    let lift_force: f64 = {
        let t: f64 = (lean_angle as f64).cos();
        t * t
    };
    vel.y += gravity * (-1.0 + lift_force * 0.75);

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
        let convert: f64 = move_hor_length * -lean_angle.sin() as f64 * 0.04;
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
        x: 0.99_f32 as f64,
        y: 0.98_f32 as f64,
        z: 0.99_f32 as f64,
    }
}
