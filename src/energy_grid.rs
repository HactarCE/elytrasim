use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

pub type GridCoord = f32;
pub type Goodness = f64;

pub static LOOKAHEAD: AtomicUsize = AtomicUsize::new(1);

/// don't store samples at center of cells.
pub struct Grid<T>(pub Box<[Box<[T]>]>);

#[derive(Debug, Clone, PartialEq)]
pub struct GridMeta {
    pub width: usize,
    pub height: usize,
    pub y_vel_lo: Vel,
    pub y_vel_hi: Vel,
    pub z_vel_lo: Vel,
    pub z_vel_hi: Vel,
}
impl GridMeta {
    pub fn new_uniform(
        max_width_height: usize,
        y_vel_mid: Vel,
        z_vel_lo: Vel,
        z_vel_hi: Vel,
        rect: egui::Rect,
    ) -> Self {
        let scale = rect.height() / rect.width();
        assert!(
            scale > 0.,
            "rect must have positive width and height, got rect: {:?}",
            rect
        );
        let (width, height) = if scale < 1. {
            let width = max_width_height;
            let height = ((max_width_height as f32 * scale).round() as usize).max(1);
            (width, height)
        } else {
            let height = max_width_height;
            let width = ((max_width_height as f32 / scale).round() as usize).max(1);
            (width, height)
        };
        assert!(
            width > 0 && height > 0,
            "width and height must be positive, got width: {}, height: {}. rect: {:?}",
            width,
            height,
            rect
        );
        assert_eq!(usize::max(width, height), max_width_height);
        // let height = (max_width_height as f32 * scale).round() as usize;
        const MIN_DELTA_Z_VEL: Vel = 0.01;
        let delta_z_vel = (z_vel_hi - z_vel_lo).max(MIN_DELTA_Z_VEL);
        let delta_y_vel = delta_z_vel * scale as Vel;
        assert!(delta_y_vel > 0.);
        assert!(delta_z_vel > 0.);

        Self {
            width,
            height,
            y_vel_lo: y_vel_mid - delta_y_vel / 2.,
            y_vel_hi: y_vel_mid + delta_y_vel / 2.,
            z_vel_lo,
            z_vel_hi,
        }
    }

    // fn assert_is_uniform(&self) {
    //     let delta_z_vel = self.z_vel_hi - self.z_vel_lo;
    //     let delta_y_vel = self.y_vel_hi - self.y_vel_lo;
    //     let scale = delta_y_vel / delta_z_vel;
    //     let expected_height = (self.width as f64 * scale).round() as usize;
    //     if (self.height as isize - expected_height as isize).abs() <= 1 {
    //         return;
    //     }
    //     eprintln!("grid is not uniform");
    //     dbg!(self.width, self.height, scale, expected_height);
    //     panic!("grid is not uniform");
    // }

    pub fn vel_step(&self) -> Vel {
        let horizontal_step = (self.z_vel_hi - self.z_vel_lo) / self.width as Vel;
        // let vertical_step = (self.y_vel_hi - self.y_vel_lo) / self.height as Vel;
        // self.assert_is_uniform();
        horizontal_step
    }

    pub fn egui_step(&self, rect: egui::Rect) -> f32 {
        let horizontal_step = rect.width() / self.width as f32;
        // let vertical_step = rect.height() / self.height as f32;
        // self.assert_is_uniform();
        horizontal_step
    }

    pub fn vel_to_grid_row_col_float(&self, vel: Vel3) -> (GridCoord, GridCoord) {
        // assert_eq!(vel.x, 0., "not a hard error, but probably should have this");
        (
            lerp_f32(
                0.,
                self.height as GridCoord,
                1.0 - inv_lerp_f64(self.y_vel_lo, self.y_vel_hi, vel.y) as GridCoord,
            ) as GridCoord,
            lerp_f32(
                0.,
                self.width as GridCoord,
                inv_lerp_f64(self.z_vel_lo, self.z_vel_hi, vel.z) as GridCoord,
            ) as GridCoord,
        )
    }

