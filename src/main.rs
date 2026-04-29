mod energy_grid;
mod replay_pitches;
mod sim;

use crate::energy_grid::*;
use crate::sim::*;

pub const TICK_DURATION: std::time::Duration = std::time::Duration::from_millis(50); // 20 per second
// const REPLAY_PITCHES: &[f32] = replay_pitches::FORTY_FORTY;
// const REPLAY_PITCHES: &[f32] = replay_pitches::FORTY_ZERO_FORTY;
// const REPLAY_PITCHES: &[f32] = replay_pitches::FOUR_LINES_300;
// const REPLAY_PITCHES: &[f32] = replay_pitches::REPLAY_PITCHES_200;
const REPLAY_PITCHES: &[f32] = replay_pitches::REPLAY_PITCHES_300;
// const REPLAY_PITCHES: &[f32] = replay_pitches::REPLAY_PITCHES_400;

const PINK: egui::Color32 = egui::Color32::from_rgb(252, 3, 198);

fn main() -> eframe::Result {
    #[cfg(false)]
    {
        let vel = Vec3 {
            x: 0.,
            y: -0.6,
            z: 2.4,
        };
        let state = State {
            pos: Vec3::ZERO,
            vel,
        };

        let next_plus = state.ticked(Rot { x: 1., y: 0. });
        println!("plus: {:#?}", next_plus.sub(&state));

        let next_zero = state.ticked(Rot { x: 0., y: 0. });
        println!("zero: {:#?}", next_zero.sub(&state));

        let next_minus = state.ticked(Rot { x: -1., y: 0. });
        println!("minus: {:#?}", next_minus.sub(&state));

        panic!();
    }

    let mut mag_scale = 0.04;
    let mut arrow_scale = 0.6;
    let mut arrow_thickness = 3.;

    let mut fixed_rot = Rot::new(0., 0.);

    let mut draw_arrow_type = DrawArrowType::DeepOptimalPitch;

    const Y_VEL_MID: f64 = 0.;
    const Z_VEL_LO: f64 = 0.;
    let mut z_vel_hi = 5.;
    // let mut grid_width = 100;
    // coprime with the to_representative multiples
    // to avoid drawing the cache as though it's perfect
    let mut grid_width = 93;

    let mut grid_meta = GridMeta::new_uniform(
        grid_width,
        Y_VEL_MID,
        Z_VEL_LO,
        z_vel_hi,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000., 1000.)),
    );
    let mut fixed_pitch_energies =
        Grid::<DeltaTotalEnergy>::from_fixed_pitch(&grid_meta, fixed_rot.x);
    let (mut immediate_optimal_pitches, mut immediate_optimal_energies) =
        energy_grid::new_grid_immediate_optimal_pitch(&grid_meta);
    // let (mut deep_optimal_pitches, mut deep_optimal_energies) =
    //     energy_grid::new_grid_immediate_optimal_pitch(&grid_meta);
    // let mut deep_optim = DeepOptim::new(grid_meta.clone());

    // let mut dp = DP::new_base(DPMeta::from_grid_meta(&grid_meta, -100.0, 30.0, 40));
    let mut dp = DP::empty();
    let mut dp_tick: usize = 0;
    // let mut deep_optimizer_running = false;
    // let mut deep_optimizer_steps_per_frame = 1;

    let mut clicked_state = None;
    // let mut hovered_vel = Vel3::ZERO;
    let mut hovered_state = State {
        pos: Pos3::ZERO,
        vel: Vel3::ZERO,
    };

    let mut state_index: usize = 0;
    let replay_states = {
        let mut replay_states = vec![State {
            pos: Pos3::ZERO,
            vel: Vel3 {
                x: 0.,
                y: 0.167467,
                z: 0.200887,
            },
        }];
        for p in REPLAY_PITCHES {
            let state = replay_states.last().expect("wtf");
            replay_states.push(state.ticked(Rot { x: *p, y: 0. }));
        }
        replay_states
    };

    eframe::run_ui_native(
        "Elytra Sim",
        eframe::NativeOptions::default(),
        move |ui, _frame| {
            ui.ctx().request_repaint();
            egui::Panel::left("left")
                .resizable(false)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                        ui.group(|ui| {
                            ui.label("Grid Width");
                            ui.add(
                                egui::Slider::new(&mut grid_width, 1..=500)
                                    .clamping(egui::SliderClamping::Never),
                            );
                            grid_width = grid_width.max(1);

                            ui.label("Max Z Vel");
                            ui.add(
                                egui::Slider::new(&mut z_vel_hi, 0.0..=8.0)
                                    .clamping(egui::SliderClamping::Never),
                            );

                            ui.label("Mag Scale");
                            ui.add(
                                egui::Slider::new(&mut mag_scale, 0.001..=100.0)
                                    .clamping(egui::SliderClamping::Never)
                                    .logarithmic(true),
                            );

                            ui.label("Arrow Scale");
                            ui.add(
                                egui::Slider::new(&mut arrow_scale, 0.0..=2.0)
                                    .clamping(egui::SliderClamping::Never),
                            );

                            ui.label("Arrow Thickness");
                            ui.add(
                                egui::Slider::new(&mut arrow_thickness, 0.0..=5.0)
                                    .clamping(egui::SliderClamping::Never),
                            );

                            // draw_arrow_type
                            ui.label("Draw Arrow Type");
                            egui::ComboBox::from_id_salt("Draw Arrow Type")
                                .selected_text(match draw_arrow_type {
                                    DrawArrowType::FixedDeltaVel => "Global Pitch",
                                    DrawArrowType::ImmediateOptimalPitch => {
                                        "Immediate Optimal Pitch"
                                    }
                                    DrawArrowType::ImmediateOptimalDeltaVel => {
                                        "Immediate Optimal Delta Vel"
                                    }
                                    DrawArrowType::DeepOptimalPitch => "Deep Optimal Pitch",
                                    DrawArrowType::DeepOptimalDeltaVel => "Deep Optimal Delta Vel",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut draw_arrow_type,
                                        DrawArrowType::FixedDeltaVel,
                                        "Global Pitch",
                                    );
                                    ui.selectable_value(
                                        &mut draw_arrow_type,
                                        DrawArrowType::ImmediateOptimalPitch,
                                        "Immediate Optimal Pitch",
                                    );
                                    ui.selectable_value(
                                        &mut draw_arrow_type,
                                        DrawArrowType::ImmediateOptimalDeltaVel,
                                        "Immediate Optimal Delta Vel",
                                    );
                                    ui.selectable_value(
                                        &mut draw_arrow_type,
                                        DrawArrowType::DeepOptimalPitch,
                                        "Deep Optimal Pitch",
                                    );
                                    ui.selectable_value(
                                        &mut draw_arrow_type,
                                        DrawArrowType::DeepOptimalDeltaVel,
                                        "Deep Optimal Delta Vel",
                                    );
                                });
                        });
                        ui.group(|ui| {
                            ui.strong("Rotation");
                            ui.label("Pitch");

                            ui.add(
                                egui::Slider::new(&mut fixed_rot.x, -90.0..=90.0)
                                    .clamping(egui::SliderClamping::Never),
                            );
                            ui.label("Yaw");
                            ui.add(
                                egui::Slider::new(&mut fixed_rot.y, -90.0..=90.0)
                                    .clamping(egui::SliderClamping::Never),
                            );
                            ui.allocate_ui(egui::vec2(50., 50.), |ui| {
                                ui.painter().arrow(
                                    ui.available_rect_before_wrap().left_top(),
                                    40. * egui::Vec2::angled(
                                        fixed_rot.x * std::f32::consts::PI / 180.,
                                    ),
                                    (3., PINK),
                                );
                            });
                        });
                        ui.group(|ui| {
                            // let mut do_step_back = || {
                            //     let (new_deep_optimal_pitches, new_deep_optimal_energies) =
                            //         energy_grid::optimal_pitch_step_back(
                            //             &grid_meta,
                            //             &deep_optimal_energies,
                            //         );
                            //     deep_optimal_pitches = new_deep_optimal_pitches;
                            //     deep_optimal_energies = new_deep_optimal_energies;
                            // };

                            // if ui.button("Step Back").clicked() {
                            //     // deep_optim.step();
                            //     // dp.step();
                            //     dp_tick += 1;
                            // }

                            // ui.checkbox(&mut deep_optimizer_running, "Deep Optimizer Running");

                            // ui.label("Deep Optimizer Steps Per Frame");
                            // ui.add(
                            //     egui::Slider::new(&mut deep_optimizer_steps_per_frame, 1..=1000)
                            //         .logarithmic(true)
                            //         .clamping(egui::SliderClamping::Never),
                            // );

                            // if deep_optimizer_running {
                            //     for _ in 0..deep_optimizer_steps_per_frame {
                            //         // deep_optim.step();
                            //         dp.step();
                            //     }
                            // }

                            // {
                            //     let mut lookahead = LOOKAHEAD.load(Ordering::Relaxed);
                            //     ui.label("Lookahead");
                            //     if ui
                            //         .add(
                            //             egui::Slider::new(&mut lookahead, 1..=20)
                            //                 .clamping(egui::SliderClamping::Never),
                            //         )
                            //         .changed()
                            //     {
                            //         LOOKAHEAD.store(lookahead, Ordering::Relaxed);
                            //     }
                            // }
                            ui.label("DP Tick");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Slider::new(&mut dp_tick, 0..=100)
                                        .clamping(egui::SliderClamping::Never),
                                );
                                if ui.button("-").clicked() {
                                    dp_tick = dp_tick.saturating_sub(1);
                                }
                                if ui.button("+").clicked() {
                                    dp_tick += 1;
                                }
                            });
                        });
                        ui.group(|ui| {
                            ui.label("Replay Progress");
                            ui.horizontal(|ui| {
                                let mut changed = ui
                                    .add(egui::Slider::new(
                                        &mut state_index,
                                        // TODO: should this be 0..=replay_states.len() - 2,
                                        0..=replay_states.len() - 1,
                                    ))
                                    .changed();
                                if ui.button("-").clicked() {
                                    state_index = state_index.saturating_sub(1);
                                    changed = true;
                                }
                                if ui.button("+").clicked() {
                                    state_index =
                                        std::cmp::min(state_index + 1, replay_states.len() - 1);
                                    changed = true;
                                }
                                if changed {
                                    fixed_rot.x =
                                        REPLAY_PITCHES[state_index % REPLAY_PITCHES.len()];
                                }
                            });
                        });
                        ui.group(|ui| {
                            // let (row, col) = grid_meta.vel_to_grid_row_col_float(hovered_state.vel);
                            // let hovered_state = State {
                            //     pos: Vec3::ZERO,
                            //     vel: hovered_vel,
                            // };

                            ui.group(|ui| {
                                ui.label(format!("y vel: {:.09?} bpt", hovered_state.vel.y));
                                ui.label(format!("z vel: {:.09?} bpt", hovered_state.vel.z));
                            });

                            ui.group(|ui| {
                                let new_state = hovered_state.ticked(fixed_rot);
                                let delta_vel = new_state.vel - hovered_state.vel;
                                let delta_kinetic =
                                    new_state.kinetic_energy() - hovered_state.kinetic_energy();
                                let delta_potential =
                                    new_state.potential_energy() - hovered_state.potential_energy();
                                let delta_energy =
                                    new_state.total_energy() - hovered_state.total_energy();
                                ui.label(format!("fixed pitch: {:.09?} deg", fixed_rot.x));
                                ui.label(format!("|dv|: {:.09?}", delta_vel.length()));
                                ui.label(format!("dk: {:.09?}", delta_kinetic));
                                ui.label(format!("dp: {:.09?}", delta_potential));
                                ui.label(format!("de: {:.09?}", delta_energy));
                            });

                            #[cfg(false)]
                            ui.group(|ui| {
                                // stuff for argmax_{pitch} (delta_energy) oracle
                                let pitch = argmax_over_pitch_of_delta_energy(init_state.vel);
                                let optimal_new_state = init_state.ticked(Rot { x: pitch, y: 0. });
                                let optimal_delta_vel = optimal_new_state.vel - init_state.vel;
                                let optimal_delta_kinetic = optimal_new_state.kinetic_energy()
                                    - init_state.kinetic_energy();
                                let optimal_delta_potential = optimal_new_state.potential_energy()
                                    - init_state.potential_energy();
                                let optimal_delta_energy =
                                    optimal_new_state.total_energy() - init_state.total_energy();
                                ui.label("immediate optimizer");
                                ui.label(format!("pitch: {:.09?} deg", pitch));
                                ui.label(format!("|dv|: {:.09?}", optimal_delta_vel.length()));
                                ui.label(format!("dk: {:.09?}", optimal_delta_kinetic));
                                ui.label(format!("dp: {:.09?}", optimal_delta_potential));
                                ui.label(format!("de: {:.09?}", optimal_delta_energy));
                            });

                            ui.group(|ui| {
                                // stuff for argmax_{pitch} (delta_energy)
                                // but with formatting consistent with the deep optimizer
                                let pitch = argmax_over_pitch_of_delta_energy(hovered_state.vel);
                                let optimal_new_state =
                                    hovered_state.ticked(Rot { x: pitch, y: 0. });
                                ui.label("immediate delta energy");
                                ui.label(format!("pitch: {:.09?} deg", pitch));
                                ui.label(format!(
                                    "initial energy: {:.09?}",
                                    hovered_state.total_energy()
                                ));
                                ui.label(format!(
                                    "final energy: {:.09?}",
                                    optimal_new_state.total_energy()
                                ));
                                ui.label(format!(
                                    "delta energy: {:.09?}",
                                    optimal_new_state.total_energy() - hovered_state.total_energy()
                                ));
                            });

                            ui.group(|ui| {
                                // stuff for argmax_{pitch} (energy)
                                // but with formatting consistent with the deep optimizer
                                let pitch = argmax_over_pitch_of_energy(hovered_state.vel);
                                let optimal_new_state =
                                    hovered_state.ticked(Rot { x: pitch, y: 0. });
                                ui.label("immediate energy");
                                ui.label(format!("pitch: {:.09?} deg", pitch));
                                ui.label(format!(
                                    "initial energy: {:.09?}",
                                    hovered_state.total_energy()
                                ));
                                ui.label(format!(
                                    "final energy: {:.09?}",
                                    optimal_new_state.total_energy()
                                ));
                                ui.label(format!(
                                    "delta energy: {:.09?}",
                                    optimal_new_state.total_energy() - hovered_state.total_energy()
                                ));
                            });

                            #[cfg(false)]
                            ui.group(|ui| {
                                // stuff for argmax_{pitch} (energy) from the representative
                                let key_representative =
                                    DPKey::from_state(&hovered_state).to_representative();
                                let init_state = key_representative.0.to_state();
                                let pitch = argmax_over_pitch_of_energy(init_state.vel);
                                let optimal_new_state = init_state.ticked(Rot { x: pitch, y: 0. });
                                ui.label("immediate energy representative");
                                ui.label(format!("pitch: {:.09?} deg", pitch));
                                ui.label(format!(
                                    "initial energy: {:.09?}",
                                    init_state.total_energy()
                                ));
                                ui.label(format!(
                                    "final energy: {:.09?}",
                                    optimal_new_state.total_energy()
                                ));
                                ui.label(format!(
                                    "delta energy: {:.09?}",
                                    optimal_new_state.total_energy() - init_state.total_energy()
                                ));
                            });

                            // representative of the hovered vel
                            #[cfg(false)]
                            ui.group(|ui| {
                                let key_representative =
                                    DPKey::from_state(&hovered_state).to_representative();
                                ui.label("dp representative");
                                ui.label(format!("y_pos: {:.09?} b", key_representative.0.y_pos));
                                ui.label(format!("y_vel: {:.09?} bpt", key_representative.0.y_vel));
                                ui.label(format!("z_vel: {:.09?} bpt", key_representative.0.z_vel));
                            });

                            ui.group(|ui| {
                                let key_query = DPKey::from_state(&hovered_state);
                                let DPValue { pitch, goodness } = dp.get(dp_tick, key_query);
                                ui.label("dp hovered");

                                ui.label(format!("pitch: {:.09?} deg", pitch.unwrap_or(f32::NAN)));
                                ui.label(format!(
                                    "initial goodness: {:.09?}",
                                    hovered_state.total_energy()
                                ));
                                ui.label(format!("final goodness: {:.09?}", goodness));
                                ui.label(format!(
                                    "delta goodness: {:.09?}",
                                    goodness - hovered_state.total_energy()
                                ));
                                if let Some(pitch) = pitch {
                                    let new_state = hovered_state.ticked(Rot { x: pitch, y: 0. });
                                    // this should match, or maybe only from the representative
                                    ui.label(format!(
                                        "immediate delta goodness: {:.09?}",
                                        new_state.total_energy() - hovered_state.total_energy()
                                    ));
                                }
                            });

                            #[cfg(false)]
                            ui.group(|ui| {
                                let key_representative =
                                    DPKey::from_state(&hovered_state).to_representative();
                                let init_state = key_representative.0.to_state();
                                let DPValue { pitch, goodness } =
                                    dp.get(dp_tick, key_representative.0);

                                ui.label("dp representative");
                                ui.label(format!("pitch: {:.09?} deg", pitch.unwrap_or(f32::NAN)));
                                ui.label(format!(
                                    "initial goodness: {:.09?}",
                                    init_state.total_energy()
                                ));
                                ui.label(format!("final goodness: {:.09?}", goodness));
                                ui.label(format!(
                                    "delta goodness: {:.09?}",
                                    goodness - init_state.total_energy()
                                ));
                                if let Some(pitch) = pitch {
                                    let new_state = init_state.ticked(Rot { x: pitch, y: 0. });
                                    // this should match, or maybe only from the representative
                                    ui.label(format!(
                                        "immediate delta goodness: {:.09?}",
                                        new_state.total_energy() - init_state.total_energy()
                                    ));
                                }
                            });
                        });
                    })
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                let rect = ui.available_rect_before_wrap();

                // update grid meta and resize grids if changed
                {
                    let new_grid_meta =
                        GridMeta::new_uniform(grid_width, Y_VEL_MID, Z_VEL_LO, z_vel_hi, rect);
                    if new_grid_meta != grid_meta {
                        grid_meta = new_grid_meta;
                        fixed_pitch_energies =
                            Grid::<DeltaTotalEnergy>::from_fixed_pitch(&grid_meta, fixed_rot.x);
                        (immediate_optimal_pitches, immediate_optimal_energies) =
                            energy_grid::new_grid_immediate_optimal_pitch(&grid_meta);
                        // (deep_optimal_pitches, deep_optimal_energies) =
                        //     energy_grid::new_grid_immediate_optimal_pitch(&grid_meta);
                        // deep_optim = DeepOptim::new(grid_meta.clone());
                        // dp = DP::new_base(DPMeta::from_grid_meta(&grid_meta, -100.0, 30.0, 40));
                    }
                }

                // update hover_state and clicked_state
                if let Some(hovered_egui_pos2) = ui.input(|i| i.pointer.latest_pos())
                    && rect.contains(hovered_egui_pos2)
                {
                    hovered_state.vel = grid_meta.egui_pos2_to_vel(hovered_egui_pos2, rect);
                    if ui.input(|i| i.pointer.primary_clicked()) {
                        clicked_state = if clicked_state.as_ref() == Some(&hovered_state) {
                            None
                        } else {
                            Some(hovered_state.clone())
                        };
                    }
                }

                let step = grid_meta.egui_step(rect);

                let color_of_delta_energy = |delta_energy: DeltaTotalEnergy| {
                    // fade to slightly different purples to show off 0
                    if delta_energy >= 0. {
                        egui::Color32::lerp_to_gamma(
                            &egui::Color32::from_rgb(130, 0, 100),
                            egui::Color32::RED,
                            (delta_energy / mag_scale) as f32,
                        )
                    } else {
                        egui::Color32::lerp_to_gamma(
                            &egui::Color32::from_rgb(100, 0, 130),
                            egui::Color32::BLUE,
                            (-delta_energy / mag_scale) as f32,
                        )
                    }
                };

                let color_of_total_energy = |total_energy: TotalEnergy| {
                    // TODO: does this work
                    color_of_delta_energy(total_energy / 100.0)
                };

                let color_of_goodness = |goodness: Goodness| {
                    // TODO: does this work
                    color_of_delta_energy(goodness / 100.0)
                };

                let color_of_delta_goodness =
                    |delta_goodness: Goodness| color_of_delta_energy(delta_goodness);

                for (row, line) in grid_meta.rects(rect).enumerate() {
                    for (col, cell_rect) in line.enumerate() {
                        let init_vel = grid_meta.row_col_usize_to_vel((row, col));
                        let init_state = State {
                            pos: Vec3::ZERO,
                            vel: init_vel,
                        };

                        let cen = rect.left_top() + egui::vec2(col as f32, row as f32) * step;
                        let get_delta_vel = |pitch: f32| {
                            let new_state = init_state.ticked(Rot { x: pitch, y: 0. });
                            new_state.vel - init_state.vel
                        };
                        let get_immediate_energy_color = |pitch: f32| {
                            let new_state = init_state.ticked(Rot { x: pitch, y: 0. });
                            let delta_energy = new_state.total_energy() - init_state.total_energy();
                            color_of_delta_energy(delta_energy)
                        };

                        // let draw_pitch_arrow = |pitch, | {

                        // };

                        match draw_arrow_type {
                            DrawArrowType::FixedDeltaVel => {
                                // delta vel along global pitch (colored by delta energy)
                                // let new_state = init_state.ticked(fixed_rot);
                                // let delta_vel = new_state.vel - init_state.vel;
                                // let delta_energy =
                                //     new_state.total_energy() - init_state.total_energy();
                                // let color = color_of_energy(delta_energy);
                                let delta_vel = get_delta_vel(fixed_rot.x);
                                let color = get_immediate_energy_color(fixed_rot.x);
                                ui.painter().arrow(
                                    cen,
                                    delta_vel.yz_to_egui_vec2().normalized() * arrow_scale * step,
                                    egui::Stroke::new(0.2 * step, color),
                                );
                            }
                            DrawArrowType::ImmediateOptimalPitch => {
                                // optimal pitch (colored by delta energy)
                                let pitch = immediate_optimal_pitches.0[row][col];
                                // let rot = Rot { x: pitch, y: 0. };
                                // let new_state = init_state.ticked(rot);
                                // let delta_energy =
                                //     new_state.total_energy() - init_state.total_energy();
                                // let color = color_of_energy(delta_energy);
                                let color = get_immediate_energy_color(pitch);
                                ui.painter().arrow(
                                    cen,
                                    egui::Vec2::angled(pitch * std::f32::consts::PI / 180.)
                                        * arrow_scale
                                        * step,
                                    egui::Stroke::new(0.2 * step, color),
                                );
                            }
                            DrawArrowType::ImmediateOptimalDeltaVel => {
                                // delta vel along optimal pitch (colored by delta energy)
                                let pitch = immediate_optimal_pitches.0[row][col];
                                // let rot = Rot { x: pitch, y: 0. };
                                // let new_state = init_state.ticked(rot);
                                // let delta_vel = new_state.vel - init_state.vel;
                                // let delta_energy =
                                //     new_state.total_energy() - init_state.total_energy();
                                // let color = color_of_energy(delta_energy);
                                let delta_vel = get_delta_vel(pitch);
                                let color = get_immediate_energy_color(pitch);
                                ui.painter().arrow(
                                    cen,
                                    egui::vec2(delta_vel.z as f32, -delta_vel.y as f32)
                                        .normalized()
                                        * arrow_scale
                                        * step,
                                    egui::Stroke::new(0.2 * step, color),
                                );
                            }
                            // DrawArrowType::DeepOptimalPitch => {
                            //     // optimal pitch (colored by delta energy)
                            //     let pitch = deep_optim.pitches.0[row][col];
                            //     // let rot = Rot { x: pitch, y: 0. };
                            //     // let new_state = init_state.ticked(rot);
                            //     // let delta_energy =
                            //     //     new_state.total_energy() - init_state.total_energy();
                            //     // let color = color_of_energy(delta_energy);
                            //     // let color = get_immediate_energy_color(pitch);
                            //     // let color = deep_optim
                            //     //     .states
                            //     //     .state_bilinear_from_row_col_float((row as f32, col as f32))
                            //     //     .map(|state| color_of_total_energy(state.total_energy()))
                            //     //     .unwrap_or(egui::Color32::BLACK);
                            //     let color = deep_optim
                            //         .goodnesses
                            //         .f64_bilinear_from_row_col_float((row as f32, col as f32))
                            //         .map(color_of_goodness)
                            //         .unwrap_or(egui::Color32::BLACK);
                            //     ui.painter().arrow(
                            //         cen,
                            //         egui::Vec2::angled(pitch * std::f32::consts::PI / 180.)
                            //             * arrow_scale
                            //             * step,
                            //         egui::Stroke::new(0.2 * step, color),
                            //     );
                            // }
                            DrawArrowType::DeepOptimalPitch => {
                                // optimal pitch (colored by delta energy)

                                // let Vel3 {
                                //     x: _,
                                //     y: y_vel,
                                //     z: z_vel,
                                // } = grid_meta.row_col_usize_to_vel((row, col));
                                // if let Some((pitch, goodness)) =
                                //     dp.trilinear_from_vals((Y_POS_INIT, y_vel, z_vel))
                                // {
                                //     let color = color_of_goodness(goodness);
                                //     ui.painter().arrow(
                                //         cen,
                                //         egui::Vec2::angled(pitch * std::f32::consts::PI / 180.)
                                //             * arrow_scale
                                //             * step,
                                //         egui::Stroke::new(0.2 * step, color),
                                //     );
                                // }

                                let Vel3 {
                                    x: _,
                                    y: y_vel,
                                    z: z_vel,
                                } = grid_meta.row_col_usize_to_vel((row, col));
                                let key_query = DPKey::from_yz_vel(y_vel, z_vel);
                                // let key_representative = key_query.to_representative();
                                let DPValue { pitch, goodness } = dp.get(dp_tick, key_query);
                                // let color = color_of_goodness(goodness);
                                // let color =
                                //     color_of_delta_goodness(goodness - key.base_case().goodness);
                                let color = color_of_delta_goodness(
                                    goodness - key_query.to_state().total_energy(),
                                );
                                // let color = color_of_delta_goodness(
                                //     goodness - key_representative.0.to_state().total_energy(),
                                // );
                                // make the grains visible
                                // let representative_hash = {
                                //     use std::hash::{Hash, Hasher};
                                //     let mut h = std::hash::DefaultHasher::new();
                                //     key_representative.hash(&mut h);
                                //     h.finish()
                                // };
                                // let color = egui::Color32::from_rgba_premultiplied(
                                //     color.r(),
                                //     color.g(),
                                //     color.b(),
                                //     127 + representative_hash as u8 / 2,
                                // );
                                ui.painter().arrow(
                                    cen,
                                    egui::Vec2::angled(
                                        pitch.unwrap_or(-180.0) * std::f32::consts::PI / 180.,
                                    ) * arrow_scale
                                        * step,
                                    egui::Stroke::new(0.2 * step, color),
                                );
                            }
                            // DrawArrowType::DeepOptimalPitch => {
                            //     let pitch = 0.0;
                            //     let goodness = goodness_for_vel_y_after_ticks(
                            //         init_state.pos.y,
                            //         init_state.vel.y,
                            //         init_state.vel.z,
                            //         LOOKAHEAD.load(Ordering::Relaxed) as usize,
                            //     );
                            //     let color = color_of_goodness(goodness);
                            //     ui.painter().arrow(
                            //         cen,
                            //         egui::Vec2::angled(pitch * std::f32::consts::PI / 180.)
                            //             * arrow_scale
                            //             * step,
                            //         egui::Stroke::new(0.2 * step, color),
                            //     );
                            // }
                            // DrawArrowType::DeepOptimalDeltaVel => {
                            //     // delta vel along optimal pitch (colored by delta energy)
                            //     let pitch = deep_optim.pitches.0[row][col];
                            //     // let rot = Rot { x: pitch, y: 0. };
                            //     // let new_state = init_state.ticked(rot);
                            //     // let delta_vel = new_state.vel - init_state.vel;
                            //     // let delta_energy =
                            //     //     new_state.total_energy() - init_state.total_energy();
                            //     // let color = color_of_energy(delta_energy);
                            //     let delta_vel = get_delta_vel(pitch);
                            //     // let color = get_immediate_energy_color(pitch);
                            //     // let color = deep_optim
                            //     //     .states
                            //     //     .state_bilinear_from_row_col_float((row as f32, col as f32))
                            //     //     .map(|state| color_of_total_energy(state.total_energy()))
                            //     //     .unwrap_or(egui::Color32::BLACK);
                            //     let color = deep_optim
                            //         .goodnesses
                            //         .f64_bilinear_from_row_col_float((row as f32, col as f32))
                            //         .map(color_of_goodness)
                            //         .unwrap_or(egui::Color32::BLACK);
                            //     ui.painter().arrow(
                            //         cen,
                            //         egui::vec2(delta_vel.z as f32, -delta_vel.y as f32)
                            //             .normalized()
                            //             * arrow_scale
                            //             * step,
                            //         egui::Stroke::new(0.2 * step, color),
                            //     );
                            // }
                            DrawArrowType::DeepOptimalDeltaVel => {}
                        }

                        // // set/toggle clicked_cell on click
                        // if ui.allocate_rect(cell_rect, egui::Sense::CLICK).clicked() {
                        //     if clicked_cell == Some((row, col)) {
                        //         clicked_cell = None;
                        //     } else {
                        //         clicked_cell = Some((row, col));
                        //     }
                        // }
                    }
                }

                // draw the dot on the representative, if showing a dp arrow grid
                #[cfg(false)]
                if matches!(
                    draw_arrow_type,
                    DrawArrowType::DeepOptimalPitch | DrawArrowType::DeepOptimalDeltaVel
                ) {
                    let key_representative = DPKey::from_state(&hovered_state).to_representative();
                    let rep_vel = key_representative.0.to_state().vel;
                    let rep_egui_pos = grid_meta.vel_to_egui_pos2(rep_vel, rect);
                    ui.painter()
                        .circle_filled(rep_egui_pos, 3., egui::Color32::WHITE);
                }

                // draw the path from the clicked cell
                // if let Some((row, col)) = clicked_cell {
                if let Some(clicked_state) = &clicked_state {
                    // let mut start = grid_meta.row_col_usize_to_egui_pos2((row, col), rect);
                    let mut state = clicked_state.clone();
                    ui.painter().circle_filled(
                        grid_meta.vel_to_egui_pos2(state.vel, rect),
                        4.,
                        egui::Color32::GOLD,
                    );
                    const PATH_LEN: usize = 10;
                    for tick in 0..PATH_LEN {
                        let pitch = match draw_arrow_type {
                            DrawArrowType::FixedDeltaVel => fixed_rot.x,
                            DrawArrowType::ImmediateOptimalPitch
                            | DrawArrowType::ImmediateOptimalDeltaVel => immediate_optimal_pitches
                                .f32_bilinear_from_row_col_float(
                                    grid_meta.vel_to_grid_row_col_float(state.vel),
                                )
                                .unwrap_or(0.),
                            DrawArrowType::DeepOptimalPitch
                            | DrawArrowType::DeepOptimalDeltaVel => dp
                                // the pitch displayed on the grid (for a constant tick)
                                .get(dp_tick, DPKey::from_state(&state))
                                .pitch
                                .unwrap_or(0.),
                            // the pitch as though we want to optimize our goodness after `PATH_LEN` ticks
                            // .get(PATH_LEN - tick, DPKey::from_state(&state))
                            // .pitch
                            // .unwrap(),
                        };

                        let start = grid_meta.vel_to_egui_pos2(state.vel, rect);
                        state = state.ticked(Rot { x: pitch, y: 0. });
                        let end = grid_meta.vel_to_egui_pos2(state.vel, rect);

                        // pitch
                        ui.painter().arrow(
                            start,
                            20. * egui::Vec2::angled(pitch * std::f32::consts::PI / 180.),
                            (2., PINK),
                        );
                        ui.painter()
                            .line_segment([start, end], (3., egui::Color32::GOLD));
                        ui.painter().circle_filled(end, 4., egui::Color32::GOLD);
                    }
                }

                // TODO: factor out, show multiple at once
                // replay path
                for i in 0..state_index {
                    let state = &replay_states[i];
                    let next = &replay_states[i + 1];

                    // draw dot at state
                    let start = grid_meta.vel_to_egui_pos2(state.vel, rect);
                    ui.painter().circle_filled(start, 4., egui::Color32::GOLD);

                    // draw line to next state
                    let end = grid_meta.vel_to_egui_pos2(next.vel, rect);
                    ui.painter()
                        .line_segment([start, end], (3., egui::Color32::GOLD));
                }

                // at last state draw dot and pitch arrow and delta vel arrow
                {
                    let state = &replay_states[state_index];
                    let start = grid_meta.vel_to_egui_pos2(state.vel, rect);

                    // dot
                    ui.painter().circle_filled(start, 4., egui::Color32::GOLD);

                    // pitch arrow (pink)
                    ui.painter().arrow(
                        start,
                        40. * egui::Vec2::angled(
                            REPLAY_PITCHES[state_index % REPLAY_PITCHES.len()]
                                * std::f32::consts::PI
                                / 180.,
                        ),
                        (3., PINK),
                    );

                    let vel_scale = 60.;

                    // vel arrow (green)
                    {
                        ui.painter().arrow(
                            start,
                            vel_scale * egui::vec2(state.vel.z as f32, -state.vel.y as f32),
                            (3., egui::Color32::from_rgb(0, 170, 0)),
                        );
                    }

                    // TODO: arrow for (potential, kinetic)
                    // TODO: arrow direction for best energy?

                    // arrow of argmax_over_pitch_of_delta_energy (fancy color)
                    {
                        let best_pitch = argmax_over_pitch_of_delta_energy(state.vel);
                        let rot = Rot {
                            x: best_pitch,
                            y: 0.,
                        };
                        let new_state = state.ticked(rot);
                        let delta_vel = new_state.vel - state.vel;
                        let color = egui::Color32::lerp_to_gamma(
                            &color_of_delta_energy(new_state.total_energy() - state.total_energy()),
                            egui::Color32::WHITE,
                            0.5,
                        );
                        ui.painter().arrow(
                            start,
                            vel_scale * 18. * delta_vel.yz_to_egui_vec2(),
                            egui::Stroke::new(3., color),
                        );
                    }

                    // delta vel arrow (light green)
                    {
                        let next = state.ticked(Rot {
                            x: REPLAY_PITCHES[state_index % REPLAY_PITCHES.len()],
                            y: 0.,
                        });
                        let delta_vel = next.vel - state.vel;
                        ui.painter().arrow(
                            start,
                            vel_scale * 20. * delta_vel.yz_to_egui_vec2(),
                            (3., egui::Color32::from_rgb(100, 238, 100)),
                        );
                    }
                }
            });
        },
    )
}

