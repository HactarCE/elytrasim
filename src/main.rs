mod optimizer;
mod sim;

use itertools::Itertools;
use rayon::prelude::*;

use crate::optimizer::*;

pub const TICKS_PER_SECOND: u8 = 20;
pub const TICK_DURATION: std::time::Duration = std::time::Duration::from_millis(50); // 20 per second

fn main() -> eframe::Result {
    let mut goodness_params = GoodnessParams {
        energy_or_y: EnergyOrY::Energy,
        y_z_blend: 0.0,
    };

    // let ticks = 10;
    // let ticks = 20;
    // let ticks = 50;
    // let ticks = 100; // like -6 delta y
    // let ticks = 150; // like 2 delta y
    // let ticks = 200; // like 12 delta y
    // let ticks = 250;
    let mut num_ticks = 300; // like 21.5 delta y
    // let ticks = 310; // like 21.65 delta y
    // let ticks = 400; // like 18 delta y
    // let ticks = 500;

    let mut pitches = default_pitches(num_ticks);

    let mut init_vel = Vel3::ZERO;
    // the optimal steady state vel
    // let mut init_vel = Vel3::new(0.0, 0.17, 0.2);

    let mut draw_without_jitter = false;
    // disabling this should show overfitting
    // TODO: check that happens
    let mut resample_jitter_on_optimization_step = true;

    let mut jitter_params = JitterParams {
        init_vel_y_std: Some(0.1),
        init_vel_z_std: Some(0.1),
        poses_y_std: Some(0.1),
        poses_z_std: Some(0.1),
        vels_y_std: Some(0.01),
        vels_z_std: Some(0.01),
        pitches_std: Some(0.0),
    };

    let mut jitter = Jitter::new(&jitter_params, num_ticks);

    let mut learning_rate = 500.0;
    // this has no effect if resample_jitter_on_optimization_step is disabled
    let mut batch_size = 8;
    let mut decay = 0.0;
    let mut optimizing = false;
    let mut optimization_steps_per_frame: usize = 10;

    eframe::run_ui_native(
        "Elytra Sim",
        eframe::NativeOptions::default(),
        move |ui, _frame| {
            ui.request_repaint();
            egui::Panel::left("side_panel")
                .resizable(false)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // goodness
                        egui::CollapsingHeader::new("goodness function")
                            .default_open(true)
                            .show(ui, |ui| {
                                egui::ComboBox::from_id_salt(egui::Id::new("energy or y"))
                                    .selected_text(match goodness_params.energy_or_y {
                                        EnergyOrY::Energy => "energy",
                                        EnergyOrY::Y => "y",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut goodness_params.energy_or_y,
                                            EnergyOrY::Energy,
                                            "energy",
                                        );
                                        ui.selectable_value(
                                            &mut goodness_params.energy_or_y,
                                            EnergyOrY::Y,
                                            "y",
                                        );
                                    });

                                ui.label("y/z blend:");
                                ui.add(egui::Slider::new(
                                    &mut goodness_params.y_z_blend,
                                    0.0..=1.0,
                                ));
                            });

                        // init_vel
                        egui::CollapsingHeader::new("init vel")
                            .default_open(true)
                            .show(ui, |ui| {
                                if ui.button("zero").clicked() {
                                    init_vel = Vel3::ZERO;
                                }
                                if ui.button("steady state").clicked() {
                                    init_vel = Vel3::new(0.0, 0.17, 0.2);
                                }
                                ui.label("init vel.y:");
                                ui.add(vel_slider(&mut init_vel.y));
                                ui.label("init vel.z:");
                                ui.add(vel_slider(&mut init_vel.z));
                            });

                        // num_ticks
                        egui::CollapsingHeader::new("num ticks")
                            .default_open(true)
                            .show(ui, |ui| {
                                assert_eq!(num_ticks, pitches.len());
                                assert_eq!(num_ticks, jitter.num_ticks());

                                // increase / decrease num_ticks
                                ui.horizontal(|ui| {
                                    ui.label(format!("num ticks: {}", num_ticks));

                                    let mul = if ui.ctx().input(|i| i.modifiers.shift) {
                                        10
                                    } else {
                                        1
                                    };

                                    let mut changed = false;
                                    if ui.button("-").on_hover_text("hold shift for 10x").clicked()
                                    {
                                        changed |= true;
                                        num_ticks = num_ticks.saturating_sub(mul);
                                    }
                                    if ui.button("+").on_hover_text("hold shift for 10x").clicked()
                                    {
                                        changed |= true;
                                        num_ticks += mul;
                                    }
                                    if changed {
                                        pitches.resize(
                                            num_ticks,
                                            pitches.last().copied().unwrap_or(0.0),
                                        );
                                        jitter.resize(&jitter_params, num_ticks);
                                    }
                                });
                            });

                        // pitches
                        egui::CollapsingHeader::new("pitches stuff")
                            .default_open(true)
                            .show(ui, |ui| {
                                if ui.button("duplicate").clicked() {
                                    num_ticks *= 2;
                                    pitches =
                                        pitches.iter().chain(pitches.iter()).cloned().collect();
                                    jitter.resize(&jitter_params, num_ticks);
                                }

                                if ui.button("default").clicked() {
                                    pitches = default_pitches(num_ticks);
                                }

                                if ui.button("random uniform").clicked() {
                                    pitches = PitchesUtil::new_rand_uniform(num_ticks);
                                }

                                if ui.button("random walk").clicked() {
                                    pitches = PitchesUtil::new_rand_walk(num_ticks, 10.0);
                                }

                                // if ui.button("print pitches").clicked() {
                                //     println!("{:#?}", pitches);
                                // }
                                // if ui.button("print speed pitches").clicked() {
                                //     for tick in (0..pitches.len()).step_by(5) {
                                //         let state = Pitches(pitches[..tick].to_owned())
                                //             .after_cycle(init_vel);
                                //         let speed = state.vel.length();
                                //         let pitch = pitches[tick];
                                //         // println!("tick: {}, speed: {:.06}, pitch: {:.06}", tick, 20.0*speed, pitch);
                                //         println!("{}, {:.06}, {:.06}", tick, 20.0 * speed, pitch);
                                //     }
                                // }
                            });

                        // jitter
                        egui::CollapsingHeader::new("jitter stuff")
                            .default_open(true)
                            .show(ui, |ui| {
                                /// returns whether it should be resampled
                                fn f(ui: &mut egui::Ui, value: &mut Option<f64>, hi: f64) -> bool {
                                    let mut v = value.unwrap_or(0.0);
                                    let r = ui.add(
                                        egui::Slider::new(&mut v, 0.0..=hi)
                                            .clamping(egui::SliderClamping::Never),
                                    );
                                    if r.changed() {
                                        *value = Some(v);
                                    }
                                    r.changed()
                                }

                                ui.checkbox(&mut draw_without_jitter, "draw without jitter");

                                {
                                    let mut undo_jitter_vel =
                                        UNDO_JITTER_VEL.load(std::sync::atomic::Ordering::Relaxed);
                                    ui.checkbox(&mut undo_jitter_vel, "undo jitter vel");
                                    UNDO_JITTER_VEL.store(
                                        undo_jitter_vel,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                }

                                ui.checkbox(
                                    &mut resample_jitter_on_optimization_step,
                                    "resample jitter on optimization step",
                                );

                                if ui.button("resample all").clicked() {
                                    jitter.resample_all(&jitter_params);
                                }

                                ui.label("init_vel_y std:");
                                if f(ui, &mut jitter_params.init_vel_y_std, 0.2) {
                                    jitter.resample_init_vel_y(jitter_params.init_vel_y_std);
                                }
                                ui.label("init_vel_z std:");
                                if f(ui, &mut jitter_params.init_vel_z_std, 0.2) {
                                    jitter.resample_init_vel_z(jitter_params.init_vel_z_std);
                                }
                                ui.label("poses_y std:");
                                if f(ui, &mut jitter_params.poses_y_std, 1.0) {
                                    jitter.resample_poses_y(jitter_params.poses_y_std);
                                }
                                ui.label("poses_z std:");
                                if f(ui, &mut jitter_params.poses_z_std, 1.0) {
                                    jitter.resample_poses_z(jitter_params.poses_z_std);
                                }
                                ui.label("vels_y std:");
                                if f(ui, &mut jitter_params.vels_y_std, 0.05) {
                                    jitter.resample_vels_y(jitter_params.vels_y_std);
                                }
                                ui.label("vels_z std:");
                                if f(ui, &mut jitter_params.vels_z_std, 0.05) {
                                    jitter.resample_vels_z(jitter_params.vels_z_std);
                                }
                                ui.label("pitches std:");
                                if f(ui, &mut jitter_params.pitches_std, 1.0) {
                                    jitter.resample_pitches(jitter_params.pitches_std);
                                }
                            });

                        // optimizer
                        egui::CollapsingHeader::new("optimizer stuff")
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.label("learning rate:");
                                ui.add(
                                    egui::Slider::new(&mut learning_rate, 10.0..=10000.0)
                                        .logarithmic(true)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                ui.label("batch size:");
                                ui.add(
                                    egui::Slider::new(&mut batch_size, 1..=20)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                ui.label("decay:");
                                ui.add(
                                    egui::Slider::new(&mut decay, 0.0..=0.01)
                                        .logarithmic(true)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                let goodness = goodness_params.build();
                                let mut do_optimization_step = || {
                                    let grad = if resample_jitter_on_optimization_step {
                                        // one of the jitters must be the one shown in the ui
                                        // so that with a batch size of 1, you see exactly what's happening.
                                        // TODO: really all of these should be shown on the ui. like the average gradient of them.
                                        let jitters =
                                            std::iter::once(jitter.clone())
                                                .chain((1..batch_size).map(|_| {
                                                    Jitter::new(&jitter_params, num_ticks)
                                                }))
                                                .collect_vec();
                                        jitter.resample_all(&jitter_params);

                                        let grads = jitters
                                            .into_par_iter()
                                            .map(|jitter| {
                                                let mut pitches = pitches.clone();
                                                get_grad(&goodness, init_vel, &mut pitches, &jitter)
                                                    .collect_vec()
                                            })
                                            .collect::<Vec<_>>();

                                        grads
                                            .into_iter()
                                            .reduce(|a, b| {
                                                a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
                                            })
                                            .unwrap()
                                            .into_iter()
                                            .map(|g| g / batch_size as f32)
                                            .collect_vec()
                                    } else {
                                        get_grad(&goodness, init_vel, &mut pitches, &jitter)
                                            .collect_vec()
                                    };

                                    // TODO: batched gradients

                                    apply_grad(&mut pitches, &grad, learning_rate);

                                    // let before = pitches.clone();
                                    apply_decay(&mut pitches, 1.0 - decay);
                                    // dbg!(before.iter().zip(&pitches).map(|(b, p)| (b, p, b - p)).collect_vec());
                                    // if decay == 0.0 {
                                    //     for (b, p) in before.iter().zip(&pitches) {
                                    //         assert!(
                                    //             (b - p).abs() < 1e-3,
                                    //             "before: {}, after: {}, diff: {}",
                                    //             b,
                                    //             p,
                                    //             b - p
                                    //         );
                                    //     }
                                    // }
                                };

                                // optimization on / off
                                if ui.button("optimization step").clicked() {
                                    do_optimization_step();
                                }
                                ui.checkbox(&mut optimizing, "optimizing");
                                ui.label("steps per frame:");
                                ui.add(egui::Slider::new(
                                    &mut optimization_steps_per_frame,
                                    0..=100,
                                ));

                                // do the optimization steps
                                if optimizing {
                                    for _ in 0..optimization_steps_per_frame {
                                        do_optimization_step();
                                    }
                                }
                            });

                        // state before and after
                        ui.group(|ui| {
                            ui.label(format!("before vel.y: {:.06}", init_vel.y));
                            ui.label(format!("before vel.z: {:.06}", init_vel.z));
                            {
                                let after = forward(init_vel, &pitches, &jitter).last().unwrap();
                                ui.label("with jitter:");
                                ui.label(format!("after vel.y: {:.06}", after.vel.y));
                                ui.label(format!("after vel.z: {:.06}", after.vel.z));
                                ui.strong(format!("after pos.y: {:.06}", after.pos.y));
                                ui.label(format!("after pos.z: {:.06}", after.pos.z));
                                {}
                                let after = forward(init_vel, &pitches, &Jitter::zero(num_ticks))
                                    .last()
                                    .unwrap();
                                ui.label("without jitter:");
                                ui.label(format!("after vel.y: {:.06}", after.vel.y));
                                ui.label(format!("after vel.z: {:.06}", after.vel.z));
                                ui.strong(format!("after pos.y: {:.06}", after.pos.y));
                                ui.label(format!("after pos.z: {:.06}", after.pos.z));
                            }
                        });
                    });
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                let rect = ui.max_rect();

                // horizontal center line
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.left(), rect.center().y),
                        egui::pos2(rect.right(), rect.center().y),
                    ],
                    egui::Stroke::new(1.0, egui::Color32::from_gray(250)),
                );

                // vertical lines for seconds
                {
                    let seconds = pitches.len() as f32 / TICKS_PER_SECOND as f32;
                    for second in 0..=seconds.ceil() as usize {
                        let x = rect.left()
                            + (second as f32 * TICKS_PER_SECOND as f32 / pitches.len() as f32)
                                * rect.width();
                        ui.painter().line_segment(
                            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                            egui::Stroke::new(1.0, egui::Color32::from_gray(150)),
                        );
                    }
                }

                show_optimizer(
                    ui,
                    &rect,
                    &goodness_params,
                    init_vel,
                    &pitches,
                    &jitter,
                    4.0,
                );
                if draw_without_jitter {
                    show_optimizer(
                        ui,
                        &rect,
                        &goodness_params,
                        init_vel,
                        &pitches,
                        &Jitter::zero(num_ticks),
                        4.0,
                    );
                }
            });
        },
    )
}

