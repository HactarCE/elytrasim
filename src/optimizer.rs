use std::sync::atomic::{self, AtomicBool};

use itertools::Itertools;
use rand::prelude::*;

use crate::sim::*;

pub type Goodness = f64;
pub type Energy = f64;
pub type Pos3 = Vec3;
pub type Vel3 = Vec3;
pub type Pitch = f32;

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
    pub init_vel_y_std: f64,
    pub init_vel_z_std: f64,
}

#[derive(Debug, Clone)]
pub struct Jitter {
    pub init_vel: Vel3,
}
impl Jitter {
    pub fn new(params: &JitterParams) -> Self {
        let mut ret = Self {
            init_vel: Vel3::ZERO,
        };
        ret.resample_init_vel_y(params.init_vel_y_std);
        ret.resample_init_vel_z(params.init_vel_z_std);
        ret
    }

    pub fn zero() -> Self {
        Self {
            init_vel: Vel3::ZERO,
        }
    }

    pub fn resample_all(&mut self, params: &JitterParams) {
        self.resample_init_vel_y(params.init_vel_y_std);
        self.resample_init_vel_z(params.init_vel_z_std);
    }

    pub fn resample_init_vel_y(&mut self, std: f64) {
        self.init_vel.y = Self::sample(std).next().unwrap();
    }

