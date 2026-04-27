mod entity;
mod mth;
mod rot;
mod state;
mod vec3;

pub use entity::Entity;
pub use entity::update_fall_flying_movement;
pub use mth::Mth;
pub use rot::{Pitch, Rot, Yaw};
pub use state::{
    DeltaKineticEnergy, DeltaPotentialEnergy, DeltaTotalEnergy, KineticEnergy, PotentialEnergy,
    State, TotalEnergy,
};
pub use vec3::{Acc, Acc3, Pos, Pos3, Vec3, Vel, Vel3};

pub const GRAVITY: f64 = 0.08; // m/tick/tick
