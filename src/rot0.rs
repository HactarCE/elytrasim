use crate::sim::*;

pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    update_fall_flying_movement(vel, rot)
}
