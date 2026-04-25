use std::f64::consts::PI;

use super::{GRAVITY, Mth, Vec3};

pub struct Entity {
    pub pos: Vec3,
    pub vel: Vec3,
    pub look_angle: Vec3,
    pub x_rot: f32,
    pub y_rot: f32,
}

impl Entity {
    pub fn travel(&mut self) {
        self.vel = self.update_fall_flying_movement(self.vel);
        self.mov(self.vel);
    }

    pub fn update_fall_flying_movement(&self, mut movement: Vec3) -> Vec3 {
        let look_angle: Vec3 = self.look_angle();
        let lean_angle: f32 = self.x_rot * (PI / 180.0) as f32;

        let look_hor_length: f64 =
            (look_angle.x * look_angle.x + look_angle.z * look_angle.z).sqrt();
        let move_hor_length: f64 = movement.horizontal_distance();
        let gravity: f64 = GRAVITY;
        let lift_force: f64 = Mth::square((lean_angle as f64).cos());
        movement.y += gravity * (-1.0 + lift_force * 0.75);
        if movement.y < 0.0 && look_hor_length > 0.0 {
            let convert: f64 = movement.y * -0.1 * lift_force;
            movement += Vec3::new(
                look_angle.x * convert / look_hor_length,
                convert,
                look_angle.z * convert / look_hor_length,
            );
        }

        if lean_angle < 0.0 && look_hor_length > 0.0 {
            let convert: f64 = move_hor_length * -Mth::sin(lean_angle) as f64 * 0.04;
            movement += Vec3::new(
                -look_angle.x * convert / look_hor_length,
                convert * 3.2,
                -look_angle.z * convert / look_hor_length,
            );
        }

        if look_hor_length > 0.0 {
            movement += Vec3::new(
                (look_angle.x / look_hor_length * move_hor_length - movement.x) * 0.1,
                0.0,
                (look_angle.z / look_hor_length * move_hor_length - movement.z) * 0.1,
            );
        }

        movement * Vec3::new(0.99_f32 as f64, 0.98_f32 as f64, 0.99_f32 as f64)
    }

    pub fn look_angle(&self) -> Vec3 {
        let real_x_rot = self.x_rot * (PI / 180.0) as f32;
        let real_y_rot = -self.y_rot * (PI / 180.0) as f32;
        let y_cos = Mth::cos(real_y_rot);
        let y_sin = Mth::cos(real_y_rot);
        let x_cos = Mth::cos(real_x_rot);
        let x_sin = Mth::cos(real_x_rot);
        Vec3::new((y_sin * x_cos) as f64, x_sin as f64, (y_cos * x_cos) as f64)
    }

    pub fn mov(&mut self, delta: Vec3) {
        let pos = self.pos;
        let new_pos = pos + delta;
        self.pos = new_pos;
    }

    pub fn effective_gravity(&self) -> f64 {
        0.08
    }
}
