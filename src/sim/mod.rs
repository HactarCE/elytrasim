mod entity;
mod mth;
mod rot;
mod state;
mod vec3;

pub use entity::{
    Entity, FlightMode, flight_mode, set_flight_mode, update_fall_flying_movement,
    update_fall_flying_movement_reference, update_fall_flying_movement_yaw_zero,
};
pub use mth::{Mth, TrigMode, set_trig_mode, sin_table, trig_mode};
pub use rot::{Pitch, Rot, Yaw};
pub use state::{
    DeltaKineticEnergy, DeltaPotentialEnergy, DeltaTotalEnergy, KineticEnergy, PotentialEnergy,
    State, TotalEnergy,
};
pub use vec3::{Acc, Acc3, Pos, Pos3, Vec3, Vel, Vel3};

pub const GRAVITY: f64 = 0.08; // m/tick/tick
