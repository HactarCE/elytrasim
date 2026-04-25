use std::f64::consts::PI;

use super::{Mth, Vec3};

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Rot {
    /// Pitch
    pub x: f32,
    /// Yaw
    pub y: f32,
}

impl Rot {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn look_angle(&self) -> Vec3 {
        let real_x_rot = self.x * (PI / 180.0) as f32;
        let real_y_rot = -self.y * (PI / 180.0) as f32;
        let y_cos = Mth::cos(real_y_rot);
        let y_sin = Mth::sin(real_y_rot);
        let x_cos = Mth::cos(real_x_rot);
        let x_sin = Mth::sin(real_x_rot);
        Vec3::new((y_sin * x_cos) as f64, x_sin as f64, (y_cos * x_cos) as f64)
    }
}
