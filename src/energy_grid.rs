use super::*;

type GridCoord = f32;

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
                    })
                    .collect()
            })
            .collect(),
    );
    (pitches, energies)
}

pub fn optimal_pitch_step_back(
    meta: &GridMeta,
    old_energies: &Grid<DeltaTotalEnergy>,
) -> (Grid<Pitch>, Grid<DeltaTotalEnergy>) {
    fn energy_for_vel_pitch(
        meta: &GridMeta,
        old_energies: &Grid<DeltaTotalEnergy>,
        vel: Vel3,
        pitch: Pitch,
    ) -> Option<DeltaTotalEnergy> {
        let state = State {
            pos: Vec3::ZERO,
            vel,
        };
        let new_state = state.ticked(Rot { x: pitch, y: 0. });
        let delta_energy = new_state.total_energy() - state.total_energy();
        // bilinear interpolation
        let old_energy = {
            let (row, col) = meta.vel_to_grid_row_col_float(vel);
            old_energies.f64_bilinear_from_row_col_float((row, col))
        }?;
        Some(old_energy + delta_energy)
    }
    fn optimal_pitch_for_vel(
        meta: &GridMeta,
        old_energies: &Grid<DeltaTotalEnergy>,
        vel: Vel3,
    ) -> Pitch {
        let mut best_pitch = 0.;
        let mut best_delta_energy = f64::NEG_INFINITY;
        for pitch in -90..=90 {
            let delta_energy = energy_for_vel_pitch(meta, old_energies, vel, pitch as Pitch);
            if let Some(delta_energy) = delta_energy
                && delta_energy > best_delta_energy
            {
                best_delta_energy = delta_energy;
                best_pitch = pitch as Pitch;
            }
        }
        best_pitch
    }
    let new_pitches = Grid(
        (0..meta.height)
            .map(|row| {
                (0..meta.width)
                    .map(|col| {
                        let vel = meta.row_col_usize_to_vel((row, col));
                        optimal_pitch_for_vel(meta, old_energies, vel)
                    })
                    .collect()
            })
            .collect(),
    );
    let new_energies = Grid(
        new_pitches
            .0
            .iter()
            .enumerate()
            .map(|(row, line)| {
                line.iter()
                    .enumerate()
                    .map(|(col, &pitch)| {
                        let vel = meta.row_col_usize_to_vel((row, col));
                        delta_total_energy_for_vel_at_pitch(vel, pitch)
                    })
                    .collect()
            })
            .collect(),
    );
    (new_pitches, new_energies)
}

struct Optim {
    meta: GridMeta,
    pitches: Grid<Pitch>,
    delta_energies: Grid<DeltaTotalEnergy>,
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
        let delta_energy = new_state.total_energy() - state.total_energy();
        if delta_energy > best_delta_energy {
            best_delta_energy = delta_energy;
            best_pitch = pitch as f32;
        }
    }
    best_pitch
}

// fn optimal_delta_total_energy_for_vel(vel: Vel3) -> (Pitch, DeltaTotalEnergy) {}