    pub fn vel_to_grid_row_col_usize(&self, vel: Vel3) -> (usize, usize) {
        let (row, col) = self.vel_to_grid_row_col_float(vel);
        (row.floor() as usize, col.floor() as usize)
    }

    pub fn row_col_float_to_vel(&self, (row, col): (GridCoord, GridCoord)) -> Vel3 {
        Vec3 {
            x: 0.,
            y: lerp_f64(
                self.y_vel_lo,
                self.y_vel_hi,
                1.0 - inv_lerp_f32(0., self.height as GridCoord, row) as Vel,
            ),
            z: lerp_f64(
                self.z_vel_lo,
                self.z_vel_hi,
                inv_lerp_f32(0., self.width as GridCoord, col) as Vel,
            ),
        }
    }

    /// from the center of the cell
    pub fn row_col_usize_to_vel(&self, (row, col): (usize, usize)) -> Vel3 {
        self.row_col_float_to_vel((row as GridCoord, col as GridCoord))
    }

    pub fn vel_to_egui_pos2(&self, vel: Vel3, rect: egui::Rect) -> egui::Pos2 {
        let (row, col) = self.vel_to_grid_row_col_float(vel);
        rect.left_top() + egui::vec2(col, row) * self.egui_step(rect)
    }

    pub fn egui_pos2_to_vel(&self, pos: egui::Pos2, rect: egui::Rect) -> Vel3 {
        let egui_step = self.egui_step(rect);
        let col = (pos.x - rect.left()) / egui_step;
        let row = (pos.y - rect.top()) / egui_step;
        self.row_col_float_to_vel((row, col))
    }

    pub fn row_col_float_to_egui_pos2(
        &self,
        (row, col): (GridCoord, GridCoord),
        rect: egui::Rect,
    ) -> egui::Pos2 {
        rect.left_top() + egui::vec2(col, row) * self.egui_step(rect)
    }

    /// from the center of the cell
    pub fn row_col_usize_to_egui_pos2(
        &self,
        (row, col): (usize, usize),
        rect: egui::Rect,
    ) -> egui::Pos2 {
        self.row_col_float_to_egui_pos2((row as GridCoord, col as GridCoord), rect)
    }

    pub fn rects(
        &self,
        rect: egui::Rect,
    ) -> impl Iterator<Item = impl Iterator<Item = egui::Rect>> {
        let step = self.egui_step(rect);
        (0..self.height).map(move |row| {
            (0..self.width).map(move |col| {
                egui::Rect::from_center_size(
                    rect.left_top() + egui::vec2(col as f32, row as f32) * step,
                    egui::Vec2::splat(step),
                )
            })
        })
    }
}

impl Grid<DeltaTotalEnergy> {
    pub fn from_fixed_pitch(meta: &GridMeta, pitch: Pitch) -> Self {
        Self(
            (0..meta.height)
                .map(|row| {
                    (0..meta.width)
                        .map(|col| {
                            let vel = meta.row_col_usize_to_vel((row, col));
                            delta_total_energy_for_vel_at_pitch(vel, pitch)
                        })
                        .collect()
                })
                .collect(),
        )
    }
}

impl Grid<f32> {
    pub fn f32_bilinear_from_row_col_float(
        &self,
        (row, col): (GridCoord, GridCoord),
    ) -> Option<f32> {
        let row_lo = row.floor() as usize;
        let col_lo = col.floor() as usize;
        let row_hi = row_lo + 1;
        let col_hi = col_lo + 1;
        let row_frac = row - row_lo as GridCoord;
        let col_frac = col - col_lo as GridCoord;
        let energy_ll = *self.0.get(row_lo)?.get(col_lo)?;
        let energy_lh = *self.0.get(row_lo)?.get(col_hi)?;
        let energy_hl = *self.0.get(row_hi)?.get(col_lo)?;
        let energy_hh = *self.0.get(row_hi)?.get(col_hi)?;
        Some(lerp_f32(
            lerp_f32(energy_ll, energy_lh, col_frac),
            lerp_f32(energy_hl, energy_hh, col_frac),
            1.0 - row_frac,
        ))
    }
}

