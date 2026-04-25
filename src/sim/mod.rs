mod entity;
mod mth;
mod orig_entity;
mod rot;
mod vec3;

pub use entity::Entity;
pub use mth::Mth;
pub use rot::Rot;
pub use vec3::Vec3;

pub const GRAVITY: f64 = 0.08; // m/tick/tick
