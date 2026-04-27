use super::*;

type GridCoord = f32;
// type Grid<T> = Box<[Box<[T]>]>;
struct Grid<T>(Box<[Box<[T]>]>);

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
        width: usize,
        y_vel_mid: Vel,
        z_vel_lo: Vel,
        z_vel_hi: Vel,
        rect: egui::Rect,
    ) -> Self {
        let scale = rect.height() / rect.width();
        let delta_z_vel = z_vel_hi - z_vel_lo;
        let delta_y_vel = delta_z_vel * scale as Vel;
        Self {
            width,
            height: (width as f32 * scale).round() as usize,
            y_vel_lo: y_vel_mid - delta_y_vel / 2.,
            y_vel_hi: y_vel_mid + delta_y_vel / 2.,
            z_vel_lo,
            z_vel_hi,
        }
    }

    fn debug_is_uniform(&self) -> bool {
        let delta_z_vel = self.z_vel_hi - self.z_vel_lo;
        let delta_y_vel = self.y_vel_hi - self.y_vel_lo;
        let scale = delta_y_vel / delta_z_vel;
        let expected_height = (self.width as f64 * scale).round() as usize;
        (self.height as isize - expected_height as isize).abs() <= 1
    }

    /// returns an egui float.
    pub fn step(&self) -> f32 {
        // assert!()
        let horizontal_step = self.width as f32 / (self.z_vel_hi - self.z_vel_lo) as f32;
        let vertical_step = self.height as f32 / (self.y_vel_hi - self.y_vel_lo) as f32;
        assert!(
            self.debug_is_uniform(),
            "step is only well defined for uniform grids, horizontal_step: {}, vertical_step: {}",
            horizontal_step,
            vertical_step
        );
        // horizontal_step
        horizontal_step.min(vertical_step)
    }

    pub fn vel_to_grid_row_col_float(&self, vel: Vel3) -> (GridCoord, GridCoord) {
        assert_eq!(vel.x, 0., "not a hard error, but probably should have this");
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

    // pub fn vel_to_grid_row_col_usize(&self, vel: Vel3) -> (usize, usize) {
    //     let (row, col) = self.vel_to_grid_row_col_float(vel);
    //     // TODO: should this round or floor or what?
    //     (row as usize, col as usize)
    // }

    // pub fn vel_to_grid_col_row(&self, vel: Vel3) -> (GridCoord, GridCoord) {
    //     let (row, col) = self.vel_to_grid_row_col(vel);
    //     (col, row)
    // }

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
        self.row_col_float_to_vel((row as GridCoord + 0.5, col as GridCoord + 0.5))
    }

    pub fn vel_to_egui_pos2(&self, vel: Vel3, rect: egui::Rect) -> egui::Pos2 {
        let (row, col) = self.vel_to_grid_row_col_float(vel);
        rect.left_top() + egui::vec2(col, row) * self.step()
    }

    pub fn row_col_float_to_egui_pos2(
        &self,
        (row, col): (GridCoord, GridCoord),
        rect: egui::Rect,
    ) -> egui::Pos2 {
        rect.left_top() + egui::vec2(col, row) * self.step()
    }

    /// from the center of the cell
    pub fn row_col_usize_to_egui_pos2(
        &self,
        (row, col): (usize, usize),
        rect: egui::Rect,
    ) -> egui::Pos2 {
        self.row_col_float_to_egui_pos2((row as GridCoord + 0.5, col as GridCoord + 0.5), rect)
    }

    pub fn rects(
        &self,
        rect: egui::Rect,
    ) -> impl Iterator<Item = impl Iterator<Item = egui::Rect>> {
        (0..self.width).map(move |col| {
            (0..self.height).map(move |row| {
                let cell_width = rect.width() / self.width as f32;
                let cell_height = rect.height() / self.height as f32;
                egui::Rect::from_min_size(
                    egui::pos2(
                        rect.left() + col as f32 * cell_width,
                        rect.top() + row as f32 * cell_height,
                    ),
                    egui::vec2(cell_width, cell_height),
                )
            })
        })
    }
}

impl Grid<DeltaTotalEnergy> {
    fn from_fixed_pitch(meta: &GridMeta, pitch: Pitch) -> Self {
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

    fn from_optimal_pitch(meta: &GridMeta) -> Self {
        Self(
            (0..meta.height)
                .map(|row| {
                    (0..meta.width)
                        .map(|col| {
                            let vel = meta.row_col_usize_to_vel((row, col));
                            let optimal_pitch = argmax_over_pitch_of_delta_energy(vel);
                            delta_total_energy_for_vel_at_pitch(vel, optimal_pitch)
                        })
                        .collect()
                })
                .collect(),
        )
    }
}

struct Optim {
    meta: GridMeta,
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