impl Grid<f64> {
    pub fn f64_bilinear_from_row_col_float(
        &self,
        (row, col): (GridCoord, GridCoord),
    ) -> Option<f64> {
        let row_lo = row.floor() as usize;
        let col_lo = col.floor() as usize;
        let row_hi = row_lo + 1;
        let col_hi = col_lo + 1;
        let row_frac = row - row_lo as GridCoord;
        let col_frac = col - col_lo as GridCoord;
        let energy_ll = *self.0.get(row_lo)?.get(col_lo)?;
        let energy_lh = *self.0.get(row_lo)?.get(col_hi)?;
        let energy_hl = *self.0.get(row_hi)?.get(col_lo)?;
        let energy_hh = *self.0.get(row_hi)?.get(col_hi)?;
        Some(lerp_f64(
            lerp_f64(energy_ll, energy_lh, col_frac as f64),
            lerp_f64(energy_hl, energy_hh, col_frac as f64),
            1.0 - row_frac as f64,
        ))
    }
}

impl Grid<State> {
    pub fn state_bilinear_from_row_col_float(
        &self,
        (row, col): (GridCoord, GridCoord),
    ) -> Option<State> {
        let row_lo = row.floor() as usize;
        let col_lo = col.floor() as usize;
        let row_hi = row_lo + 1;
        let col_hi = col_lo + 1;
        let row_frac = row - row_lo as GridCoord;
        let col_frac = col - col_lo as GridCoord;
        let energy_ll = self.0.get(row_lo)?.get(col_lo)?;
        let energy_lh = self.0.get(row_lo)?.get(col_hi)?;
        let energy_hl = self.0.get(row_hi)?.get(col_lo)?;
        let energy_hh = self.0.get(row_hi)?.get(col_hi)?;
        Some(lerp_state(
            &lerp_state(energy_ll, energy_lh, col_frac as f64),
            &lerp_state(energy_hl, energy_hh, col_frac as f64),
            1.0 - row_frac as f64,
        ))
    }
}

// def goodness(state) is that we tick state n times,
// then take goodness_heuristic(ticked_state) := total_energy(ticked_state).
// the point of what we're doing is so that we don't need to tick states n times to get this value,
// we have caches of this value for tick n - 1 on a grid,
// TODO: remove storing states.
// so how does it happen that a vel gets a higher goodness?
// it got a higher goodness from a place that had a higher goodness.
// TODO: i need to store a 3d grid of goodness, indexed by vel and y,
// because that's what total_energy is computed from.

#[derive(Debug, Clone)]
pub struct DPMeta {
    y_pos_lo: Pos,
    y_pos_hi: Pos,
    y_vel_lo: Vel,
    y_vel_hi: Vel,
    z_vel_lo: Vel,
    z_vel_hi: Vel,
    y_pos_width: usize,
    y_vel_width: usize,
    z_vel_width: usize,
}
impl DPMeta {
    pub fn from_grid_meta(
        grid_meta: &GridMeta,
        y_pos_lo: Pos,
        y_pos_hi: Pos,
        y_pos_width: usize,
    ) -> Self {
        Self {
            y_pos_lo,
            y_pos_hi,
            y_vel_lo: grid_meta.y_vel_lo,
            y_vel_hi: grid_meta.y_vel_hi,
            z_vel_lo: grid_meta.z_vel_lo,
            z_vel_hi: grid_meta.z_vel_hi,
            y_pos_width,
            y_vel_width: grid_meta.height,
            z_vel_width: grid_meta.width,
        }
    }

