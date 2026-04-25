/// Emulation of `Mth.java`.
///
/// Currently this uses Rust's floating-point routines, but a proper exact
/// emulation should mimic the lookup tables of `Mth.java`.
pub struct Mth;

impl Mth {
    pub fn sin(x: f32) -> f32 {
        x.sin()
    }
    pub fn cos(x: f32) -> f32 {
        x.cos()
    }

    pub fn square(x: f64) -> f64 {
        x * x
    }
}
