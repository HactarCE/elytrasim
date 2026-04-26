use crate::sim::*;

type Goodness = f64;
pub type Pos3 = Vec3;
pub type Vel3 = Vec3;
pub type Energy = f64;
pub type Pitch = f32;
pub type DeltaPitch = f32;
pub type DeltaDeltaPitch = f32;

fn approx_eq_f32(a: f32, b: f32) -> bool {
    assert!(a.is_finite());
    assert!(b.is_finite());
    (a - b).abs() < 0.001
}

fn approx_eq_f64(a: f64, b: f64) -> bool {
    assert!(a.is_finite());
    assert!(b.is_finite());
    (a - b).abs() < 0.001
}

fn clamp_pitch(pitch: &mut Pitch) {
    *pitch = pitch.clamp(-90.0, 90.0);
}

fn clamped_pitch(pitch: Pitch) -> Pitch {
    debug_assert!(pitch.is_finite());
    pitch.clamp(-90.0, 90.0)
}

// ret.len() == it.len()
fn cyclic_forward_difference_f32(mut it: impl Iterator<Item = f32>) -> impl Iterator<Item = f32> {
    let first = it
        .next()
        .expect("cyclic_forward_difference requires at least one element");
    let mut prev = first;
    it.chain(std::iter::once(first)).map(move |cur| {
        let delta = cur - prev;
        prev = cur;
        delta
    })
}

// #[derive(Debug, Clone)]
// pub enum OptimizationStrategy {
//     FixedDelta { delta: DeltaPitch },
//     GradientDescent { learning_rate: f64 },
// }
// impl Default for OptimizationStrategy {
//     fn default() -> Self {
//         Self::GradientDescent {
//             learning_rate: 500.0,
//         }
//     }
// }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationStrategy {
    GradientDescent,
    FixedDelta,
}

#[derive(Debug, Default, Clone)]
pub struct State {
    pub pos: Pos3,
    pub vel: Vel3,
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