    fn index_usize_to_vals(
        &self,
        (y_pos_i, y_vel_i, z_vel_i): (usize, usize, usize),
    ) -> (Pos, Vel, Vel) {
        let y_pos = lerp_f64(
            self.y_pos_lo,
            self.y_pos_hi,
            y_pos_i as f64 / self.y_pos_width as f64,
        );
        let y_vel = lerp_f64(
            self.y_vel_lo,
            self.y_vel_hi,
            y_vel_i as f64 / self.y_vel_width as f64,
        );
        let z_vel = lerp_f64(
            self.z_vel_lo,
            self.z_vel_hi,
            z_vel_i as f64 / self.z_vel_width as f64,
        );
        (y_pos, y_vel, z_vel)
    }

    fn vals_to_index_float(
        &self,
        (y_pos, y_vel, z_vel): (Pos, Vel, Vel),
    ) -> (GridCoord, GridCoord, GridCoord) {
        let y_pos_i = (inv_lerp_f64(self.y_pos_lo, self.y_pos_hi, y_pos) * self.y_pos_width as f64)
            as GridCoord;
        let y_vel_i = (inv_lerp_f64(self.y_vel_lo, self.y_vel_hi, y_vel) * self.y_vel_width as f64)
            as GridCoord;
        let z_vel_i = (inv_lerp_f64(self.z_vel_lo, self.z_vel_hi, z_vel) * self.z_vel_width as f64)
            as GridCoord;
        (y_pos_i, y_vel_i, z_vel_i)
    }

    // fn values_to_index_usize(&self, vals: (Pos, Vel, Vel)) -> (usize, usize, usize) {
    //     let (y_pos_i_f, y_vel_i_f, z_vel_i_f) = self.vals_to_index_float(vals);
    //     (
    //         y_pos_i_f.floor() as usize,
    //         y_vel_i_f.floor() as usize,
    //         z_vel_i_f.floor() as usize,
    //     )
    // }
}

/// don't enforce uniform scaling
#[derive(Debug, Clone)]
pub struct DP {
    meta: DPMeta,
    arr: Box<[Box<[Box<[(Pitch, Goodness)]>]>]>,
}
impl DP {
    pub fn base(meta: DPMeta) -> Self {
        pub fn goodness_base((y_pos, y_vel, z_vel): (Pos, Vel, Vel)) -> Goodness {
            let state = State {
                pos: Vec3 {
                    x: 0.,
                    y: y_pos,
                    z: 0.,
                },
                vel: Vec3 {
                    x: 0.,
                    y: y_vel,
                    z: z_vel,
                },
            };
            state.total_energy()
        }

        Self {
            arr: (0..meta.y_pos_width)
                .map(|y_pos_i| {
                    (0..meta.y_vel_width)
                        .map(|y_vel_i| {
                            (0..meta.z_vel_width)
                                .map(|z_vel_i| {
                                    (
                                        0.0,
                                        goodness_base(
                                            meta.index_usize_to_vals((y_pos_i, y_vel_i, z_vel_i)),
                                        ),
                                    )
                                })
                                .collect()
                        })
                        .collect()
                })
                .collect(),
            meta,
        }
    }

