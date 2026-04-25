use crate::sim::*;

type Goodness = f64;

fn approx_eq_f32(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}
fn approx_eq_f64(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.001
}

#[derive(Debug, Default, Clone)]
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

    fn goodness(&self) -> Goodness {
        todo!()
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

pub struct Pitches(pub Vec<f32>);
impl Pitches {
    pub fn new(ticks: usize) -> Self {
        Self(vec![0.0; ticks])
    }

    // /// of len self.0.len() + 1
    // pub fn cycle(&self, init_vel: Vec3) -> Vec<State> {
    //     let mut states = Vec::with_capacity(self.0.len() + 1);
    //     let mut cur = State {
    //         pos: Vec3::ZERO,
    //         vel: init_vel,
    //     };
    //     states.push(cur.clone());
    //     for pitch in self.0.iter() {
    //         let rot = Rot { x: *pitch, y: 0.0 };
    //         cur = cur.ticked(rot);
    //         states.push(cur.clone());
    //     }
    //     states
    // }
    /// of len self.0.len()
    pub fn cycle(&self, init_vel: Vec3) -> Vec<State> {
        let mut states = Vec::with_capacity(self.0.len() + 1);
        let mut cur = State {
            pos: Vec3::ZERO,
            vel: init_vel,
        };
        for pitch in self.0.iter() {
            let rot = Rot { x: *pitch, y: 0.0 };
            cur = cur.ticked(rot);
            states.push(cur.clone());
        }
        states
    }

    /// given this init velocity, return the state after applying the pitches.
    pub fn after_cycle(&self, vel: Vec3) -> State {
        let mut cur = State {
            pos: Vec3::ZERO,
            vel,
        };
        for pitch in self.0.iter() {
            let rot = Rot { x: *pitch, y: 0.0 };
            cur = cur.ticked(rot);
        }
        cur
    }

    /// init vel is a guess at the stead state velocity.
    fn steady_vel_guessed(&self, steady_vel_guess: Vec3) -> Vec3 {
        let mut state = self.after_cycle(steady_vel_guess);
        loop {
            let next = self.after_cycle(state.vel);
            if approx_eq_f64(state.pos.x, next.pos.x)
                && approx_eq_f64(state.pos.y, next.pos.y)
                && approx_eq_f64(state.pos.z, next.pos.z)
            {
                break;
            }
            state = next;
        }
        state.vel
    }

    /// factor out bc we may have better heuristics in the future.
    fn steady_vel(&self) -> Vec3 {
        self.steady_vel_guessed(Vec3::ZERO)
    }
}

pub struct Optimizer {
    /// doesn't need to be exactly to maintain the invariant of the type,
    /// but should only diverge when inside the api boundary.
    /// outside the api, it should be exact.
    pub steady_vel: Vec3,
    pub pitches: Pitches,
}
impl Optimizer {
    fn optimization_step_pitch(&mut self, pitch_i: usize) {
        const EPSILON: f64 = 0.01;
        const LEARNING_RATE: f64 = 0.1;

        let cur_pitch = self.pitches.0[pitch_i];
        let cur_goodness = self.pitches.after_cycle(self.steady_vel).goodness();

        let right_pitch = cur_pitch + EPSILON as f32;
        self.pitches.0[pitch_i] = right_pitch;
        let right_goodness = self.pitches.after_cycle(self.steady_vel).goodness();

        let grad = (right_goodness - cur_goodness) / EPSILON;
        self.pitches.0[pitch_i] = cur_pitch - (LEARNING_RATE * grad) as f32;
    }

    /// apply one step of optimization to the pitches.
  pub  fn optimization_step(&mut self) {
        for i in 0..self.pitches.0.len() {
            self.optimization_step_pitch(i);
        }
        self.steady_vel = self.pitches.steady_vel();
    }

    // fn show(&self, ui: &mut egui::Ui) {}
}
impl From<Pitches> for Optimizer {
    fn from(pitches: Pitches) -> Self {
        let steady_vel = pitches.steady_vel();
        Self {
            steady_vel,
            pitches,
        }
    }
}
