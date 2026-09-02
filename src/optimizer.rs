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
        if pitch == Self::clamped(pitch) {
            Some(pitch)
        } else {
            None
        }
    }

    fn clamped(pitch: Pitch) -> Pitch {
        // the behavior of .travel() becomes weird at the endpoints
        const EPSILON: Pitch = 1e-4;
        pitch.clamp(-90.0 + EPSILON, 90.0 - EPSILON)
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

    /// blocks of height
    /// 
    /// kilograms * blocks^2 / (ticks^2 * gravity)
    pub fn kinetic_energy(&self) -> Energy {
        self.vel.length_sq() / (2.0 * GRAVITY)
    }

    /// blocks of height
    /// 
    /// kilograms * blocks^2 / (ticks^2 * gravity)
    pub fn potential_energy(&self) -> Energy {
        self.pos.y
    }

    /// blocks of height
    /// 
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
    pub time_rad: f64,
    pub init_vel_y_std: f64,
    pub init_vel_z_std: f64,
    pub vels_y_std: f64,
    pub vels_z_std: f64,
    // this really should be a `Pitch` ie `f32`,
    // but that's annoying to deal with.
    pub pitches_std: f64,
}

#[derive(Debug, Clone)]
pub struct Jitter {
    pub time: f64,
    init_vel: Vel3,
    vels: Vec<Vel3>,
    pitches: Vec<Pitch>,
}

impl Jitter {
    pub fn num_ticks(&self) -> usize {
        let ret = self.vels.len();
        assert_eq!(ret, self.pitches.len());
        ret
    }

    pub fn new(params: &JitterParams, num_ticks: usize) -> Self {
        let mut ret = Self {
            time: 0.0,
            init_vel: Vel3::ZERO,
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
            vels: vec![Vec3::ZERO; num_ticks],
            pitches: vec![0.0; num_ticks],
        }
    }

    pub fn resize(&mut self, params: &JitterParams, num_ticks: usize) {
        let old_ticks = self.num_ticks();

        self.vels.resize(num_ticks, Vec3::ZERO);
        self.pitches.resize(num_ticks, 0.0);

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
            .zip(Self::sample(params.pitches_std))
        {
            *old = new as Pitch;
        }
    }

    pub fn resample_all(&mut self, params: &JitterParams) {
        self.resample_time(params.time_rad);
        self.resample_init_vel_y(params.init_vel_y_std);
        self.resample_init_vel_z(params.init_vel_z_std);
        self.resample_vels_y(params.vels_y_std);
        self.resample_vels_z(params.vels_z_std);
        self.resample_pitches(params.pitches_std);
    }

    pub fn resample_time(&mut self, time_rad: f64) {
        assert!(time_rad >= 0.0);
        if time_rad > 0.0 {
            self.time = rand::rng().random_range(-time_rad..time_rad);
            // self.time = self.time.abs();
        } else {
            self.time = 0.0;
        }
    }

    pub fn resample_init_vel_y(&mut self, std: f64) {
        self.init_vel.y = Self::sample(std).next().unwrap();
    }

    pub fn resample_init_vel_z(&mut self, std: f64) {
        self.init_vel.z = Self::sample(std).next().unwrap();
    }

    pub fn resample_vels_y(&mut self, std: f64) {
        for (old, new) in self.vels.iter_mut().zip(Self::sample(std)) {
            old.y = new;
        }
    }

    pub fn resample_vels_z(&mut self, std: f64) {
        for (old, new) in self.vels.iter_mut().zip(Self::sample(std)) {
            old.z = new;
        }
    }

    pub fn resample_pitches(&mut self, std: f64) {
        for (old, new) in self.pitches.iter_mut().zip(Self::sample(std)) {
            *old = new as Pitch;
        }
    }

    fn sample(std: f64) -> impl Iterator<Item = f64> {
        assert!(std >= 0.0);
        if std > 0.0 {
            let distr = rand_distr::Normal::new(0.0, std).unwrap();
            Box::new(rand::rng().sample_iter(distr)) as Box<dyn Iterator<Item = f64>>
        } else {
            Box::new(std::iter::repeat(0.0)) as Box<dyn Iterator<Item = f64>>
        }
    }
}

pub static UNDO_JITTER_VEL: AtomicBool = AtomicBool::new(true);

pub fn forward(
    init_vel: Vel3,
    pitches: &[Pitch],
    jitter: &Jitter,
) -> impl Iterator<Item = (Pitch, State)> {
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

            let mid_left = mid * (1.0 + t) + (left * -t);
            let mid_right = mid * (1.0 - t) + (right * t);

            match t.total_cmp(&0.0) {
                std::cmp::Ordering::Less => {
                    assert!((left.min(mid) - 1e-5..=left.max(mid) + 1e-5).contains(&mid_left));
                    mid_left
                }
                std::cmp::Ordering::Equal => {
                    assert!((mid - mid_left).abs() < 1e-5);
                    assert!((mid - mid_right).abs() < 1e-5);
                    mid
                }
                std::cmp::Ordering::Greater => {
                    assert!((mid.min(right) - 1e-5..=mid.max(right) + 1e-5).contains(&mid_right));
                    mid_right
                }
            }
        } + jitter.pitches[tick];

        let mut state = State {
            pos: Vec3::ZERO,
            vel: vel.elementwise_mul(Vec3::ONE + jitter.vels[tick]),
        }
        .ticked(pitch);

        if UNDO_JITTER_VEL.load(atomic::Ordering::Relaxed) {
            state.vel = state.vel.elementwise_div(Vec3::ONE + jitter.vels[tick]);
        }

        pos_accumulator += state.pos;
        vel = state.vel;

        (
            pitch,
            State {
                pos: pos_accumulator,
                vel,
            },
        )
    })
}

