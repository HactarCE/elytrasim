use std::sync::atomic::{self, AtomicBool};

use rand::prelude::*;

use crate::sim::*;

pub type Goodness = f64;
pub type Energy = f64;
pub type Pos3 = Vec3;
pub type Vel3 = Vec3;
pub type Pitch = f32;
pub type DGDP = f32;

pub struct PitchUtil;

impl PitchUtil {
    fn try_from(pitch: f32) -> Option<Pitch> {
        if (-90.0..=90.0).contains(&pitch) {
            Some(pitch)
        } else {
            None
        }
    }

    fn clamped(pitch: Pitch) -> Pitch {
        pitch.clamp(-90.0, 90.0)
    }
}

pub struct PitchesUtil;

impl PitchesUtil {
    /// look at `pitch` for all ticks.
    pub fn new_constant(ticks: usize, pitch: Pitch) -> Vec<Pitch> {
        vec![pitch; ticks]
    }

    /// lerp from start to end over the ticks.
    pub fn new_lerp(ticks: usize, start: Pitch, end: Pitch) -> Vec<Pitch> {
        (0..ticks)
            .map(|i| {
                let t = i as f32 / (ticks - 1) as f32;
                start * (1.0 - t) + end * t
            })
            .collect()
    }

    /// +40 then -40.
    pub fn new_4040(ticks: usize, cut: f64) -> Vec<Pitch> {
        let mid = (ticks as f64 * cut) as usize;

        (0..mid)
            .map(|_| 40.0)
            .chain((mid..ticks).map(|_| -40.0))
            .collect()
    }

    /// +40 then 0 then -40.
    pub fn new_40zero40(ticks: usize, left_cut: f64, right_cut: f64) -> Vec<Pitch> {
        assert!(left_cut < right_cut);
        let left = (ticks as f64 * left_cut) as usize;
        let right = (ticks as f64 * right_cut) as usize;

        (0..left)
            .map(|_| 40.0)
            .chain((left..right).map(|_| 0.0))
            .chain((right..ticks).map(|_| -40.0))
            .collect()
    }

    pub fn new_rand_uniform(ticks: usize) -> Vec<Pitch> {
        let mut rng = rand::rng();
        (0..ticks).map(|_| rng.random_range(-90.0..=90.0)).collect()
    }