    pub fn resample_init_vel_z(&mut self, std: f64) {
        self.init_vel.z = Self::sample(std).next().unwrap();
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

pub use splat::*;
mod splat {
    use super::*;

    /// check out this [graph](https://www.desmos.com/calculator/z9cfqdndyx).
    /// the stored parameters aren't in the nice user facing format,
    /// they're in a format that's better for optimization.
    /// actually don't bother with rad for now, it's kinda annoying.
    #[derive(Debug, Clone)]
    pub struct Mask {
        mu: f32,
        sigma: f32,
    }
    impl Mask {
        const ZERO: Self = Self {
            mu: 0.0,
            sigma: 0.0,
        };

        const RAD_SCALE: f32 = 3.0 - 2.0 * std::f32::consts::SQRT_2;

        pub fn new(mu: f32, sigma: f32) -> Self {
            Self { mu, sigma }
        }

        pub fn mu(&self) -> f32 {
            self.mu
        }
        pub fn sigma(&self) -> f32 {
            self.sigma
        }

        pub fn set_mu(&mut self, mu: f32) {
            self.mu = mu;
        }
        pub fn set_sigma(&mut self, sigma: f32) {
            self.sigma = sigma;
        }

        fn into_array(self) -> [f32; 2] {
            [self.mu, self.sigma]
        }
        fn from_array([mu, sigma]: [f32; 2]) -> Self {
            Self { mu, sigma }
        }

        fn forward(&self, x: f32) -> f32 {
            1.0 / (1.0 + (-self.sigma * (x - self.mu)).exp())
        }

        // /// guarantee that at least `1.0 - epsilon` of the area
        // /// is contained in the bounding box.
        // fn aabb(&self, epsilon: f32) -> [f32; 2] {}
    }

    #[derive(Debug, Clone)]
    pub struct Affine<const N: usize> {
        pub weights: [f32; N],
        pub bias: f32,
    }
    impl<const N: usize> Affine<N> {
        const ZERO: Self = Self {
            weights: [0.0; N],
            bias: 0.0,
        };

        // `Vec` bc rust doesn't understand `[f32; N + 1]`.
        fn into_array(self) -> Vec<f32> {
            self.weights
                .into_iter()
                .chain(std::iter::once(self.bias))
                .collect()
        }
        fn from_array(arr: &[f32]) -> Self {
            Self {
                weights: arr[..N].try_into().unwrap(),
                bias: arr[N],
            }
        }

        fn forward(&self, x: &[f32; N]) -> f32 {
            self.weights.iter().zip(x).map(|(w, x)| w * x).sum::<f32>() + self.bias
        }
    }

    #[derive(Debug, Clone)]
    pub struct Term {
        pub tick_mask: Mask,
        // TODO: this should get put through a 90*sigmoid (or something that's near linear at the origin)
        // the ui stuff can get fed through the sigmoid_inv before being show.
        pub pitch_map: Affine<3>,
        // weight:
    }
    impl Term {
        const ZERO: Self = Self {
            tick_mask: Mask::ZERO,
            pitch_map: Affine::ZERO,
        };

        pub fn new_random() -> Self {
            let mut rng = rand::rng();
            let tick_mask = Mask::new(
                rng.random_range(-50.0..=500.0),
                rng.random_range(-5.0..=5.0),
            );
            let pitch_map = Affine {
                weights: [
                    rng.random_range(-0.5..=0.5),
                    rng.random_range(-1.0..=1.0),
                    rng.random_range(-1.0..=1.0),
                ],
                bias: rng.random_range(-80.0..=80.0),
            };
            Term {
                tick_mask,
                pitch_map,
            }
        }

        fn into_array(self) -> [f32; 6] {
            self.tick_mask
                .into_array()
                .into_iter()
                .chain(self.pitch_map.into_array())
                .collect_vec()
                .try_into()
                .unwrap()
        }
        fn from_array(arr: [f32; 6]) -> Self {
            Self {
                tick_mask: Mask::from_array(arr[..2].try_into().unwrap()),
                pitch_map: Affine::from_array(arr[2..].try_into().unwrap()),
            }
        }

        fn forward(&self, tick: usize, vel: Vel3) -> Pitch {
            let x = [tick as f32, vel.y as f32, vel.z as f32];
            let mask = self.tick_mask.forward(x[0]);
            let pitch = mask * self.pitch_map.forward(&x);
            // activation function to soft clamp the pitch to [-90, 90].
            // check out this [graph](https://www.desmos.com/calculator/y5qqjt4m8w).
            90.0 * (2.0 / (1.0 + (-(2.0 / 90.0) * pitch).exp()) - 1.0)
        }
    }

    /// this isn't really a neural network but whatever.
    #[derive(Debug, Clone)]
    pub struct Nn {
        // TODO: maybe a bvh?
        pub terms: Vec<Term>,
    }
    impl Nn {
        pub fn zero(num_terms: usize) -> Self {
            Self {
                terms: vec![Term::ZERO; num_terms],
            }
        }

        pub fn new_40_0_down() -> Self {
            Self {
                terms: vec![
                    Term {
                        tick_mask: Mask::new(110.0, 3.0),
                        pitch_map: Affine {
                            weights: [0.0, 0.0, 0.0],
                            bias: 40.0,
                        },
                    },
                    Term {
                        tick_mask: Mask::new(90.0, -3.0),
                        pitch_map: Affine {
                            weights: [-0.5, 0.0, 0.0],
                            bias: -10.0,
                        },
                    },
                ],
            }
        }

        /// the guess of the best pitch at the given tick and velocity.
        pub fn forward(&self, tick: usize, vel: Vel3) -> Pitch {
            let forward = self
                .terms
                .iter()
                .map(|term| term.forward(tick, vel))
                .sum::<f32>();
            PitchUtil::clamped(forward)
        }

        /// do `num_ticks` autoregressive steps.
        pub fn forward_iter(
            &self,
            num_ticks: usize,
            init_vel: Vel3,
        ) -> impl Iterator<Item = (Pitch, State)> {
            let mut pos_accumulator = Vec3::ZERO;
            let mut vel = init_vel;

            (0..num_ticks).rev().map(move |tick| {
                let pitch = self.forward(tick, vel);

                let state = State {
                    pos: Vec3::ZERO,
                    vel,
                }
                .ticked(pitch);

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

        /// do `num_ticks` autoregressive steps and return the final state.
        pub fn forward_last(&self, num_ticks: usize, init_vel: Vel3) -> State {
            self.forward_iter(num_ticks, init_vel).last().unwrap().1
        }

        /// `goodness` of the final state after `num_ticks` autoregressive steps.
        pub fn goodness(
            &self,
            goodness: impl Fn(State) -> Goodness,
            num_ticks: usize,
            init_vel: Vel3,
        ) -> Goodness {
            goodness(self.forward_last(num_ticks, init_vel))
        }

        /// the gradient of goodness with respect to each parameter.
        ///
        /// &mut self bc we want to modify `self` in place instead of cloning,
        /// but we guarantee that it won't be different after return.
        pub fn get_grad(
            &mut self,
            goodness: impl Fn(State) -> Goodness,
            num_ticks: usize,
            init_vel: Vel3,
        ) -> Self {
            // you could also find the grad of pitches
            // then tell the terms to make the pitches closer to that target.
            // and you could do this in parallel for each term, then sum the terms at the end.

            const EPSILON: f32 = 0.01;

            let cur_goodness = self.goodness(&goodness, num_ticks, init_vel);

            let mut grad = Nn::zero(self.terms.len());

            for term_idx in 0..self.terms.len() {
                let mut term_arr = self.terms[term_idx].clone().into_array();
                let mut grad_term_arr = grad.terms[term_idx].clone().into_array();

                for parameter_idx in 0..term_arr.len() {
                    let mid = term_arr[parameter_idx];
                    let right = mid + EPSILON;

                    term_arr[parameter_idx] = right;
                    self.terms[term_idx] = Term::from_array(term_arr);

                    let right_goodness = self.goodness(&goodness, num_ticks, init_vel);

                    term_arr[parameter_idx] = mid;
                    self.terms[term_idx] = Term::from_array(term_arr);

                    grad_term_arr[parameter_idx] = (right_goodness - cur_goodness) as f32 / EPSILON;
                }

                grad.terms[term_idx] = Term::from_array(grad_term_arr);
            }

            grad
        }

        pub fn apply_grad(&mut self, grad: &Self, learning_rate: f32) {
            assert_eq!(self.terms.len(), grad.terms.len());
            for (term, grad_term) in self.terms.iter_mut().zip(grad.terms.iter()) {
                let mut term_arr = term.clone().into_array();
                let grad_arr = grad_term.clone().into_array();
                for i in 0..term_arr.len() {
                    term_arr[i] += learning_rate * grad_arr[i];
                }
                *term = Term::from_array(term_arr);
            }
        }

        /// 0.0 means no decay.
        pub fn decay(&mut self, decay: f32) {
            for term in self.terms.iter_mut() {
                let mut term_arr = term.clone().into_array();
                for i in 0..term_arr.len() {
                    term_arr[i] *= decay;
                }
                *term = Term::from_array(term_arr);
            }
        }
    }
    impl std::ops::AddAssign<&Self> for Nn {
        fn add_assign(&mut self, rhs: &Self) {
            assert_eq!(self.terms.len(), rhs.terms.len());
            for (term, rhs_term) in self.terms.iter_mut().zip(rhs.terms.iter()) {
                let mut term_arr = term.clone().into_array();
                let rhs_arr = rhs_term.clone().into_array();
                for i in 0..term_arr.len() {
                    term_arr[i] += rhs_arr[i];
                }
                *term = Term::from_array(term_arr);
            }
        }
    }
    impl std::ops::Add<&Self> for Nn {
        type Output = Self;
        fn add(mut self, rhs: &Self) -> Self {
            self += rhs;
            self
        }
    }
    impl std::ops::MulAssign<f32> for Nn {
        fn mul_assign(&mut self, rhs: f32) {
            for term in self.terms.iter_mut() {
                let mut term_arr = term.clone().into_array();
                for i in 0..term_arr.len() {
                    term_arr[i] *= rhs;
                }
                *term = Term::from_array(term_arr);
            }
        }
    }
    impl std::ops::Mul<f32> for Nn {
        type Output = Self;
        fn mul(mut self, rhs: f32) -> Self {
            self *= rhs;
            self
        }
    }
    impl std::ops::DivAssign<f32> for Nn {
        fn div_assign(&mut self, rhs: f32) {
            for term in self.terms.iter_mut() {
                let mut term_arr = term.clone().into_array();
                for i in 0..term_arr.len() {
                    term_arr[i] /= rhs;
                }
                *term = Term::from_array(term_arr);
            }
        }
    }
    impl std::ops::Div<f32> for Nn {
        type Output = Self;
        fn div(mut self, rhs: f32) -> Self {
            self /= rhs;
            self
        }
    }

    pub struct Adam {
        m: Vec<[f32; 6]>,
        v: Vec<[f32; 6]>,
        t: u64,
        pub beta1: f32,
        pub beta2: f32,
        pub epsilon: f32,
        pub weight_decay: f32,
    }
    impl Adam {
        pub fn new(num_terms: usize) -> Self {
            Self {
                m: vec![[0.0; 6]; num_terms],
                v: vec![[0.0; 6]; num_terms],
                t: 0,
                beta1: 0.9,
                beta2: 0.999,
                epsilon: 1e-8,
                weight_decay: 0.01,
            }
        }

        pub fn reset(&mut self, num_terms: usize) {
            self.m = vec![[0.0; 6]; num_terms];
            self.v = vec![[0.0; 6]; num_terms];
            self.t = 0;
        }

        pub fn step(&mut self, nn: &mut Nn, grad: &Nn, learning_rate: f32) {
            assert_eq!(nn.terms.len(), grad.terms.len());
            assert_eq!(nn.terms.len(), self.m.len());
            assert_eq!(nn.terms.len(), self.v.len());

            self.t += 1;
            let t = self.t;
            let b1 = self.beta1;
            let b2 = self.beta2;
            let eps = self.epsilon;
            let wd = self.weight_decay;
            let lr = learning_rate;

            let bc1 = 1.0 - b1.powi(t as i32);
            let bc2 = 1.0 - b2.powi(t as i32);

            for term_idx in 0..nn.terms.len() {
                let mut param = nn.terms[term_idx].clone().into_array();
                let g = grad.terms[term_idx].clone().into_array();

                for i in 0..6 {
                    self.m[term_idx][i] = b1 * self.m[term_idx][i] + (1.0 - b1) * g[i];
                    self.v[term_idx][i] = b2 * self.v[term_idx][i] + (1.0 - b2) * g[i] * g[i];

                    let m_hat = self.m[term_idx][i] / bc1;
                    let v_hat = self.v[term_idx][i] / bc2;

                    param[i] = param[i] * (1.0 - lr * wd) + lr * m_hat / (v_hat.sqrt() + eps);
                }

                nn.terms[term_idx] = Term::from_array(param);
            }
        }
    }
}