fn show_optimizer(
    ui: &mut egui::Ui,
    rect: &egui::Rect,
    goodness_params: &GoodnessParams,
    init_vel: Vel3,
    pitches: &[Pitch],
    jitter: &Jitter,
    rad: f32,
) {
    let value_to_y = |value: f32, approx_max_value: f32| {
        rect.center().y - (value / approx_max_value) * (rect.height() / 2.0)
    };

    let mut dot_at = |x, y: f32, color: egui::Color32| {
        let dot_rect =
            egui::Rect::from_center_size(egui::Pos2::new(x, y), egui::Vec2::splat(2.0 * rad));
        let r = ui.allocate_rect(dot_rect, egui::Sense::hover());
        ui.painter().circle_filled(egui::pos2(x, y), rad, color);
        r
    };

    for (tick, (state, pitch)) in forward(init_vel, pitches, jitter)
        .zip(pitches.iter())
        .enumerate()
    {
        let x = rect.left() + (tick as f32 / pitches.len() as f32) * rect.width();

        // pitch (pink)
        {
            let y = value_to_y(-*pitch, 90.0);
            dot_at(x, y, egui::Color32::from_rgb(252, 3, 198))
                .on_hover_text(format!("tick: {}, pitch: {}", tick, pitch));
        }

        // pitch gradient (purple)
        // actually this just goes to zero, so it's not very interesting
        // #[cfg(false)]
        {
            // this is for the ui, just clone it.
            let mut pitches = pitches.to_owned();
            let grad = grad_at_tick(
                goodness_params.build(),
                init_vel,
                &mut pitches,
                jitter,
                tick,
            );
            let approx_max_grad = 0.001;
            let y = value_to_y(
                -grad.clamp(-approx_max_grad, approx_max_grad),
                approx_max_grad,
            );
            dot_at(x, y, egui::Color32::from_rgb(128, 0, 128))
                .on_hover_text(format!("tick: {}, pitch gradient: {}", tick, grad));
        }

        // pos.y (dark green)
        {
            let y = value_to_y(state.pos.y as f32, 100.0);
            dot_at(x, y, egui::Color32::from_rgb(0, 100, 0))
                .on_hover_text(format!("tick: {}, pos.y: {}", tick, state.pos.y));
        }

        // pos.z (dark blue)
        {
            let y = value_to_y(state.pos.z as f32, 100.0);
            dot_at(x, y, egui::Color32::from_rgb(52, 61, 235))
                .on_hover_text(format!("tick: {}, pos.z: {}", tick, state.pos.z));
        }

        // vel.y (light green)
        {
            let y = value_to_y(state.vel.y as f32, 5.0);
            dot_at(x, y, egui::Color32::from_rgb(144, 238, 144))
                .on_hover_text(format!("tick: {}, vel.y: {}", tick, state.vel.y));
        }

        // vel.z (light blue)
        {
            let y = value_to_y(state.vel.z as f32, 5.0);
            dot_at(x, y, egui::Color32::from_rgb(52, 165, 235))
                .on_hover_text(format!("tick: {}, vel.z: {}", tick, state.vel.z));
        }

        let approx_max_energy = 7.0;
        // kinetic energy (yellow)
        {
            let ke = state.kinetic_energy();
            let y = value_to_y(ke as f32, approx_max_energy);
            dot_at(x, y, egui::Color32::from_rgb(235, 214, 52))
                .on_hover_text(format!("tick: {}, kinetic energy: {}", tick, ke));
        }

        // potential energy (red)
        {
            let pe = state.potential_energy();
            let y = value_to_y(pe as f32, approx_max_energy);
            dot_at(x, y, egui::Color32::from_rgb(255, 0, 0))
                .on_hover_text(format!("tick: {}, potential energy: {}", tick, pe));
        }

        // total energy (orange)
        {
            let energy = state.total_energy();
            let y = value_to_y(energy as f32, approx_max_energy);
            dot_at(x, y, egui::Color32::from_rgb(235, 143, 52))
                .on_hover_text(format!("tick: {}, total energy: {}", tick, energy));
        }
    }
}

