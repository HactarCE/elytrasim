use std::sync::atomic::{self, AtomicBool};

use itertools::Itertools;
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

    // /// inserts a duplicate pitch so `ret` has the same length as `pitches`.
    // /// if t < 0.0, inserts a duplicate of the first pitch at the start.
    // /// if t > 0.0, inserts a duplicate of the last pitch at the end.
    // /// if t == 0.0, returns the original pitches.
    // /// TODO: instead of duplicating, do a linear interpolation.
    // pub fn lerp_between(pitches: &[Pitch], t: f32) -> impl Iterator<Item = Pitch> {
    //     assert!((-1.0..=1.0).contains(&t));
    //     pitches
    //         .first()
    //         .into_iter()
    //         .chain(pitches.iter().chain(pitches.iter().last()))
    //         .array_windows::<3>()
    //         .map(move |[left, mid, right]| match t.total_cmp(&0.0) {
    //             std::cmp::Ordering::Less => mid * (1.0 + t) + left * -t,
    //             std::cmp::Ordering::Equal => *mid,
    //             std::cmp::Ordering::Greater => mid * (1.0 - t) + right * t,
    //         })
    // }
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
    pub time_rad: Option<f64>,
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
    time: f64,
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
            time: 0.0,
            init_vel: Vel3::ZERO,
            poses: Vec::new(),
            vels: Vec::new(),
            pitches: Vec::new(),
        };
        ret.resample_time(params.time_rad);
        ret.resample_init_vel_y(params.init_vel_y_std);
        ret.resample_init_vel_z(params.init_vel_z_std);
        ret.resize(params, num_ticks);
        ret
    }

    pub fn zero(num_ticks: usize) -> Self {
        Self {
            time: 0.0,
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
        self.resample_time(params.time_rad);
        self.resample_init_vel_y(params.init_vel_y_std);
        self.resample_init_vel_z(params.init_vel_z_std);
        self.resample_poses_y(params.poses_y_std);
        self.resample_poses_z(params.poses_z_std);
        self.resample_vels_y(params.vels_y_std);
        self.resample_vels_z(params.vels_z_std);
        self.resample_pitches(params.pitches_std.map(|x| x as f64));
    }

    pub fn resample_time(&mut self, time_rad: Option<f64>) {
        match time_rad {
            Some(time_rad) => {
                self.time = rand::rng().random_range(-time_rad..time_rad);
            }
            None => {
                self.time = 0.0;
            }
        }
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
                let distr = rand_distr::Normal::new(0.0, std).unwrap();
                Box::new(rand::rng().sample_iter(distr)) as Box<dyn Iterator<Item = f64>>
            }
            None => Box::new(std::iter::repeat(0.0)) as Box<dyn Iterator<Item = f64>>,
        }
    }
}

pub static UNDO_JITTER_VEL: AtomicBool = AtomicBool::new(true);

pub fn forward(init_vel: Vel3, pitches: &[Pitch], jitter: &Jitter) -> impl Iterator<Item = State> {
    assert_eq!(pitches.len(), jitter.num_ticks());

    let mut pos_accumulator = Vec3::ZERO;
    let mut vel = init_vel + jitter.init_vel;

    (0..jitter.num_ticks()).map(move |tick| {
        //  if t < 0.0, inserts a duplicate of the first pitch at the start.
        //  if t > 0.0, inserts a duplicate of the last pitch at the end.
        //  if t == 0.0, returns the original pitch.
        //  TODO: instead of duplicating, do a linear interpolation.
        let pitch = {
            let left = pitches[tick.saturating_sub(1)];
            let mid = pitches[tick];
            let right = pitches[(tick + 1).min(pitches.len() - 1)];
            let t = jitter.time as f32;
            match t.total_cmp(&0.0) {
                std::cmp::Ordering::Less => mid * (1.0 + t) + left * -t,
                std::cmp::Ordering::Equal => mid,
                std::cmp::Ordering::Greater => mid * (1.0 - t) + right * t,
            }
        };

        let mut state = State {
            pos: jitter.poses[tick],
            vel: vel.elementwise_mul(Vec3::ONE + jitter.vels[tick]),
        }
        .ticked(pitch * (1.0 + jitter.pitches[tick]));

        state.pos -= jitter.poses[tick];
        if UNDO_JITTER_VEL.load(atomic::Ordering::Relaxed) {
            state.vel = state.vel.elementwise_div(Vec3::ONE + jitter.vels[tick]);
        }

        pos_accumulator += state.pos;
        vel = state.vel;

        State {
            pos: pos_accumulator,
            vel,
        }
    })
}

