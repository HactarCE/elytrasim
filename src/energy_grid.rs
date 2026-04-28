use super::*;

type GridCoord = f32;
// type Grid<T> = Box<[Box<[T]>]>;
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
        self.row_col_float_to_egui_pos2((row as GridCoord + 0.5, col as GridCoord + 0.5), rect)
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

pub fn new_grid_optimal_pitch(meta: &GridMeta) -> (Grid<Pitch>, Grid<DeltaTotalEnergy>) {
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