// pub const TICK_DURATION: std::time::Duration = std::time::Duration::from_millis(50); // 20 per second

// fn main() -> eframe::Result {
//     let mut entity = Entity {
//         pos: Vec3::ZERO,
//         vel: Vec3::ZERO,
//         rot: Rot { x: 0.0, y: 0.0 },
//     };

//     let mut running = false;
//     let mut next_tick = std::time::Instant::now();

//     eframe::run_ui_native(
//         "Elytra Sim",
//         eframe::NativeOptions::default(),
//         move |ui, _frame| {
//             let now = std::time::Instant::now();

//             egui::CentralPanel::default().show_inside(ui, |ui| {
//                 ui.group(|ui| {
//                     ui.checkbox(&mut running, "Running");
//                     if ui.button("Step").clicked() {
//                         entity.travel();
//                         running = false;
//                     } else if running {
//                         if next_tick <= now {
//                             entity.travel();
//                             next_tick += TICK_DURATION;
//                         }
//                         ui.request_repaint_after(next_tick.saturating_duration_since(now));
//                     }
//                     if ui.button("Reset").clicked() {
//                         entity = Entity::default();
//                     }
//                 });

//                 ui.group(|ui| {
//                     ui.strong("Position");
//                     ui.label("X");
//                     ui.add(pos_slider(&mut entity.pos.x));
//                     ui.label("Y");
//                     ui.add(pos_slider(&mut entity.pos.y));
//                     ui.label("Z");
//                     ui.add(pos_slider(&mut entity.pos.z));
//                 });