    pub fn new_rand_walk(ticks: usize, step: f32) -> Vec<Pitch> {
        let mut rng = rand::rng();
        let mut pitch = rng.random_range(-90.0..=90.0);
        (0..ticks)
            .map(|_| {
                pitch += rng.random_range(-step..=step);
                pitch = PitchUtil::clamped(pitch);
                pitch
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct State {
    pub pos: Pos3,
    pub vel: Vel3,
}

impl State {
    const ZERO: Self = Self {
        pos: Pos3::ZERO,
        vel: Vel3::ZERO,
    };

    fn ticked(&self, pitch: Pitch) -> Self {
        let rot = Rot { x: pitch, y: 0.0 };
        let mut entity = Entity {
            pos: self.pos,
            vel: self.vel,
            rot,
        };
        entity.travel();
        assert_eq!(entity.rot, rot);
        Self {
            pos: entity.pos,
            vel: entity.vel,
        }
    }

    /// kilograms * blocks^2 / ticks^2
    pub fn kinetic_energy(&self) -> Energy {
        self.vel.length_sq() * 0.5
    }

    /// kilograms * blocks^2 / ticks^2
    pub fn potential_energy(&self) -> Energy {
        self.pos.y * GRAVITY
    }

    /// kilograms * blocks^2 / ticks^2
    pub fn total_energy(&self) -> Energy {
        self.kinetic_energy() + self.potential_energy()
    }
}

/// `None` means disabled ie 0.0,
/// not "don't update".
// TODO: these probably shouldn't be options
#[derive(Debug, Clone)]
pub struct JitterParams {
    pub init_vel_y_std: Option<f64>,
    pub init_vel_z_std: Option<f64>,
    pub poses_y_std: Option<f64>,
    pub poses_z_std: Option<f64>,
    pub vels_y_std: Option<f64>,
    pub vels_z_std: Option<f64>,
    // this really should be a `Pitch` ie `f32`,
    // but that's annoying to deal with.
    pub pitches_std: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Jitter {
    init_vel: Vel3,
    poses: Vec<Vec3>,
    vels: Vec<Vel3>,
    pitches: Vec<Pitch>,
}

impl Jitter {
    pub fn num_ticks(&self) -> usize {
        let ret = self.poses.len();
        assert_eq!(ret, self.vels.len());
        assert_eq!(ret, self.pitches.len());
        ret
    }

    pub fn new(params: &JitterParams, num_ticks: usize) -> Self {
        let mut ret = Self {
            init_vel: Vel3::ZERO,
            poses: Vec::new(),
            vels: Vec::new(),
            pitches: Vec::new(),
        };
        ret.resample_init_vel_y(params.init_vel_y_std);
        ret.resample_init_vel_z(params.init_vel_z_std);
        ret.resize(params, num_ticks);
        ret
    }

    pub fn zero(num_ticks: usize) -> Self {
        Self {
            init_vel: Vel3::ZERO,
            poses: vec![Vec3::ZERO; num_ticks],
            vels: vec![Vec3::ZERO; num_ticks],
            pitches: vec![0.0; num_ticks],
        }
    }

    pub fn resize(&mut self, params: &JitterParams, num_ticks: usize) {
        let old_ticks = self.num_ticks();

        self.poses.resize(num_ticks, Vec3::ZERO);
        self.vels.resize(num_ticks, Vec3::ZERO);
        self.pitches.resize(num_ticks, 0.0);

        for (old, (new_y, new_z)) in self
            .poses
            .iter_mut()
            .skip(old_ticks)
            .zip(Self::sample(params.poses_y_std).zip(Self::sample(params.poses_z_std)))
        {
            old.y = new_y;
            old.z = new_z;
        }

        for (old, (new_y, new_z)) in self
            .vels
            .iter_mut()
            .skip(old_ticks)
            .zip(Self::sample(params.vels_y_std).zip(Self::sample(params.vels_z_std)))
        {
            old.y = new_y;
            old.z = new_z;
        }

        for (old, new) in self
            .pitches
            .iter_mut()
            .skip(old_ticks)
            .zip(Self::sample(params.pitches_std.map(|x| x as f64)))
        {
            *old = new as Pitch;
        }
    }

    pub fn resample_all(&mut self, params: &JitterParams) {
        self.resample_init_vel_y(params.init_vel_y_std);
        self.resample_init_vel_z(params.init_vel_z_std);
        self.resample_poses_y(params.poses_y_std);
        self.resample_poses_z(params.poses_z_std);
        self.resample_vels_y(params.vels_y_std);
        self.resample_vels_z(params.vels_z_std);
        self.resample_pitches(params.pitches_std.map(|x| x as f64));
    }

    pub fn resample_init_vel_y(&mut self, std: Option<f64>) {
        self.init_vel.y = Self::sample(std).next().unwrap();
    }

    pub fn resample_init_vel_z(&mut self, std: Option<f64>) {
        self.init_vel.z = Self::sample(std).next().unwrap();
    }

    pub fn resample_poses_y(&mut self, std: Option<f64>) {
        for (old, new) in self.poses.iter_mut().zip(Self::sample(std)) {
            old.y = new;
        }
    }

    pub fn resample_poses_z(&mut self, std: Option<f64>) {
        for (old, new) in self.poses.iter_mut().zip(Self::sample(std)) {
            old.z = new;
        }
    }

    pub fn resample_vels_y(&mut self, std: Option<f64>) {
        for (old, new) in self.vels.iter_mut().zip(Self::sample(std)) {
            old.y = new;
        }
    }

    pub fn resample_vels_z(&mut self, std: Option<f64>) {
        for (old, new) in self.vels.iter_mut().zip(Self::sample(std)) {
            old.z = new;
        }
    }

    pub fn resample_pitches(&mut self, std: Option<f64>) {
        for (old, new) in self.pitches.iter_mut().zip(Self::sample(std)) {
            *old = new as Pitch;
        }
    }

    fn sample(std: Option<f64>) -> impl Iterator<Item = f64> {
        match std {
            Some(std) => {
                let dist = rand_distr::Normal::new(0.0, std).unwrap();
                Box::new(rand::rng().sample_iter(dist)) as Box<dyn Iterator<Item = f64>>
            }
            None => Box::new(std::iter::repeat(0.0)) as Box<dyn Iterator<Item = f64>>,
        }
    }
}

// /// the state at each tick *after* applying the pitches.
// /// so `init_vel` isn't `ret[0].vel`.
// /// we have `ret.len() == self.0.len()`.
// pub fn cycle(init_vel: Vel3, pitches: &[Pitch]) -> impl Iterator<Item = State> {
//     let mut cur = State {
//         pos: Pos3::ZERO,
//         vel: init_vel,
//     };
//     pitches.iter().copied().map(move |pitch| {
//         cur = cur.ticked(pitch);
//         cur.clone()
//     })
// }

pub static UNDO_JITTER_VEL: AtomicBool = AtomicBool::new(true);

pub fn forward(init_vel: Vel3, pitches: &[Pitch], jitter: &Jitter) -> impl Iterator<Item = State> {
    assert_eq!(pitches.len(), jitter.num_ticks());

    let mut pos_accumulator = Vec3::ZERO;
    let mut vel = init_vel + jitter.init_vel;

    (0..jitter.num_ticks()).map(move |tick| {
        let mut state = State {
            pos: jitter.poses[tick],
            vel: vel + jitter.vels[tick],
        }
        .ticked(pitches[tick] + jitter.pitches[tick]);

        state.pos -= jitter.poses[tick];
        if UNDO_JITTER_VEL.load(atomic::Ordering::Relaxed) {
            state.vel -= jitter.vels[tick];
        }

        pos_accumulator += state.pos;
        vel = state.vel;

        State {
            pos: pos_accumulator,
            vel,
        }
    })
}

/// the gradient of goodness with respect to the pitch at the given tick.
///
/// &mut pitches bc we want to modify them in place instead of cloning,
/// but we guarantee that they won't be different after return.
pub fn grad_at_tick(
    goodness: impl Fn(State) -> Goodness,
    init_vel: Vel3,
    pitches: &mut [Pitch],
    jitter: &Jitter,
    tick: usize,
) -> DGDP {
    const EPSILON: Pitch = 0.1;

    let cur_pitch = pitches[tick];

    let right_goodness = PitchUtil::try_from(cur_pitch + EPSILON).map(|right_pitch| {
        pitches[tick] = right_pitch;
        let after = forward(init_vel, pitches, jitter).last().unwrap();
        pitches[tick] = cur_pitch;
        goodness(after)
    });

    let left_goodness = PitchUtil::try_from(cur_pitch - EPSILON).map(|left_pitch| {
        pitches[tick] = left_pitch;
        let after = forward(init_vel, pitches, jitter).last().unwrap();
        pitches[tick] = cur_pitch;
        goodness(after)
    });

    let cur_goodness = || goodness(forward(init_vel, pitches, jitter).last().unwrap());

    (match (left_goodness, right_goodness) {
        // central difference if we can
        (Some(left_goodness), Some(right_goodness)) => {
            (right_goodness - left_goodness) / (2.0 * EPSILON) as Goodness
        }
        (None, Some(right_goodness)) => (right_goodness - cur_goodness()) / EPSILON as Goodness,
        (Some(left_goodness), None) => (cur_goodness() - left_goodness) / EPSILON as Goodness,
        (None, None) => unreachable!(),
    }) as f32
}

/// &mut pitches bc we want to modify them in place instead of cloning,
/// but we guarantee that they won't be different after return.
pub fn grad(
    goodness: impl Fn(State) -> Goodness,
    init_vel: Vel3,
    pitches: &mut [Pitch],
    jitter: &Jitter,
) -> impl Iterator<Item = DGDP> {
    (0..pitches.len()).map(move |i| grad_at_tick(&goodness, init_vel, pitches, jitter, i))
}

pub fn gradient_descent_step(
    goodness: impl Fn(State) -> Goodness,
    init_vel: Vel3,
    pitches: &mut [Pitch],
    jitter: &Jitter,
    learning_rate: f32,
) {
    let grads: Vec<DGDP> = grad(&goodness, init_vel, pitches, jitter).collect();
    for (pitch, grad) in pitches.iter_mut().zip(grads) {
        *pitch -= learning_rate * grad;
        *pitch = PitchUtil::clamped(*pitch);
    }
}