pub fn forward_last(init_vel: Vel3, pitches: &[Pitch], jitter: &Jitter) -> State {
    forward(init_vel, pitches, jitter).last().unwrap().1
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
        let after = forward_last(init_vel, pitches, jitter);
        pitches[tick] = cur_pitch;
        goodness(after)
    });

    let left_goodness = PitchUtil::try_from(cur_pitch - EPSILON).map(|left_pitch| {
        pitches[tick] = left_pitch;
        let after = forward_last(init_vel, pitches, jitter);
        pitches[tick] = cur_pitch;
        goodness(after)
    });

    let cur_goodness = || goodness(forward_last(init_vel, pitches, jitter));

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

pub use decay::*;
mod decay {
    use super::*;

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

pub use myopic::*;
mod myopic {
    use super::*;

    fn gamma(v: Vel3) -> f64 {
        (-v.y).atan2(v.z).to_degrees()
    }
    fn ticked(s: &State, p: Pitch) -> State {
        s.ticked(p)
    }
    fn run_n(s: &State, p: Pitch, n: usize) -> State {
        let mut s = s.clone();
        for _ in 0..n {
            s = s.ticked(p)
        }
        s
    }

    fn argmax<F: Fn(Pitch) -> f64>(f: F, step: Pitch) -> Pitch {
        let (mut best_pitch, mut best_value) = (0.0, f64::NEG_INFINITY);
        let n = (180.0 / step).round() as i64;
        for i in 0..=n {
            let pitch = PitchUtil::clamped(-90.0 + step * i as Pitch);
            let value = f(pitch);
            if value > best_value {
                best_value = value;
                best_pitch = pitch
            }
        }
        let (mut a, mut b) = (
            (best_pitch - step).max(-90.0),
            (best_pitch + step).min(90.0),
        );
        for _ in 0..60 {
            let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
            if f(m1) < f(m2) { a = m1 } else { b = m2 }
        }
        let p = 0.5 * (a + b);
        if f(p) > best_value { p } else { best_pitch }
    }

    /// DIVE. Pitch whose next tick leaves the flight-path angle at `target`.
    ///
    /// gamma(v') is monotone in pitch at dive speeds, but not at low speed, where two branches
    /// reach a given angle and only the nose-down one accelerates. So: scan for the last upward
    /// crossing, then bisect. Picking the wrong branch stalls the dive completely.
    pub fn bug_gamma_to(s: &State, target: f64) -> Pitch {
        let h = |p: Pitch| gamma(ticked(s, p).vel) - target;
        let (mut lo, mut hi) = (Pitch::NAN, Pitch::NAN);
        let (mut prev, mut pp) = (h(-90.0), -90.0);
        for i in 1..=1440 {
            let p = -90.0 + 0.125 * i as Pitch;
            let c = h(p);
            if prev <= 0.0 && c > 0.0 {
                lo = pp;
                hi = p
            }
            prev = c;
            pp = p;
        }
        if lo.is_nan() {
            return if h(90.0) < 0.0 { 90.0 } else { -90.0 };
        }
        for _ in 0..60 {
            let m = 0.5 * (lo + hi);
            if h(m) <= 0.0 { lo = m } else { hi = m }
        }
        0.5 * (lo + hi)
    }

    pub fn bug_dive(s: &State, g_star: f64, k: f64) -> Pitch {
        let g0 = gamma(s.vel);
        bug_gamma_to(s, g0 + k * (g_star - g0))
    }

    /// the pitch st the next angle of travel is `target`.
    fn pitch_for_flight_angle(s: &State, target: f64) -> Pitch {
        let fa = gamma(s.vel);
        argmax(|pitch| -(gamma(ticked(s, pitch).vel) - target).abs(), 1.0)
    }

    /// the pitch st the next angle of travel is preserved.
    /// note that this is multimodal at the beginning,
    /// and only becomes unimodal after a few seconds into the dive.
    // pitch_for_preserved_flight_angle
    pub fn preserve_vel_angle(s: &State) -> Pitch {
        let fa = gamma(s.vel);
        pitch_for_flight_angle(s, fa)
    }

    /// the pitch st the (delta) total energy is maximized,
    /// if you were to hold that pitch for num_ticks.
    ///
    /// empirically `num_ticks = 20` is good.
    // pitch_te_gain
    pub fn argmax_dte_n(s: &State, num_ticks: usize) -> Pitch {
        let te = s.total_energy();
        argmax(
            |pitch| run_n(s, pitch, num_ticks).total_energy() - te,
            5.0,
        )
    }
}