//                 ui.group(|ui| {
//                     ui.strong("Velocity");
//                     ui.label(format!("X = {:.3}", entity.vel.x * 20.0));
//                     ui.add(vel_slider(&mut entity.vel.x));
//                     ui.label(format!("Y = {:.3}", entity.vel.y * 20.0));
//                     ui.add(vel_slider(&mut entity.vel.y));
//                     ui.label(format!("Z = {:.3}", entity.vel.z * 20.0));
//                     ui.add(vel_slider(&mut entity.vel.z));
//                 });

//                 ui.group(|ui| {
//                     ui.strong("Rotation");
//                     ui.label("X");
//                     ui.add(
//                         egui::Slider::new(&mut entity.rot.x, -180.0..=180.0)
//                             .clamping(egui::SliderClamping::Never),
//                     );
//                     ui.label("Y");
//                     ui.add(
//                         egui::Slider::new(&mut entity.rot.y, -90.0..=90.0)
//                             .clamping(egui::SliderClamping::Never),
//                     );
//                 });
//             });
//         },
//     )
// }

pub fn pos_slider(value: &mut f64) -> egui::Slider<'_> {
    egui::Slider::new(value, -100.0..=100.0).clamping(egui::SliderClamping::Never)
}

pub fn vel_slider(value: &mut f64) -> egui::Slider<'_> {
    egui::Slider::new(value, -5.0..=5.0).clamping(egui::SliderClamping::Never)
}