pub fn pos_slider(value: &mut f64) -> egui::Slider<'_> {
    egui::Slider::new(value, -100.0..=100.0).clamping(egui::SliderClamping::Never)
}

pub fn vel_slider(value: &mut f64) -> egui::Slider<'_> {
    egui::Slider::new(value, -5.0..=5.0).clamping(egui::SliderClamping::Never)
}

pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn inv_lerp_f32(a: f32, b: f32, v: f32) -> f32 {
    (v - a) / (b - a)
}

pub fn lerp_f64(a: f64, b: f64, t: f64) -> f64 {
    // assert!((0.0..=1.0).contains(&t));
    a + (b - a) * t
}

pub fn inv_lerp_f64(a: f64, b: f64, v: f64) -> f64 {
    // assert!((a..=b).contains(&v));
    (v - a) / (b - a)
}

pub fn lerp_vec3(a: Vec3, b: Vec3, t: f64) -> Vec3 {
    Vec3::new(
        lerp_f64(a.x, b.x, t),
        lerp_f64(a.y, b.y, t),
        lerp_f64(a.z, b.z, t),
    )
}

pub fn lerp_state(a: &State, b: &State, t: f64) -> State {
    State {
        pos: lerp_vec3(a.pos, b.pos, t),
        vel: lerp_vec3(a.vel, b.vel, t),
    }
}

#[derive(Debug, PartialEq)]
/// everything is colored by delta energy
enum DrawArrowType {
    // don't actually do this because it's just arrows pointing in the same direction
    // /// draw the pitch for the global fixed pitch
    // FixedPitch,
    /// draw the delta vel for the global fixed pitch
    FixedDeltaVel,
    /// draw the pitch for the immediate optimizer
    ImmediateOptimalPitch,
    /// draw the delta vel for the immediate optimizer
    ImmediateOptimalDeltaVel,
    /// draw the pitch for the deep optimizer
    DeepOptimalPitch,
    /// draw the delta vel for the deep optimizer
    DeepOptimalDeltaVel,
}