    /// vaguely normalized.
    fn state_goodness(&self) -> Goodness {
        // real optimization targets
        // self.pos.y / 20.0
        self.total_energy() / 2.0

        // mental illnesses
        // self.vel.y
        // self.pos.y / self.pos.z
        // z vel is only interesting for not steady state
        // self.vel.z
        // self.pos.z
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

#[derive(Debug, Clone)]
pub struct Pitches(pub Vec<f32>);
impl Pitches {
    // /// look at 0 for all ticks.
    // pub fn new_0(ticks: usize) -> Self {
    //     Self(vec![0.0; ticks])
    // }

    /// look at `pitch` for all ticks.
    pub fn new_uniform(ticks: usize, pitch: Pitch) -> Self {
        Self(vec![pitch; ticks])
    }

    /// lerp from start to end over the ticks.
    pub fn new_lerp(ticks: usize, start: Pitch, end: Pitch) -> Self {
        Self(
            (0..ticks)
                .map(|i| {
                    let t = i as f32 / (ticks - 1) as f32;
                    start * (1.0 - t) + end * t
                })
                .collect(),
        )
    }

    /// +40 then -40.
    pub fn new_4040(ticks: usize, cut: f64) -> Self {
        let mid = (ticks as f64 * cut) as usize;
        Self(
            (0..mid)
                .map(|_| 40.0)
                .chain((mid..ticks).map(|_| -40.0))
                .collect(),
        )
    }

    /// +40 then 0 then -40.
    pub fn new_40zero40(ticks: usize, left_cut: f64, right_cut: f64) -> Self {
        assert!(left_cut < right_cut);
        let left = (ticks as f64 * left_cut) as usize;
        let right = (ticks as f64 * right_cut) as usize;
        Self(
            (0..left)
                .map(|_| 40.0)
                .chain((left..right).map(|_| 0.0))
                .chain((right..ticks).map(|_| -40.0))
                .collect(),
        )
    }

    /// the state at each tick *after* applying the pitches.
    /// so `init_vel` isn't `ret[0].vel`.
    /// we have `ret.len() == self.0.len()`.
    pub fn cycle(&self, init_vel: Vel3) -> impl Iterator<Item = State> {
        // let mut states = Vec::with_capacity(self.0.len());
        let mut cur = State {
            pos: Pos3::ZERO,
            vel: init_vel,
        };
        self.0.iter().map(move |pitch| {
            let rot = Rot { x: *pitch, y: 0.0 };
            cur = cur.ticked(rot);
            cur.clone()
        })
        // for pitch in self.0.iter() {
        //     let rot = Rot { x: *pitch, y: 0.0 };
        //     cur = cur.ticked(rot);
        //     states.push(cur.clone());
        // }
        // states
    }

    /// given this init velocity, return the state after applying the pitches.
    // /// `None` if we're empty.
    pub fn after_cycle(&self, init_vel: Vel3) -> State {
        self.cycle(init_vel).last().expect("`Pitches` is empty")
    }

    /// init vel is a guess at the stead state velocity.
    pub fn steady_vel_guessed(&self, steady_vel_guess: Vel3) -> Vel3 {
        let mut state = self.after_cycle(steady_vel_guess);
        loop {
            let next = self.after_cycle(state.vel);
            if approx_eq_f64(state.vel.x, next.vel.x)
                && approx_eq_f64(state.vel.y, next.vel.y)
                && approx_eq_f64(state.vel.z, next.vel.z)
            {
                break;
            }
            state = next;
        }
        state.vel
    }

    /// clamp each pitch.
    fn clamp(&mut self) {
        for pitch in self.0.iter_mut() {
            clamp_pitch(pitch);
        }
    }

    /// ret.len() == self.0.len() - 1.
    pub fn cyclic_pitch_deltas(&self) -> impl Iterator<Item = DeltaPitch> {
        // self.0.iter().zip(self.0.iter().skip(1)).map(|(a, b)| b - a)
        // assert_eq!(
        //     cyclic_forward_difference_f32(self.0.iter().cloned()).count(),
        //     self.0.len()
        // );
        cyclic_forward_difference_f32(self.0.iter().cloned())
    }

    /// ret.len() == self.0.len() - 2.
    pub fn cyclic_pitch_deltas_deltas(&self) -> impl Iterator<Item = DeltaDeltaPitch> {
        cyclic_forward_difference_f32(self.cyclic_pitch_deltas())
    }

    fn cyclic_pitch_deltas_abs_average(&self) -> DeltaPitch {
        self.cyclic_pitch_deltas().fold(0.0, |a, b| a + b.abs()) / self.0.len() as f32
    }

    fn cyclic_pitch_deltas_deltas_abs_average(&self) -> DeltaDeltaPitch {
        self.cyclic_pitch_deltas_deltas()
            .fold(0.0, |a, b| a + b.abs())
            / self.0.len() as f32
    }

    // /// vaguely normalized.
    // fn pitches_goodness(&self) -> Goodness {
    //     // -self
    //     //     .pitch_deltas()
    //     //     .fold(0.0, |a, b| a + b.abs() as Goodness)
    //     //     / self.0.len() as Goodness
    //     -self
    //         .cyclic_pitch_deltas_deltas()
    //         .fold(0.0, |a, b| a + b.abs() as Goodness)
    //         / self.0.len() as Goodness
    // }

    /// the gradient of goodness with respect to the pitch at index i.
    ///
    /// for central difference, we do goodness after a cycle,
    /// rather than goodness after steady state,
    /// because it's more differentiable that way.
    /// (also also cheaper)
    ///
    /// &mut self bc we want to modify self in place instead of cloning,
    /// but we guarantee that we won't be different after return.
    pub fn grad_at_tick(&mut self, init_vel: Vel3, tick: usize) -> DeltaPitch {
        const EPSILON: f64 = 0.1;

        let cur_pitch = self.0[tick];

        let right_pitch = cur_pitch + EPSILON as f32;
        let right_goodness = if right_pitch == clamped_pitch(right_pitch) {
            self.0[tick] = right_pitch;
            Some(goodness(&self.after_cycle(init_vel), self))
            // let mut slf = self.clone();
            // slf.pitches.0[pitch_i] = right_pitch;
            // slf.steady_vel = slf.pitches.steady_vel_guessed(slf.steady_vel);
            // Some(slf.pitches.after_cycle(slf.steady_vel).goodness())
        } else {
            None
        };

        let left_pitch = cur_pitch - EPSILON as f32;
        let left_goodness = if left_pitch == clamped_pitch(left_pitch) {
            self.0[tick] = left_pitch;
            Some(goodness(&self.after_cycle(init_vel), self))
            // let mut slf = self.clone();
            // slf.pitches.0[pitch_i] = left_pitch;
            // slf.steady_vel = slf.pitches.steady_vel_guessed(slf.steady_vel);
            // Some(slf.pitches.after_cycle(slf.steady_vel).goodness())
        } else {
            None
        };

        // only compute this if we need to, otherwise we can use central difference
        self.0[tick] = cur_pitch;
        let cur_goodness = if left_goodness.is_none() || right_goodness.is_none() {
            // TODO: cache this
            Some(goodness(&self.after_cycle(init_vel), self))
        } else {
            None
        };

        (match (left_goodness, right_goodness) {
            // central difference if we can
            (Some(left_goodness), Some(right_goodness)) => {
                (right_goodness - left_goodness) / (2.0 * EPSILON)
            }
            (None, Some(right_goodness)) => (right_goodness - cur_goodness.unwrap()) / EPSILON,
            (Some(left_goodness), None) => (cur_goodness.unwrap() - left_goodness) / EPSILON,
            (None, None) => unreachable!(),
        }) as f32
    }

    /// &mut self bc we want to modify self in place instead of cloning,
    /// but we guarantee that we won't be different after return.
    fn grad(&mut self, init_vel: Vel3) -> impl Iterator<Item = DeltaPitch> {
        (0..self.0.len()).map(move |i| self.grad_at_tick(init_vel, i))
    }

    /// applies one step of gradient descent.
    fn gradient_descent_step(&mut self, init_vel: Vel3, learning_rate: f64) {
        let grads = self.grad(init_vel).collect::<Vec<_>>();
        // TODO: try normalizing
        for (i, grad) in grads.into_iter().enumerate() {
            let cur_pitch = self.0[i];
            let delta_pitch = ((learning_rate as f32) * grad).clamp(-5.0, 5.0);
            self.0[i] = clamped_pitch(cur_pitch + delta_pitch);
        }
    }

    /// look try adding and subtracting delta to the pitch at index i,
    /// and return the delta that improves goodness the most,
    /// or return 0.0 if neither improves goodness.
    ///
    /// &mut self bc we want to modify self in place instead of cloning,
    /// but we guarantee that we won't be different after return.
    fn fixed_delta_at_tick(&mut self, init_vel: Vel3, delta: DeltaPitch, tick: usize) -> DeltaPitch {
        let cur_pitch = self.0[tick];
        // TODO: cache this
        let cur_goodness = goodness(&self.after_cycle(init_vel), self);

        // TODO: try doing goodness after steady state
        // instead of assuming it's the same for the delta.
        // actually i think it's better to not update steady state,
        // because it's more differentiable that way.
        // or actually that doesn't apply for this, only for grad.
        let right_pitch = cur_pitch + delta;
        if right_pitch == clamped_pitch(right_pitch) {
            self.0[tick] = right_pitch;
            let right_goodness = goodness(&self.after_cycle(init_vel), self);
            self.0[tick] = cur_pitch;
            if right_goodness > cur_goodness {
                return delta;
            }
        }

        let left_pitch = cur_pitch - delta;
        if left_pitch == clamped_pitch(left_pitch) {
            self.0[tick] = left_pitch;
            let left_goodness = goodness(&self.after_cycle(init_vel), self);
            self.0[tick] = cur_pitch;
            if left_goodness > cur_goodness {
                return -delta;
            }
        }

        0.0
    }

    /// &mut self bc we want to modify self in place instead of cloning,
    /// but we guarantee that we won't be different after return.
    fn fixed_delta(
        &mut self,
        init_vel: Vel3,
        delta: DeltaPitch,
    ) -> impl Iterator<Item = DeltaPitch> {
        (0..self.0.len()).map(move |i| self.fixed_delta_at_tick(init_vel, delta, i))
    }

    /// applies one step of fixed delta descent to the pitches.
    fn fixed_delta_step(&mut self, init_vel: Vel3, delta: DeltaPitch) {
        let deltas = self.fixed_delta(init_vel, delta).collect::<Vec<_>>();
        for (i, delta) in deltas.into_iter().enumerate() {
            let cur_pitch = self.0[i];
            self.0[i] = clamped_pitch(cur_pitch + delta);
        }
    }
}

/// linear combination of various goodnesses.
fn goodness(state: &State, pitches: &Pitches) -> Goodness {
    state.state_goodness()
        - 0.01 * pitches.cyclic_pitch_deltas_abs_average() as Goodness
        // - 0.01 * pitches.cyclic_pitch_deltas_deltas_abs_average() as Goodness
}

/// optimize with the constraint that
/// our velocity is the same before and after applying the pitches for a cycle.
/// (we find this by iterating the cycle until it converges)
#[derive(Debug, Clone)]
pub struct OptimizerSteadyState {
    pub steady_vel: Vel3,
    pub pitches: Pitches,
}
impl OptimizerSteadyState {
    /// steady_vel_guessed doesn't need to be good,
    /// but if you don't have any guess, use [`Self::new`] instead.
    pub fn from_guessed(steady_vel_guessed: Vel3, pitches: Pitches) -> Self {
        let steady_vel = pitches.steady_vel_guessed(steady_vel_guessed);
        Self {
            steady_vel,
            pitches,
        }
    }

    /// if you have a guess for the steady state velocity,
    /// you can use [`Self::from_guessed`] instead.
    pub fn new(pitches: Pitches) -> Self {
        // just use Vel::ZERO as the guess.
        // this is mostly to document that you don't have a guess.
        Self::from_guessed(Vel3::ZERO, pitches)
    }

    /// applies one step of gradient descent to the pitches,
    /// and updates the init_vel to be the new steady state velocity.
    // TODO: cache the value of forward passes.
    pub fn gradient_descent_step(&mut self, learning_rate: f64) {
        self.pitches
            .gradient_descent_step(self.steady_vel, learning_rate);
        self.steady_vel = self.pitches.steady_vel_guessed(self.steady_vel)
    }

    /// applies one step of gradient descent to the pitches,
    /// and updates the init_vel to be the new steady state velocity.
    // TODO: cache the value of forward passes.
    pub fn fixed_delta_step(&mut self, delta: DeltaPitch) {
        self.pitches.fixed_delta_step(self.steady_vel, delta);
        self.steady_vel = self.pitches.steady_vel_guessed(self.steady_vel)
    }

    /// cursed hack bc i don't want to make a trait.
    /// (i'm commenting out stuff to toggle between `OptimizerSteadState` and `OptimizerInitState`)
    pub fn init_vel(&self) -> Vel3 {
        self.steady_vel
    }
}

/// optimize from a fixed initial velocity.
#[derive(Debug, Clone)]
pub struct OptimizerInitState {
    pub init_vel: Vel3,
    pub pitches: Pitches,
}
impl OptimizerInitState {
    pub fn new(init_vel: Vel3, pitches: Pitches) -> Self {
        Self { init_vel, pitches }
    }

    /// applies one step of gradient descent to the pitches.
    /// the init_vel doesn't change.
    // TODO: cache the value of forward passes.
    pub fn gradient_descent_step(&mut self, learning_rate: f64) {
        self.pitches
            .gradient_descent_step(self.init_vel, learning_rate);
    }

    /// applies one step of fixed delta descent to the pitches.
    /// the init_vel doesn't change.
    // TODO: cache the value of forward passes.
    pub fn fixed_delta_step(&mut self, delta: DeltaPitch) {
        self.pitches.fixed_delta_step(self.init_vel, delta);
    }

    pub fn init_vel(&self) -> Vel3 {
        self.init_vel
    }
}

pub trait Optimizer {
    fn init_vel(&self) -> Vel3;
    fn pitches(&self) -> &Pitches;
    fn pitches_mut(&mut self) -> &mut Pitches;
    fn gradient_descent_step(&mut self, learning_rate: f64);
    fn fixed_delta_step(&mut self, delta: DeltaPitch);
}
impl Optimizer for OptimizerSteadyState {
    fn init_vel(&self) -> Vel3 {
        self.steady_vel
    }

    fn pitches(&self) -> &Pitches {
        &self.pitches
    }

    fn pitches_mut(&mut self) -> &mut Pitches {
        &mut self.pitches
    }

    fn gradient_descent_step(&mut self, learning_rate: f64) {
        Self::gradient_descent_step(self, learning_rate);
    }

    fn fixed_delta_step(&mut self, delta: DeltaPitch) {
        Self::fixed_delta_step(self, delta);
    }
}
impl Optimizer for OptimizerInitState {
    fn init_vel(&self) -> Vel3 {
        self.init_vel
    }

    fn pitches(&self) -> &Pitches {
        &self.pitches
    }

    fn pitches_mut(&mut self) -> &mut Pitches {
        &mut self.pitches
    }

    fn gradient_descent_step(&mut self, learning_rate: f64) {
        Self::gradient_descent_step(self, learning_rate);
    }

    fn fixed_delta_step(&mut self, delta: DeltaPitch) {
        Self::fixed_delta_step(self, delta);
    }
}