fn default_pitches(num_ticks: usize) -> Vec<Pitch> {
    // let pitches = PitchesUtil::new_uniform(ticks, 0.0);
    // let pitches = PitchesUtil::new_4040(ticks, 0.65);
    // let pitches = PitchesUtil::new_40zero40(ticks, 0.65, 0.70);
    // close to the optimal curve with four lines
    let left_cut = 0.65;
    let right_cut = 0.70;
    let right_right_cut = 0.80;
    let left = (num_ticks as f64 * left_cut) as usize;
    let right = (num_ticks as f64 * right_cut) as usize;
    let right_right = (num_ticks as f64 * right_right_cut) as usize;

    // PitchesUtil::new_lerp(left_left, 0.0, 10.0)
    //     .iter()
    //     .chain(PitchesUtil::new_lerp(left - left_left, 10.0, 50.0).0.iter())
    PitchesUtil::new_lerp(left, 10.0, 50.0)
        .iter()
        .chain(PitchesUtil::new_constant(right - left, 0.0).iter())
        .chain(PitchesUtil::new_lerp(right_right - right, -85.0, -30.0).iter())
        .chain(PitchesUtil::new_lerp(num_ticks - right_right, -30.0, -10.0).iter())
        .cloned()
        .collect::<Vec<_>>()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnergyOrY {
    Energy,
    Y,
}

struct GoodnessParams {
    energy_or_y: EnergyOrY,
    y_z_blend: f64,
}

impl GoodnessParams {
    fn build(&self) -> impl Fn(State) -> Goodness {
        let Self {
            energy_or_y,
            y_z_blend,
        } = self;
        move |state: State| {
            let goodness_left = match energy_or_y {
                EnergyOrY::Energy => state.total_energy(),
                EnergyOrY::Y => state.pos.y,
            };
            let goodness_right = state.pos.z;
            goodness_left * (1.0 - y_z_blend) + goodness_right * y_z_blend
        }
    }
}
