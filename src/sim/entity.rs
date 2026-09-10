use std::f64::consts::PI;
use std::sync::atomic::{AtomicU8, Ordering};

use super::{GRAVITY, Mth, Rot, Vec3};

/// Which implementation [`update_fall_flying_movement`] routes through.
///
/// `Reference` preserves the mutation-for-mutation port. `Algebraic` uses the collapsed
/// yaw-zero equations when possible and falls back to the reference path for nonzero yaw.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FlightMode {
    #[default]
    Reference,
    Algebraic,
}

static FLIGHT_MODE: AtomicU8 = AtomicU8::new(0);

/// Select the movement implementation. Set this once, before starting parallel physics work.
pub fn set_flight_mode(mode: FlightMode) {
    FLIGHT_MODE.store(mode as u8, Ordering::Relaxed);
}

pub fn flight_mode() -> FlightMode {
    match FLIGHT_MODE.load(Ordering::Relaxed) {
        0 => FlightMode::Reference,
        _ => FlightMode::Algebraic,
    }
}

impl std::str::FromStr for FlightMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "reference" | "ref" => Ok(Self::Reference),
            "algebraic" | "algebra" => Ok(Self::Algebraic),
            _ => Err(format!(
                "unknown flight mode {s:?}, want reference or algebraic"
            )),
        }
    }
}

impl std::fmt::Display for FlightMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Reference => "reference",
            Self::Algebraic => "algebraic",
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct Entity {
    pub pos: Vec3,
    pub vel: Vec3,
    pub rot: Rot,
}

impl Entity {
    pub fn travel(&mut self) {
        self.vel = update_fall_flying_movement(self.vel, self.rot);
        self.mov();
    }

    pub fn mov(&mut self) {
        self.pos += self.vel;
    }
}

#[inline]
pub fn update_fall_flying_movement(vel: Vec3, rot: Rot) -> Vec3 {
    match flight_mode() {
        FlightMode::Algebraic if rot.y == 0.0 => {
            update_fall_flying_movement_yaw_zero(vel, rot.x)
        }
        FlightMode::Reference | FlightMode::Algebraic => {
            update_fall_flying_movement_reference(vel, rot)
        }
    }
}

/// The pitch-only inputs to one yaw-zero flight tick.
///
/// Optimizer candidates repeatedly fly the same schedule tail from different prefix states.
/// None of these values depends on the state, so computing them once per tail pitch avoids
/// recomputing three pitch-dependent trigonometric values on every forward-pass tick.
#[derive(Debug, Clone, Copy)]
pub struct PitchTrig {
    cos_sq: f64,
    sin: f32,
    cos: f32,
}

impl PitchTrig {
    #[inline]
    pub fn new(pitch: f32) -> Self {
        let lean = pitch * (PI / 180.0) as f32;
        Self {
            // Vanilla uses double-precision Math.cos here, independently of Mth trig.
            cos_sq: Mth::square((lean as f64).cos()),
            sin: Mth::sin(lean),
            cos: Mth::cos(lean),
        }
    }
}

/// Fly one yaw-zero tick using pitch terms cached independently of the entity state.
#[inline]
pub fn update_fall_flying_movement_cached(vel: Vec3, pitch: PitchTrig) -> Vec3 {
    match flight_mode() {
        FlightMode::Reference => update_fall_flying_movement_reference_cached(vel, pitch),
        FlightMode::Algebraic => update_fall_flying_movement_yaw_zero_cached(vel, pitch),
    }
}

/// The direct port, retained as the comparison route and for arbitrary yaw.
pub fn update_fall_flying_movement_reference(mut vel: Vec3, rot: Rot) -> Vec3 {
    let look_angle: Vec3 = rot.look_angle();
    let lean_angle: f32 = rot.x * (PI / 180.0) as f32;

    let look_hor_length: f64 = (look_angle.x * look_angle.x + look_angle.z * look_angle.z).sqrt();
    let move_hor_length: f64 = vel.horizontal_distance();
    let gravity: f64 = GRAVITY;
    let lift_force: f64 = Mth::square((lean_angle as f64).cos());
    vel.y += gravity * (-1.0 + lift_force * 0.75);

    // converts v_down to v_forward
    if vel.y < 0.0 && look_hor_length > 0.0 {
        let convert: f64 = vel.y * -0.1 * lift_force;
        vel += Vec3::new(
            look_angle.x * convert / look_hor_length,
            convert,
            look_angle.z * convert / look_hor_length,
        );
    }

    // converts v_forward to v_up if looking up
    if lean_angle < 0.0 && look_hor_length > 0.0 {
        let convert: f64 = move_hor_length * -Mth::sin(lean_angle) as f64 * 0.04;
        vel += Vec3::new(
            -look_angle.x * convert / look_hor_length,
            convert * 3.2,
            -look_angle.z * convert / look_hor_length,
        );
    }

    // turning
    if look_hor_length > 0.0 {
        vel += Vec3::new(
            (look_angle.x / look_hor_length * move_hor_length - vel.x) * 0.1,
            0.0,
            (look_angle.z / look_hor_length * move_hor_length - vel.z) * 0.1,
        );
    }

    vel * Vec3::new(0.99_f32 as f64, 0.98_f32 as f64, 0.99_f32 as f64)
    //vel
}