    pub fn stepped(&self) -> Self {
        fn goodness_step(dp: &DP, (y_pos, y_vel, z_vel): (Pos, Vel, Vel)) -> (Pitch, Goodness) {
            let state = State {
                pos: Vec3 {
                    x: 0.,
                    y: y_pos,
                    z: 0.,
                },
                vel: Vec3 {
                    x: 0.,
                    y: y_vel,
                    z: z_vel,
                },
            };
            let mut best_pitch = 0.;
            let mut best_goodness = f64::NEG_INFINITY;
            for pitch in -90..=90 {
                let rot = Rot {
                    x: pitch as Pitch,
                    y: 0.,
                };
                let new_state = state.ticked(rot);
                // let (y_pos, y_vel, z_vel) = dp.meta.vals_to_index_float((
                //     new_state.pos.y,
                //     new_state.vel.y,
                //     new_state.vel.z,
                // ));
                let goodness = dp
                    .trilinear_from_vals((new_state.pos.y, new_state.vel.y, new_state.vel.z))
                    .map(|(_, goodness)| goodness)
                    .unwrap_or(f64::NEG_INFINITY);
                if goodness > best_goodness {
                    best_goodness = goodness;
                    best_pitch = pitch as Pitch;
                }
            }
            (best_pitch, best_goodness)
        }

        let arr = (0..self.meta.y_pos_width)
            .map(|y_pos_i| {
                (0..self.meta.y_vel_width)
                    .map(|y_vel_i| {
                        (0..self.meta.z_vel_width)
                            .map(|z_vel_i| {
                                goodness_step(
                                    self,
                                    self.meta.index_usize_to_vals((y_pos_i, y_vel_i, z_vel_i)),
                                )
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();

        Self {
            arr,
            meta: self.meta.clone(),
        }
    }

    pub fn trilinear_from_indexes(
        &self,
        (y_pos_i_f, y_vel_i_f, z_vel_i_f): (GridCoord, GridCoord, GridCoord),
    ) -> Option<(Pitch, Goodness)> {
        let y_pos_i_lo = y_pos_i_f.floor() as usize;
        let y_vel_i_lo = y_vel_i_f.floor() as usize;
        let z_vel_i_lo = z_vel_i_f.floor() as usize;
        let y_pos_i_hi = y_pos_i_lo + 1;
        let y_vel_i_hi = y_vel_i_lo + 1;
        let z_vel_i_hi = z_vel_i_lo + 1;
        let y_pos_frac = y_pos_i_f - y_pos_i_lo as GridCoord;
        let y_vel_frac = y_vel_i_f - y_vel_i_lo as GridCoord;
        let z_vel_frac = z_vel_i_f - z_vel_i_lo as GridCoord;

        // get the 8 surrounding values
        let (p000, g000) = *self.arr.get(y_pos_i_lo)?.get(y_vel_i_lo)?.get(z_vel_i_lo)?;
        let (p001, g001) = *self.arr.get(y_pos_i_lo)?.get(y_vel_i_lo)?.get(z_vel_i_hi)?;
        let (p010, g010) = *self.arr.get(y_pos_i_lo)?.get(y_vel_i_hi)?.get(z_vel_i_lo)?;
        let (p011, g011) = *self.arr.get(y_pos_i_lo)?.get(y_vel_i_hi)?.get(z_vel_i_hi)?;
        let (p100, g100) = *self.arr.get(y_pos_i_hi)?.get(y_vel_i_lo)?.get(z_vel_i_lo)?;
        let (p101, g101) = *self.arr.get(y_pos_i_hi)?.get(y_vel_i_lo)?.get(z_vel_i_hi)?;
        let (p110, g110) = *self.arr.get(y_pos_i_hi)?.get(y_vel_i_hi)?.get(z_vel_i_lo)?;
        let (p111, g111) = *self.arr.get(y_pos_i_hi)?.get(y_vel_i_hi)?.get(z_vel_i_hi)?;

        // trilinear interpolation
        Some((
            lerp_f32(
                lerp_f32(
                    lerp_f32(p000, p001, z_vel_frac as f32),
                    lerp_f32(p010, p011, z_vel_frac as f32),
                    y_vel_frac as f32,
                ),
                lerp_f32(
                    lerp_f32(p100, p101, z_vel_frac as f32),
                    lerp_f32(p110, p111, z_vel_frac as f32),
                    y_vel_frac as f32,
                ),
                y_pos_frac as f32,
            ),
            lerp_f64(
                lerp_f64(
                    lerp_f64(g000, g001, z_vel_frac as f64),
                    lerp_f64(g010, g011, z_vel_frac as f64),
                    y_vel_frac as f64,
                ),
                lerp_f64(
                    lerp_f64(g100, g101, z_vel_frac as f64),
                    lerp_f64(g110, g111, z_vel_frac as f64),
                    y_vel_frac as f64,
                ),
                y_pos_frac as f64,
            ),
        ))
    }

    pub fn trilinear_from_vals(
        &self,
        (y_pos, y_vel, z_vel): (Pos, Vel, Vel),
    ) -> Option<(Pitch, Goodness)> {
        let (y_pos_i_f, y_vel_i_f, z_vel_i_f) =
            self.meta.vals_to_index_float((y_pos, y_vel, z_vel));
        self.trilinear_from_indexes((y_pos_i_f, y_vel_i_f, z_vel_i_f))
    }

    pub fn step(&mut self) {
        *self = self.stepped();
    }
}

pub fn goodness_for_vel_y_after_ticks(
    y_pos: Pos,
    y_vel: Vel,
    z_vel: Vel,
    ticks: usize,
) -> Goodness {
    let state = State {
        pos: Vec3 {
            x: 0.,
            y: y_pos,
            z: 0.,
        },
        vel: Vec3 {
            x: 0.,
            y: y_vel,
            z: z_vel,
        },
    };
    if ticks == 0 {
        return state.total_energy();
    }
    let mut best_goodness = Goodness::NEG_INFINITY;
    for pitch in -90..=90 {
        let rot = Rot {
            x: pitch as Pitch,
            y: 0.,
        };
        let new_state = state.ticked(rot);
        let goodness = goodness_for_vel_y_after_ticks(
            new_state.vel.y,
            new_state.vel.z,
            new_state.pos.y,
            ticks - 1,
        );
        if goodness > best_goodness {
            best_goodness = goodness;
        }
    }
    best_goodness
}

pub struct DeepOptim {
    pub meta: GridMeta,
    pub pitches: Grid<Pitch>,
    pub states: Grid<State>,
    pub goodnesses: Grid<Goodness>,
}

impl DeepOptim {
    pub fn new(meta: GridMeta) -> Self {
        let old_states = Grid(
            (0..meta.height)
                .map(|row| {
                    (0..meta.width)
                        .map(|col| State {
                            pos: Vec3::ZERO,
                            vel: meta.row_col_usize_to_vel((row, col)),
                        })
                        .collect()
                })
                .collect(),
        );
        let old_goodnesses = Grid(
            old_states
                .0
                .iter()
                .map(|line| line.iter().map(|state| state.total_energy()).collect())
                .collect(),
        );
        // just use some default
        let old_pitches = Grid(
            old_states
                .0
                .iter()
                .map(|line| line.iter().map(|_| 0.).collect())
                .collect(),
        );
        // let (pitches, states, goodnesses) = Self::stepped(&meta, &old_states, &old_goodnesses);
        Self {
            meta,
            pitches: old_pitches,
            states: old_states,
            goodnesses: old_goodnesses,
        }
    }

    fn stepped(
        meta: &GridMeta,
        old_states: &Grid<State>,
        goodnesses: &Grid<Goodness>,
    ) -> (Grid<Pitch>, Grid<State>, Grid<Goodness>) {
        // fn goodness(state: &State) -> f64 {
        //     state.total_energy()
        // }

        // fn state_for_vel_pitch(
        //     meta: &GridMeta,
        //     old_states: &Grid<State>,
        //     vel: Vel3,
        //     pitch: Pitch,
        // ) -> Option<State> {
        //     let (row, col) = meta.vel_to_grid_row_col_float(vel);
        //     let state = old_states.state_bilinear_from_row_col_float((row, col))?;
        //     let rot = Rot { x: pitch, y: 0. };
        //     Some(state.ticked(rot))
        // }

        fn goodness_for_vel_pitch(
            meta: &GridMeta,
            goodnesses: &Grid<Goodness>,
            vel: Vel3,
            pitch: Pitch,
        ) -> Option<Goodness> {
            let rot = Rot { x: pitch, y: 0. };
            let init_state = State {
                pos: Vec3::ZERO,
                vel,
            };
            let new_state = init_state.ticked(rot);
            // let delta_goodness = new_state.total_energy() - init_state.total_energy();
            let (row, col) = meta.vel_to_grid_row_col_float(new_state.vel);
            goodnesses.f64_bilinear_from_row_col_float((row, col))
            // .map(|old_goodness| old_goodness + delta_goodness)
        }

        fn optimal_pitch_for_state(
            meta: &GridMeta,
            goodnesses: &Grid<Goodness>,
            init_state: &State,
        ) -> Pitch {
            let mut best_pitch = 0.;
            let mut best_goodness = f64::NEG_INFINITY;
            for pitch in -90..=90 {
                if let Some(goodness) =
                    goodness_for_vel_pitch(meta, goodnesses, init_state.vel, pitch as Pitch)
                    && goodness > best_goodness
                {
                    best_goodness = goodness;
                    best_pitch = pitch as Pitch;
                }
            }
            best_pitch
        }

        let new_pitches = Grid(
            old_states
                .0
                .iter()
                .enumerate()
                .map(|(row, line)| {
                    line.iter()
                        .enumerate()
                        .map(|(col, state)| optimal_pitch_for_state(meta, goodnesses, state))
                        .collect()
                })
                .collect(),
        );

        let new_states = Grid(
            new_pitches
                .0
                .iter()
                .zip(old_states.0.iter())
                .map(|(pitch_line, state_line)| {
                    pitch_line
                        .iter()
                        .zip(state_line.iter())
                        .map(|(pitch, state)| {
                            let rot = Rot { x: *pitch, y: 0. };
                            state.ticked(rot)
                        })
                        .collect()
                })
                .collect(),
        );

        let new_goodnesses = Grid(
            new_pitches
                .0
                .iter()
                .zip(old_states.0.iter())
                .map(|(pitch_line, state_line)| {
                    pitch_line
                        .iter()
                        .zip(state_line.iter())
                        .map(|(pitch, state)| {
                            goodness_for_vel_pitch(meta, goodnesses, state.vel, *pitch)
                                .unwrap_or(f64::NEG_INFINITY)
                        })
                        .collect()
                })
                .collect(),
        );

        (new_pitches, new_states, new_goodnesses)
    }

    pub fn step(&mut self) {
        let (new_pitches, new_states, new_goodnesses) =
            Self::stepped(&self.meta, &self.states, &self.goodnesses);
        self.pitches = new_pitches;
        self.states = new_states;
        self.goodnesses = new_goodnesses;
    }
}

// pub fn optimal_pitch_step_back(
//     meta: &GridMeta,
//     old_energies: &Grid<TotalEnergy>,
// ) -> (Grid<Pitch>, Grid<TotalEnergy>) {
//     // fn energy_for_vel_pitch(
//     //     meta: &GridMeta,
//     //     old_energies: &Grid<TotalEnergy>,
//     //     vel: Vel3,
//     //     pitch: Pitch,
//     // ) -> Option<TotalEnergy> {
//     //     let state = State {
//     //         pos: Vec3::ZERO,
//     //         vel,
//     //     };
//     //     let rot = Rot { x: pitch, y: 0. };
//     //     // let new_state = state.ticked(rot);
//     //     // TODO: this is goofy
//     //     let new_state = {
//     //         let mut new_state = state.clone();
//     //         for _ in 0..LOOKAHEAD.load(Ordering::Relaxed) {
//     //             new_state = new_state.ticked(rot)
//     //         }
//     //         new_state
//     //     };
//     //     let delta_energy = new_state.total_energy() - state.total_energy();
//     //     // bilinear interpolation
//     //     let old_energy = {
//     //         let (row, col) = meta.vel_to_grid_row_col_float(new_state.vel);
//     //         old_energies.f64_bilinear_from_row_col_float((row, col))
//     //     }?;
//     //     Some(old_energy + delta_energy)
//     //     // Some(old_energy + new_state.vel.y - state.vel.y)
//     // }
//     // fn optimal_pitch_for_vel(
//     //     meta: &GridMeta,
//     //     old_energies: &Grid<TotalEnergy>,
//     //     vel: Vel3,
//     // ) -> Pitch {
//     //     let mut best_pitch = 0.;
//     //     let mut best_delta_energy = f64::NEG_INFINITY;
//     //     for pitch in -90..=90 {
//     //         let delta_energy = energy_for_vel_pitch(meta, old_energies, vel, pitch as Pitch);
//     //         if let Some(delta_energy) = delta_energy
//     //             && delta_energy > best_delta_energy
//     //         {
//     //             best_delta_energy = delta_energy;
//     //             best_pitch = pitch as Pitch;
//     //         }
//     //     }
//     //     best_pitch
//     // }
//     let new_pitches = Grid(
//         (0..meta.height)
//             .map(|row| {
//                 (0..meta.width)
//                     .map(|col| {
//                         let vel = meta.row_col_usize_to_vel((row, col));
//                         optimal_pitch_for_vel(meta, old_energies, vel)
//                     })
//                     .collect()
//             })
//             .collect(),
//     );
//     let new_energies = Grid(
//         new_pitches
//             .0
//             .iter()
//             .enumerate()
//             .map(|(row, line)| {
//                 line.iter()
//                     .enumerate()
//                     .map(|(col, &pitch)| {
//                         let vel = meta.row_col_usize_to_vel((row, col));
//                         delta_total_energy_for_vel_at_pitch(vel, pitch)
//                     })
//                     .collect()
//             })
//             .collect(),
//     );
//     (new_pitches, new_energies)
// }

pub fn new_grid_immediate_optimal_pitch(meta: &GridMeta) -> (Grid<Pitch>, Grid<DeltaTotalEnergy>) {
    let pitches = Grid(
        (0..meta.height)
            .map(|row| {
                (0..meta.width)
                    .map(|col| {
                        let vel = meta.row_col_usize_to_vel((row, col));
                        argmax_over_pitch_of_delta_energy(vel)
                    })
                    .collect()
            })
            .collect(),
    );
    let energies = Grid(
        pitches
            .0
            .iter()
            .enumerate()
            .map(|(row, line)| {
                line.iter()
                    .enumerate()
                    .map(|(col, &pitch)| {
                        let vel = meta.row_col_usize_to_vel((row, col));
                        delta_total_energy_for_vel_at_pitch(vel, pitch)
                        // let state = State {
                        //     pos: Vec3::ZERO,
                        //     vel,
                        // };
                        // let new_state = state.ticked(Rot { x: pitch, y: 0. });
                        // new_state.vel.y - state.vel.y
                    })
                    .collect()
            })
            .collect(),
    );
    (pitches, energies)
}

fn delta_total_energy_for_vel_at_pitch(vel: Vel3, pitch: Pitch) -> DeltaTotalEnergy {
    let state = State {
        pos: Vec3::ZERO,
        vel,
    };
    let new_state = state.ticked(Rot { x: pitch, y: 0. });
    new_state.total_energy() - state.total_energy()
}

// TODO: longer time horizon
// TODO: flow field which the optimal path is following by definition
pub fn argmax_over_pitch_of_delta_energy(vel: Vel3) -> Pitch {
    let mut best_pitch = 0.;
    let mut best_delta_energy = f64::NEG_INFINITY;
    // for pitch in -40..=90 {
    for pitch in -90..=90 {
        let rot = Rot {
            x: pitch as f32,
            y: 0.,
        };
        let state = State {
            pos: Vec3::ZERO,
            vel,
        };
        let new_state = state.ticked(rot);
        // let new_state = {
        //     let mut new_state = state.clone();
        //     for _ in 0..LOOKAHEAD.load(Ordering::Relaxed) {
        //         new_state = new_state.ticked(rot)
        //     }
        //     new_state
        // };
        let delta_energy = new_state.total_energy() - state.total_energy();
        if delta_energy > best_delta_energy {
            best_delta_energy = delta_energy;
            best_pitch = pitch as f32;
        }
    }
    best_pitch
}

// fn optimal_delta_total_energy_for_vel(vel: Vel3) -> (Pitch, DeltaTotalEnergy) {}
