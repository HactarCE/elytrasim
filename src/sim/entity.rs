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

/// One yaw-zero tick and its partial derivatives, for reverse-mode gradients (`crate::adjoint`).
///
/// The returned velocity is exactly `update_fall_flying_movement_cached`'s, so a replay built
/// from it is the real one. The partials are `[[dvy'/dvy, dvy'/dvz, dvy'/dp], [dvz'/dvy,
/// dvz'/dvz, dvz'/dp]]`, `p` in degrees, and they differentiate the kernel as if its trig were
/// smooth: `d sin/dp = cos * pi/180` and so on, evaluated at the pitch's `f32` lean. Under
/// `mth_lut` the real `sin` and `cos` are 65536-step staircases whose derivative is zero almost
/// everywhere, so this is the derivative of the physics the table approximates, not of the table.
///
/// Branches are not smoothed. The down-to-forward conversion counts only when it fired
/// (`v_y` after gravity `< 0`) and the forward-to-up one only when it fired (`sin(pitch) < 0`,
/// which under `mth_lut` starts at about -0.0055 degrees, not at 0): each partial is the
/// derivative of the branch actually taken. At a switch the true derivative jumps, and this
/// reports the side the replay is on.
///
/// Assumes the yaw-zero plane (`vel.x == 0`), where every replay in this crate lives.
pub fn fall_flying_partials(vel: Vec3, pitch: f64) -> (Vec3, [[f64; 3]; 2]) {
    debug_assert!(vel.x == 0.0, "fall_flying_partials assumes the yaw-zero plane");
    let pt = PitchTrig::new(pitch as f32);
    let next = update_fall_flying_movement_cached(vel, pt);
    let (dy, dz) = (0.98_f32 as f64, 0.99_f32 as f64);
    let lift = pt.cos_sq;
    let gy = vel.y + GRAVITY * (-1.0 + lift * 0.75);
    if !(pt.cos.abs() > 0.0) {
        // The dead tick at a vertical look: gravity (with its lift term) and drag, nothing else.
        let lean = (pitch as f32 * (PI / 180.0) as f32) as f64;
        let dl = -(2.0 * lean).sin() * PI / 180.0;
        return (next, [[dy, 0.0, dy * 0.75 * GRAVITY * dl], [0.0, dz, 0.0]]);
    }
    let look_z = if pt.cos.is_sign_negative() { -1.0 } else { 1.0 };
    let lean = (pitch as f32 * (PI / 180.0) as f32) as f64;
    let k = PI / 180.0;
    let dl = -(2.0 * lean).sin() * k;          // d cos^2 / dp
    let ds = lean.cos() * k;                    // d sin / dp
    let m = vel.z.abs();                        // move_hor_length, with vel.x == 0
    let dm = if vel.z >= 0.0 { 1.0 } else { -1.0 };
    // Rows are d/d(vy, vz, p).
    let dgy = [1.0, 0.0, 0.75 * GRAVITY * dl];
    let (mut vy1, mut vz1) = (dgy, [0.0, 1.0, 0.0]);
    if gy < 0.0 {
        // conv = -0.1 * lift * gy; vy += conv; vz += look_z * conv
        let dconv = [-0.1 * lift, 0.0, -0.1 * (dl * gy + lift * dgy[2])];
        for i in 0..3 { vy1[i] += dconv[i]; vz1[i] += look_z * dconv[i] }
    }
    if pt.sin < 0.0 {
        // cu = m * -sin * 0.04; vy += 3.2 cu; vz -= look_z * cu
        let s = pt.sin as f64;
        let dcu = [0.0, -s * 0.04 * dm, -0.04 * m * ds];
        for i in 0..3 { vy1[i] += 3.2 * dcu[i]; vz1[i] -= look_z * dcu[i] }
    }
    // Turning: vz = 0.9 vz + 0.1 look_z m.
    let turn = [0.0, 0.1 * look_z * dm, 0.0];
    let rows = [std::array::from_fn(|i| dy * vy1[i]), std::array::from_fn(|i| dz * (0.9 * vz1[i] + turn[i]))];
    (next, rows)
}

/// The forward-to-up branch's `d(v_y', v_z')/dp` at a tick where it is off only because the
/// pitch sits on its switch: `pitch <= 0` but `sin` is not `< 0` (exactly 0 under `libm`; within
/// one table cell, about -0.0055 degrees, under `mth_lut`). `None` anywhere else.
///
/// Why. There the objective has a corner in this pitch, and `fall_flying_partials` reports the
/// branch the replay took, which is the *off* side: the derivative for raising the pitch. The
/// derivative for lowering it is this plus that. Neither is smoothed; a caller that wants the
/// one-sided derivative in each direction takes both.
pub fn climb_switch_partials(vel: Vec3, pitch: f64) -> Option<[f64; 2]> {
    let pt = PitchTrig::new(pitch as f32);
    if !(pitch <= 0.0) || pt.sin < 0.0 || !(pt.cos.abs() > 0.0) { return None }
    let look_z = if pt.cos.is_sign_negative() { -1.0 } else { 1.0 };
    let lean = (pitch as f32 * (PI / 180.0) as f32) as f64;
    // cu = m * -sin * 0.04 contributes dcu/dp = -0.04 m cos * pi/180 at the switch.
    let dcu = -0.04 * vel.z.abs() * lean.cos() * PI / 180.0;
    Some([0.98_f32 as f64 * 3.2 * dcu, 0.99_f32 as f64 * 0.9 * -look_z * dcu])
}
