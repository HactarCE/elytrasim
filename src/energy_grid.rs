use std::sync::atomic::{AtomicUsize, Ordering};

use egui::ahash::AHashMap;
use itertools::Itertools;

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
        assert!(
            max_width_height > 0,
            "max_width_height must be positive, got {}",
            max_width_height
        );
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
        let width = width.max(1);
        let height = height.max(1);
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DPKey {
    pub y_pos: Pos,
    pub y_vel: Vel,
    pub z_vel: Vel,
}
impl DPKey {
    pub fn from_yz_vel(y_vel: Vel, z_vel: Vel) -> Self {
        const Y_POS_INIT: Pos = 0.;
        Self {
            y_pos: Y_POS_INIT,
            y_vel,
            z_vel,
        }
    }

    // pub fn to_representative(self) -> DPKeyRepresentative {
    //     DPKeyRepresentative(DPKey {
    //         y_pos: (self.y_pos / DPKeyRepresentative::Y_POS_STEP).round()
    //             * DPKeyRepresentative::Y_POS_STEP,
    //         y_vel: (self.y_vel / DPKeyRepresentative::Y_VEL_STEP).round()
    //             * DPKeyRepresentative::Y_VEL_STEP,
    //         z_vel: (self.z_vel / DPKeyRepresentative::Z_VEL_STEP).round()
    //             * DPKeyRepresentative::Z_VEL_STEP,
    //     })
    // }

    // pub fn trilinear_from_indexes(
    //     &self,
    //     (y_pos_i_f, y_vel_i_f, z_vel_i_f): (GridCoord, GridCoord, GridCoord),
    // ) -> Option<(Pitch, Goodness)> {
    //     let y_pos_i_lo = y_pos_i_f.floor() as usize;
    //     let y_vel_i_lo = y_vel_i_f.floor() as usize;
    //     let z_vel_i_lo = z_vel_i_f.floor() as usize;
    //     let y_pos_i_hi = y_pos_i_lo + 1;
    //     let y_vel_i_hi = y_vel_i_lo + 1;
    //     let z_vel_i_hi = z_vel_i_lo + 1;
    //     let y_pos_frac = y_pos_i_f - y_pos_i_lo as GridCoord;
    //     let y_vel_frac = y_vel_i_f - y_vel_i_lo as GridCoord;
    //     let z_vel_frac = z_vel_i_f - z_vel_i_lo as GridCoord;

    //     // get the 8 surrounding values
    //     let (p000, g000) = *self.arr.get(y_pos_i_lo)?.get(y_vel_i_lo)?.get(z_vel_i_lo)?;
    //     let (p001, g001) = *self.arr.get(y_pos_i_lo)?.get(y_vel_i_lo)?.get(z_vel_i_hi)?;
    //     let (p010, g010) = *self.arr.get(y_pos_i_lo)?.get(y_vel_i_hi)?.get(z_vel_i_lo)?;
    //     let (p011, g011) = *self.arr.get(y_pos_i_lo)?.get(y_vel_i_hi)?.get(z_vel_i_hi)?;
    //     let (p100, g100) = *self.arr.get(y_pos_i_hi)?.get(y_vel_i_lo)?.get(z_vel_i_lo)?;
    //     let (p101, g101) = *self.arr.get(y_pos_i_hi)?.get(y_vel_i_lo)?.get(z_vel_i_hi)?;
    //     let (p110, g110) = *self.arr.get(y_pos_i_hi)?.get(y_vel_i_hi)?.get(z_vel_i_lo)?;
    //     let (p111, g111) = *self.arr.get(y_pos_i_hi)?.get(y_vel_i_hi)?.get(z_vel_i_hi)?;

    //     // trilinear interpolation
    // }

    /// returns (lo, hi, frac)
    pub fn to_representatives(self) -> (DPKeyRepresentative, DPKeyRepresentative, [f64; 3]) {
        let y_pos_lo = (self.y_pos / DPKeyRepresentative::Y_POS_STEP).floor()
            * DPKeyRepresentative::Y_POS_STEP;
        let y_vel_lo = (self.y_vel / DPKeyRepresentative::Y_VEL_STEP).floor()
            * DPKeyRepresentative::Y_VEL_STEP;
        let z_vel_lo = (self.z_vel / DPKeyRepresentative::Z_VEL_STEP).floor()
            * DPKeyRepresentative::Z_VEL_STEP;

        let y_pos_hi = y_pos_lo + DPKeyRepresentative::Y_POS_STEP;
        let y_vel_hi = y_vel_lo + DPKeyRepresentative::Y_VEL_STEP;
        let z_vel_hi = z_vel_lo + DPKeyRepresentative::Z_VEL_STEP;

        let y_pos_frac = (self.y_pos - y_pos_lo) / DPKeyRepresentative::Y_POS_STEP;
        let y_vel_frac = (self.y_vel - y_vel_lo) / DPKeyRepresentative::Y_VEL_STEP;
        let z_vel_frac = (self.z_vel - z_vel_lo) / DPKeyRepresentative::Z_VEL_STEP;

        (
            DPKeyRepresentative(DPKey {
                y_pos: y_pos_lo,
                y_vel: y_vel_lo,
                z_vel: z_vel_lo,
            }),
            DPKeyRepresentative(DPKey {
                y_pos: y_pos_hi,
                y_vel: y_vel_hi,
                z_vel: z_vel_hi,
            }),
            [y_pos_frac, y_vel_frac, z_vel_frac],
        )
    }
    pub fn to_state(self) -> State {
        State {
            pos: Vec3 {
                x: 0.,
                y: self.y_pos,
                z: 0.,
            },
            vel: Vec3 {
                x: 0.,
                y: self.y_vel,
                z: self.z_vel,
            },
        }
    }