#[inline]
fn update_fall_flying_movement_reference_cached(mut vel: Vec3, pitch: PitchTrig) -> Vec3 {
    let look_hor_length = (pitch.cos as f64).abs();
    let move_hor_length = vel.horizontal_distance();
    let lift_force = pitch.cos_sq;
    vel.y += GRAVITY * (-1.0 + lift_force * 0.75);

    if vel.y < 0.0 && look_hor_length > 0.0 {
        let convert = vel.y * -0.1 * lift_force;
        vel += Vec3::new(
            0.0,
            convert,
            pitch.cos as f64 * convert / look_hor_length,
        );
    }

    if pitch.sin < 0.0 && look_hor_length > 0.0 {
        let convert = move_hor_length * -pitch.sin as f64 * 0.04;
        vel += Vec3::new(
            0.0,
            convert * 3.2,
            -pitch.cos as f64 * convert / look_hor_length,
        );
    }

    if look_hor_length > 0.0 {
        vel += Vec3::new(
            -vel.x * 0.1,
            0.0,
            (pitch.cos as f64 / look_hor_length * move_hor_length - vel.z) * 0.1,
        );
    }

    vel * Vec3::new(0.99_f32 as f64, 0.98_f32 as f64, 0.99_f32 as f64)
}

/// Algebraically collapsed movement for the yaw-zero plane used by the optimizers.
///
/// This preserves the deliberate split between `Mth` trigonometry for look/pull-up and the
/// double-precision cosine used by vanilla's lift term. The `look_cos` guard also preserves
/// the lookup-table behavior at vertical pitch and the rounded libm direction at the endpoints.
#[inline]
pub fn update_fall_flying_movement_yaw_zero(vel: Vec3, pitch: f32) -> Vec3 {
    let lean = pitch * (PI / 180.0) as f32;
    let look_cos = Mth::cos(lean);
    // The optimizer's state plane has x = 0, where hypot(0, z) is exactly |z| and no square
    // root is needed. Keep the full norm for other yaw-zero callers.
    let move_hor_length = if vel.x == 0.0 {
        vel.z.abs()
    } else {
        vel.horizontal_distance()
    };
    let lift_force = Mth::square((lean as f64).cos());
    let gravity_y = vel.y + GRAVITY * (-1.0 + lift_force * 0.75);

    // At yaw zero, normalizing the horizontal look vector leaves only sign(cos(pitch)) on z.
    // A zero (or NaN) cosine makes the reference implementation skip all three guarded blocks.
    if !(look_cos.abs() > 0.0) {
        return Vec3::new(
            vel.x * 0.99_f32 as f64,
            gravity_y * 0.98_f32 as f64,
            vel.z * 0.99_f32 as f64,
        );
    }
    let look_z = if look_cos.is_sign_negative() { -1.0 } else { 1.0 };

    let dive = if gravity_y < 0.0 {
        gravity_y * -0.1 * lift_force
    } else {
        0.0
    };
    let climb = if lean < 0.0 {
        move_hor_length * -Mth::sin(lean) as f64 * 0.04
    } else {
        0.0
    };
    let along_look = dive - climb;

    Vec3::new(
        0.9 * vel.x * 0.99_f32 as f64,
        (gravity_y + dive + climb * 3.2) * 0.98_f32 as f64,
        (0.9 * vel.z + look_z * (0.9 * along_look + 0.1 * move_hor_length))
            * 0.99_f32 as f64,
    )
}

#[inline]
fn update_fall_flying_movement_yaw_zero_cached(vel: Vec3, pitch: PitchTrig) -> Vec3 {
    let look_cos = pitch.cos;
    let move_hor_length = if vel.x == 0.0 {
        vel.z.abs()
    } else {
        vel.horizontal_distance()
    };
    let lift_force = pitch.cos_sq;
    let gravity_y = vel.y + GRAVITY * (-1.0 + lift_force * 0.75);

    if !(look_cos.abs() > 0.0) {
        return Vec3::new(
            vel.x * 0.99_f32 as f64,
            gravity_y * 0.98_f32 as f64,
            vel.z * 0.99_f32 as f64,
        );
    }
    let look_z = if look_cos.is_sign_negative() { -1.0 } else { 1.0 };

    let dive = if gravity_y < 0.0 {
        gravity_y * -0.1 * lift_force
    } else {
        0.0
    };
    let climb = if pitch.sin < 0.0 {
        move_hor_length * -pitch.sin as f64 * 0.04
    } else {
        0.0
    };
    let along_look = dive - climb;

    Vec3::new(
        0.9 * vel.x * 0.99_f32 as f64,
        (gravity_y + dive + climb * 3.2) * 0.98_f32 as f64,
        (0.9 * vel.z + look_z * (0.9 * along_look + 0.1 * move_hor_length))
            * 0.99_f32 as f64,
    )
}
