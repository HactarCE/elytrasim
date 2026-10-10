use std::ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign};

pub type Pos = f64;
pub type Pos3 = Vec3;
pub type Vel = f64;
pub type Vel3 = Vec3;
pub type Acc = f64;
pub type Acc3 = Vec3;

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn horizontal_distance(self) -> f64 {
        (self.x * self.x + self.z * self.z).sqrt()
    }

    pub fn length_sq(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn length(self) -> f64 {
        self.length_sq().sqrt()
    }

    /// coord-wise abs diff.
    pub fn abs_diff(self, rhs: Self) -> Self {
        Self {
            x: (self.x - rhs.x).abs(),
            y: (self.y - rhs.y).abs(),
            z: (self.z - rhs.z).abs(),
        }
    }

    /// whether they're abs_diff_eq on all coords.
    pub fn abs_diff_eq(self, rhs: Self, tol: f64) -> bool {
        approx::abs_diff_eq!(self.x, rhs.x, epsilon = tol)
            && approx::abs_diff_eq!(self.y, rhs.y, epsilon = tol)
            && approx::abs_diff_eq!(self.z, rhs.z, epsilon = tol)
    }

    /// coord-wise max
    pub fn max(self, rhs: Self) -> Self {
        Self {
            x: self.x.max(rhs.x),
            y: self.y.max(rhs.y),
            z: self.z.max(rhs.z),
        }
    }
}

// impl Vel3 {
//     pub fn yz_to_egui_vec2(self) -> egui::Vec2 {
//         egui::vec2(self.z as f32, -self.y as f32)
//     }
// }

impl AddAssign for Vec3 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Add for Vec3 {
    type Output = Vec3;

    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Sub for Vec3 {
    type Output = Vec3;

    fn sub(mut self, rhs: Self) -> Self::Output {
        self -= rhs;
        self
    }
}

impl MulAssign for Vec3 {
    fn mul_assign(&mut self, rhs: Self) {
        self.x *= rhs.x;
        self.y *= rhs.y;
        self.z *= rhs.z;
    }
}

impl Mul for Vec3 {
    type Output = Vec3;

    fn mul(mut self, rhs: Self) -> Self::Output {
        self *= rhs;
        self
    }
}