    pub fn from_state(state: &State) -> Self {
        let &State {
            pos:
                Pos3 {
                    x: x_pos,
                    y: y_pos,
                    z: z_pos,
                },
            vel:
                Vel3 {
                    x: x_vel,
                    y: y_vel,
                    z: z_vel,
                },
        } = state;
        assert_eq!(x_pos, 0.);
        // assert_eq!(z_pos, 0.);
        assert_eq!(x_vel, 0.);
        Self {
            y_pos,
            y_vel,
            z_vel,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DPKeyRepresentative(pub DPKey);
impl DPKeyRepresentative {
    // TODO: adaptive stuff
    const Y_POS_STEP: f64 = 0.5;
    const Z_VEL_STEP: f64 = 0.1;
    const Y_VEL_STEP: f64 = 0.1;

    fn to_array(self) -> [u64; 3] {
        let arr_f = [self.0.y_pos, self.0.y_vel, self.0.z_vel];
        arr_f.map(|f| f.to_bits())
    }
}
impl std::cmp::PartialEq for DPKeyRepresentative {
    fn eq(&self, other: &Self) -> bool {
        self.to_array() == other.to_array()
    }
}
impl std::cmp::Eq for DPKeyRepresentative {}
impl std::hash::Hash for DPKeyRepresentative {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        let arr_u = self.to_array();
        arr_u.hash(hasher);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DPValue {
    /// `None` for tick 0
    pub pitch: Option<Pitch>,
    pub goodness: Goodness,
}
impl DPValue {
    fn lerp(v0: DPValue, v1: DPValue, frac: f64) -> Self {
        assert_eq!(v0.pitch.is_none(), v1.pitch.is_none());
        let pitch = match (v0.pitch, v1.pitch) {
            (None, None) => None,
            (Some(p0), Some(p1)) => Some(lerp_f64(p0 as f64, p1 as f64, frac) as Pitch),
            _ => unreachable!(),
        };
        Self {
            pitch,
            goodness: lerp_f64(v0.goodness, v1.goodness, frac),
        }
    }
}

#[derive(Debug, Clone)]
// struct GoodnessAtTick(pub KdTree<f64, DPValue, DPKeyInner>);
pub struct GoodnessAtTick(pub AHashMap<DPKeyRepresentative, DPValue>);
impl GoodnessAtTick {
    fn empty() -> Self {
        Self(AHashMap::new())
    }

    /// `Err` contains the representatives we need to compute to get the value for this key.
    fn get_trilinear(&self, key: DPKey) -> Result<DPValue, Vec<DPKeyRepresentative>> {
        let (
            DPKeyRepresentative(DPKey {
                y_pos: y_pos_lo,
                y_vel: y_vel_lo,
                z_vel: z_vel_lo,
            }),
            DPKeyRepresentative(DPKey {
                y_pos: y_pos_hi,
                y_vel: y_vel_hi,
                z_vel: z_vel_hi,
            }),
            [y_pos_frac, y_vel_frac, z_vel_frac],
        ) = key.to_representatives();

        #[expect(unused_variables)]
        let key = ();

        const RANGE: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
        // let [k000, k001, k010, k011, k100, k101, k110, k111]
        let keys = RANGE.map(|i| {
            let y_pos = if i & 4 == 0 { y_pos_lo } else { y_pos_hi };
            let y_vel = if i & 2 == 0 { y_vel_lo } else { y_vel_hi };
            let z_vel = if i & 1 == 0 { z_vel_lo } else { z_vel_hi };
            DPKeyRepresentative(DPKey {
                y_pos,
                y_vel,
                z_vel,
            })
        });

        let values = keys.map(|key| self.0.get(&key).copied());

        let [
            Some(v000),
            Some(v001),
            Some(v010),
            Some(v011),
            Some(v100),
            Some(v101),
            Some(v110),
            Some(v111),
        ] = values
        else {
            // note that collect has funny behavior
            // return Err(keys.to_vec());
            return Err(keys
                .iter()
                .zip(values.iter())
                .filter_map(|(key, value)| match value {
                    Some(_value) => None,
                    None => Some(*key),
                })
                .collect());
        };

        let v00 = DPValue::lerp(v000, v001, z_vel_frac);
        let v01 = DPValue::lerp(v010, v011, z_vel_frac);
        let v10 = DPValue::lerp(v100, v101, z_vel_frac);
        let v11 = DPValue::lerp(v110, v111, z_vel_frac);

        let v0 = DPValue::lerp(v00, v01, y_vel_frac);
        let v1 = DPValue::lerp(v10, v11, y_vel_frac);

        let v = DPValue::lerp(v0, v1, y_pos_frac);

        Ok(v)
    }
}

#[derive(Debug, Clone)]
pub struct DP {
    // meta: DPMeta,
    // arr: Box<[Box<[Box<[(Pitch, Goodness)]>]>]>,
    /// indexed by tick
    pub caches: Vec<GoodnessAtTick>,
    // goodnesses: Vec<AHashMap<DPKeyRepresentative, Goodness>>,
    // pitches: Vec<AHashMap<DPKeyRepresentative, Pitch>>,
}
impl DP {
    pub fn empty() -> Self {
        Self { caches: vec![] }
    }

    // /// gradient of pitch wrt goodness.
    // fn grad(&mut self, tick: usize, key_query: DPKey) -> f64 {}

    /// applies `tick` pitches to key_query.
    /// so if tick == 0, returns the same state.
    /// this is rather than just getting the cached goodness.
    pub fn cycle(&mut self, tick: usize, state: &State) -> State {
        if tick == 0 {
            return state.clone();
        }
        let DPValue { pitch, goodness: _ } = self.get(tick, DPKey::from_state(&state));
        let pitch = pitch.expect("pitch should be Some for tick > 0");
        self.cycle(tick - 1, &state.ticked(Rot { x: pitch, y: 0. }))
    }

    /// the largest goodness we can obtain starting from key_query and ticking for tick ticks.
    /// the pitch is the pitch we should apply now to get that goodness.
    // TODO: don't store y, infer potential_energy from delta y? just cache pitches and apply O(n) of those?
    // TODO: don't search things outside a window
    pub fn get(&mut self, tick: usize, key_query: DPKey) -> DPValue {
        while self.caches.len() <= tick {
            self.caches.push(GoodnessAtTick::empty());
        }

        // if the key has all its representatives in the cache,
        // return the interpolation of those values.
        let representatives = match self.caches[tick].get_trilinear(key_query) {
            Ok(value) => return value,
            Err(representatives) => representatives,
        };

        // if tick == 0, compute and insert and return the base case.
        if tick == 0 {
            for key_representative in representatives {
                let exact_goodness = key_representative.0.to_state().total_energy();
                let exact_value = DPValue {
                    pitch: None,
                    goodness: exact_goodness,
                };
                self.caches[tick]
                    .0
                    .insert(key_representative, exact_value)
                    .ok_or(())
                    .unwrap_err();
            }
            return self.caches[tick].get_trilinear(key_query).unwrap();
        }

        // else, compute and insert and return argmax over pitch of get(tick - 1, ticked_key).
        for key_representative in representatives {
            let init_state = key_representative.0.to_state();
            let mut best_value = DPValue {
                pitch: None,
                goodness: f64::NEG_INFINITY,
            };
            for pitch in (-90..=90).step_by(3) {
                // TODO: gradient descent
                let rot = Rot {
                    x: pitch as Pitch,
                    y: 0.,
                };
                let ticked_state = init_state.ticked(rot);
                let ticked_key = DPKey::from_state(&ticked_state);
                let goodness = self.get(tick - 1, ticked_key).goodness;
                if goodness > best_value.goodness {
                    best_value = DPValue {
                        pitch: Some(pitch as Pitch),
                        goodness,
                    };
                }
            }
            self.caches[tick]
                .0
                .insert(key_representative, best_value)
                .ok_or(())
                .unwrap_err();
        }
        self.caches[tick].get_trilinear(key_query).unwrap()
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

pub fn argmax_over_pitch_of_energy(vel: Vel3) -> Pitch {
    let mut best_pitch = 0.;
    let mut best_energy = f64::NEG_INFINITY;
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
        let energy = new_state.total_energy();
        if energy > best_energy {
            best_energy = energy;
            best_pitch = pitch as f32;
        }
    }
    best_pitch
}

// fn optimal_delta_total_energy_for_vel(vel: Vel3) -> (Pitch, DeltaTotalEnergy) {}