// pub fn forward(init_vel: Vel3, pitches: &[Pitch], jitter: &Jitter) -> impl Iterator<Item = State> {
//     assert_eq!(pitches.len(), jitter.num_ticks());

//     let mut pos_accumulator = Vec3::ZERO;
//     let mut vel = init_vel + jitter.init_vel;

//     (0..jitter.num_ticks()).map(move |tick| {
//         let mut state = State {
//             pos: jitter.poses[tick],
//             vel: vel + jitter.vels[tick],
//         }
//         .ticked(pitches[tick] + jitter.pitches[tick]);

//         state.pos -= jitter.poses[tick];
//         if UNDO_JITTER_VEL.load(atomic::Ordering::Relaxed) {
//             state.vel -= jitter.vels[tick];
//         }

//         pos_accumulator += state.pos;
//         vel = state.vel;

//         State {
//             pos: pos_accumulator,
//             vel,
//         }
//     })
// }

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
        (None, None) => {
            dbg!(cur_pitch);
            unreachable!()
        }
    }) as f32
}

/// &mut pitches bc we want to modify them in place instead of cloning,
/// but we guarantee that they won't be different after return.
pub fn get_grad(
    goodness: impl Fn(State) -> Goodness,
    init_vel: Vel3,
    pitches: &mut [Pitch],
    jitter: &Jitter,
) -> impl Iterator<Item = DGDP> {
    (0..pitches.len()).map(move |i| grad_at_tick(&goodness, init_vel, pitches, jitter, i))
}

pub fn apply_grad(pitches: &mut [Pitch], grads: &[DGDP], learning_rate: f32) {
    assert_eq!(pitches.len(), grads.len());
    for (pitch, grad) in pitches.iter_mut().zip(grads) {
        *pitch += (learning_rate * grad).clamp(-10.0, 10.0);
        *pitch = PitchUtil::clamped(*pitch);
    }
}

pub use deriv_optim::*;
mod deriv_optim {
    use super::*;

    // problem where you need two pitches to move together
    // try picking a random direction in pitch space and doing a gradient step along that direction

    // /// random on unit sphere
    // pub fn rand_pitches_dir(num_ticks: usize) -> Vec<Pitch> {
    //     let mut ret = rand::rng()
    //         .sample_iter(rand_distr::StandardNormal)
    //         .take(num_ticks)
    //         .collect_vec();
    //     let s = ret.iter().map(|x| x * x).sum::<f32>().sqrt();
    //     for x in &mut ret {
    //         *x /= s;
    //     }
    //     ret
    // }

    /// random pure direction
    pub fn rand_pitches_dir(num_ticks: usize) -> Vec<Pitch> {
        let tick = rand::rng().random_range(0..num_ticks);
        let mut ret = vec![0.0; num_ticks];
        ret[tick] = 1.0;
        ret
    }

