use super::*;

// pub type Energy = f64;

pub type KineticEnergy = f64;
pub type PotentialEnergy = f64;
pub type TotalEnergy = f64;

pub type DeltaKineticEnergy = f64;
pub type DeltaPotentialEnergy = f64;
pub type DeltaTotalEnergy = f64;

pub type DeltaState = State;

#[derive(Debug, Default, Clone,  PartialEq)]
pub struct State {
    pub pos: Vec3,
    pub vel: Vec3,
}
impl State {
    pub fn ticked(&self, rot: Rot) -> Self {
        let mut entity = Entity {
            pos: self.pos,
            vel: self.vel,
            rot,
        };
        entity.travel();
        entity.into()
    }

    pub fn sub(&self, other: &Self) -> DeltaState {
        Self {
            pos: self.pos - other.pos,
            vel: self.vel - other.vel,
        }
    }

    // pub fn delta_for_pitch(vel: Vel3, pitch: f32) -> DeltaState {
    //     let state = State {
    //         pos: Vec3::ZERO,
    //         vel,
    //     };
    //     state.ticked(Rot { x: pitch, y: 0. }).sub(&state)
    // }

    /// kilograms * blocks^2 / ticks^2
    pub fn kinetic_energy(&self) -> KineticEnergy {
        self.vel.length_sq() * 0.5
    }

    /// kilograms * blocks^2 / ticks^2
    pub fn potential_energy(&self) -> PotentialEnergy {
        self.pos.y * GRAVITY
    }

    /// kilograms * blocks^2 / ticks^2
    pub fn total_energy(&self) -> TotalEnergy {
        self.kinetic_energy() + self.potential_energy()
    }
}
impl From<Entity> for State {
    fn from(entity: Entity) -> Self {
        Self {
            pos: entity.pos,
            vel: entity.vel,
        }
    }
}