    pub fn deriv_along_pitches_dir(
        goodness: impl Fn(State) -> Goodness,
        init_vel: Vel3,
        pitches: &mut [Pitch],
        jitter: &Jitter,
        dir: &[Pitch],
    ) -> DGDP {
        const EPSILON: f32 = 0.1;

        let right_pitches = pitches
            .iter()
            .zip(dir)
            .map(|(pitch, dir)| PitchUtil::clamped(*pitch + EPSILON * *dir))
            .collect_vec();
        let right_goodness = goodness(forward(init_vel, &right_pitches, jitter).last().unwrap());

        let left_pitches = pitches
            .iter()
            .zip(dir)
            .map(|(pitch, dir)| PitchUtil::clamped(*pitch - EPSILON * *dir))
            .collect_vec();
        let left_goodness = goodness(forward(init_vel, &left_pitches, jitter).last().unwrap());

        let diff = right_pitches
            .iter()
            .zip(left_pitches.iter())
            .map(|(r, l)| r - l)
            .collect_vec();
        let diff_len = diff.iter().map(|x| x * x).sum::<f32>().sqrt();

        // this probably isn't the most correct way to do this
        ((right_goodness - left_goodness) / diff_len as Goodness) as DGDP
    }

    pub fn deriv_step_along_pitches_dir(
        pitches: &mut [Pitch],
        dir: &[Pitch],
        deriv: DGDP,
        learning_rate: f32,
    ) {
        assert_eq!(pitches.len(), dir.len());
        for (pitch, dir) in pitches.iter_mut().zip(dir) {
            *pitch += (learning_rate * deriv * *dir).clamp(-10.0, 10.0);
            *pitch = PitchUtil::clamped(*pitch);
        }
    }
}

pub use decay::*;
mod decay {
    use super::*;

    // /// decay the pitch deltas
    // pub fn apply_decay(pitches: &mut [Pitch], decay: f32) {
    //     let deltas = pitches
    //         .windows(2)
    //         .map(|window| window[1] - window[0])
    //         .collect::<Vec<_>>();
    //     let decayed = deltas.iter().map(|delta| delta * decay).collect::<Vec<_>>();
    //     for i in 1..pitches.len() {
    //         pitches[i] = PitchUtil::clamped(pitches[i - 1] + decayed[i - 1]);
    //     }
    // }

    /// ret has len `pitches.len() - 1`
    fn delta(pitches: impl Iterator<Item = Pitch>) -> impl Iterator<Item = Pitch> {
        pitches.tuple_windows().map(|(a, b)| b - a)
    }

    fn prefix_sum(first: f32, it: impl Iterator<Item = Pitch>) -> impl Iterator<Item = Pitch> {
        std::iter::once(first).chain(
            it.scan(0.0, |acc, delta| {
                *acc += delta;
                Some(*acc)
            })
            .map(move |val| first + val),
        )
    }

    /// decay the pitch delta deltas
    // TODO: try decaying the distance from the left and right neighbors
    // rather than just the left neighbor.
    // bc we kinda want the pitches to form straight lines.
    pub fn apply_decay(pitches: &mut [Pitch], decay: f32) {
        let deltas = delta(pitches.iter().copied()).collect_vec();
        let delta_deltas = delta(deltas.iter().copied()).collect_vec();
        assert_eq!(delta_deltas.len() + 2, pitches.len());
        let decayed = delta_deltas.iter().map(|delta| delta * decay).collect_vec();
        let new_deltas = prefix_sum(deltas[0], decayed.into_iter());
        let new_pitches = prefix_sum(pitches[0], new_deltas)
            .map(PitchUtil::clamped)
            .collect_vec();
        pitches.copy_from_slice(&new_pitches);
    }
}

// mod tests {
//     use super::*;

//     #[test]
//     fn test_lerp_between_pos() {
//         let pitches = vec![10.0, 20.0, 30.0];
//         let t = 0.1;
//         let oracle = vec![11.0, 21.0, 30.0];
//         let actual = PitchesUtil::lerp_between(&pitches, t).collect_vec();
//         assert_eq!(oracle, actual);
//     }

//     #[test]
//     fn test_lerp_between_neg() {
//         let pitches = vec![10.0, 20.0, 30.0];
//         let t = -0.1;
//         let oracle = vec![10.0, 19.0, 29.0];
//         let actual = PitchesUtil::lerp_between(&pitches, t).collect_vec();
//         assert_eq!(oracle, actual);
//     }
// }
