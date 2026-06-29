mod optimizer;
mod sim;

use egui::NumExt;
use itertools::Itertools;
use rand::RngExt;
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

    let mut pitch_idx = 0;
    let mut pitches = default_pitches(num_ticks);

    let mut init_vel = Vel3::ZERO;
    // the optimal steady state vel
    // let mut init_vel = Vel3::new(0.0, 0.17, 0.2);

    let mut draw_with_jitter = true;
    let mut draw_without_jitter = false;
    // disabling this should show overfitting
    // TODO: check that happens
    let mut resample_jitter_on_optimization_step = true;

    let mut jitter_params = JitterParams {
        // time_rad: 0.2,
        time_rad: 0.0,
        init_vel_y_std: 0.1,
        init_vel_z_std: 0.1,
        // vels_y_std: 0.01,
        // vels_z_std: 0.01,
        vels_y_std: 0.0,
        vels_z_std: 0.0,
        pitches_std: 0.0,
    };

    let mut jitter = Jitter::new(&jitter_params, num_ticks);

    let mut learning_rate = 500.0;
    let mut decay = 0.0;
    // this has no effect if resample_jitter_on_optimization_step is disabled
    let mut batch_size = 8;
    let mut optimizing = false;
    let mut optimization_steps_per_frame: usize = 10;

    // TODO: try out uniform scaling
    let mut min_y = 0.0;
    let mut max_y = 25.0;
    let mut min_z = 300.0;
    let mut max_z = 400.0;
    let mut after_states: Vec<State> = Vec::new();

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

                                ui.label("pitch idx:");
                                ui.add(egui::Slider::new(
                                    &mut pitch_idx,
                                    0..=num_ticks.saturating_sub(1),
                                ));
                                ui.label("pitch:");
                                ui.add(egui::Slider::new(&mut pitches[pitch_idx], -90.0..=90.0));

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
                                fn f(ui: &mut egui::Ui, value: &mut f64, hi: f64) -> bool {
                                    let r = ui.add(
                                        egui::Slider::new(value, 0.0..=hi)
                                            .clamping(egui::SliderClamping::Never),
                                    );
                                    r.changed()
                                }

                                ui.checkbox(&mut draw_with_jitter, "draw with jitter");
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

                                ui.label("time:");
                                ui.add(
                                    egui::Slider::new(&mut jitter.time, -0.5..=0.5)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                ui.label("time rad:");
                                if f(ui, &mut jitter_params.time_rad, 0.5) {
                                    jitter.resample_time(jitter_params.time_rad);
                                }
                                ui.label("init_vel_y std:");
                                if f(ui, &mut jitter_params.init_vel_y_std, 0.2) {
                                    jitter.resample_init_vel_y(jitter_params.init_vel_y_std);
                                }
                                ui.label("init_vel_z std:");
                                if f(ui, &mut jitter_params.init_vel_z_std, 0.2) {
                                    jitter.resample_init_vel_z(jitter_params.init_vel_z_std);
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

                                ui.label("decay:");
                                ui.add(
                                    egui::Slider::new(&mut decay, 0.0..=0.01)
                                        .logarithmic(true)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                ui.label("batch size:");
                                ui.add(
                                    egui::Slider::new(&mut batch_size, 1..=20)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                ui.label("steps per frame:");
                                ui.add(
                                    egui::Slider::new(&mut optimization_steps_per_frame, 0..=100)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                let goodness = goodness_params.build();
                                let mut do_optimization_step = || {
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

                                    // gradient descent
                                    // #[cfg(false)]
                                    {
                                        let grad = if resample_jitter_on_optimization_step {
                                            // one of the jitters must be the one shown in the ui
                                            // so that with a batch size of 1, you see exactly what's happening.
                                            // TODO: really all of these should be shown on the ui. like the average gradient of them.
                                            let jitters = std::iter::once(jitter.clone())
                                                .chain((1..batch_size).map(|_| {
                                                    Jitter::new(&jitter_params, num_ticks)
                                                }))
                                                .collect_vec();
                                            jitter.resample_all(&jitter_params);

                                            let grads = jitters
                                                .into_par_iter()
                                                .map(|jitter| {
                                                    // let mut pitches = PitchesUtil::lerp_between(
                                                    //     &pitches,
                                                    //     jitter.time as f32,
                                                    // )
                                                    // .collect_vec();
                                                    let mut pitches = pitches.clone();
                                                    get_grad(
                                                        &goodness,
                                                        init_vel,
                                                        &mut pitches,
                                                        &jitter,
                                                    )
                                                    .collect_vec()
                                                })
                                                .collect::<Vec<_>>();

                                            grads
                                                .into_iter()
                                                .reduce(|a, b| {
                                                    a.iter()
                                                        .zip(b.iter())
                                                        .map(|(x, y)| x + y)
                                                        .collect()
                                                })
                                                .unwrap()
                                                .into_iter()
                                                .map(|g| g / batch_size as f32)
                                                .collect_vec()
                                        } else {
                                            get_grad(&goodness, init_vel, &mut pitches, &jitter)
                                                .collect_vec()
                                        };

                                        apply_grad(&mut pitches, &grad, learning_rate);
                                    }

                                    after_states.push(forward_last(
                                        init_vel,
                                        &pitches,
                                        &Jitter::zero(num_ticks),
                                    ));
                                };

                                // optimization on / off
                                if ui.button("optimization step").clicked() {
                                    do_optimization_step();
                                }
                                ui.checkbox(&mut optimizing, "optimizing");

                                // do the optimization steps
                                if optimizing {
                                    for _ in 0..optimization_steps_per_frame {
                                        do_optimization_step();
                                    }
                                }
                            });

                        egui::CollapsingHeader::new("pareto frontier")
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.label("min_y:");
                                ui.add(
                                    egui::Slider::new(&mut min_y, -20.0..=25.0)
                                        .clamping(egui::SliderClamping::Never),
                                );
                                ui.label("max_y:");
                                ui.add(
                                    egui::Slider::new(&mut max_y, -20.0..=25.0)
                                        .clamping(egui::SliderClamping::Never),
                                );
                                ui.label("min_z:");
                                ui.add(
                                    egui::Slider::new(&mut min_z, 0.0..=700.0)
                                        .clamping(egui::SliderClamping::Never),
                                );
                                ui.label("max_z:");
                                ui.add(
                                    egui::Slider::new(&mut max_z, 0.0..=700.0)
                                        .clamping(egui::SliderClamping::Never),
                                );
                            });

                        // state before and after
                        ui.group(|ui| {
                            ui.label(format!("before vel.y: {:.06}", init_vel.y));
                            ui.label(format!("before vel.z: {:.06}", init_vel.z));
                            {
                                let after = forward_last(init_vel, &pitches, &jitter);
                                ui.label("with jitter:");
                                ui.label(format!("after vel.y: {:.06}", after.vel.y));
                                ui.label(format!("after vel.z: {:.06}", after.vel.z));
                                ui.strong(format!("after pos.y: {:.06}", after.pos.y));
                                ui.label(format!("after pos.z: {:.06}", after.pos.z));
                                {}
                                let after =
                                    forward_last(init_vel, &pitches, &Jitter::zero(num_ticks));
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

                if draw_with_jitter {
                    show_optimizer(
                        ui,
                        &rect,
                        &goodness_params,
                        init_vel,
                        &pitches,
                        &jitter,
                        4.0,
                    );
                }
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

            egui::Window::new("pareto frontier")
                .default_pos(egui::pos2(230.0, 20.0))
                .default_size(egui::vec2(100.0, 100.0))
                .show(ui, |ui| {
                    // let rect = ui.content_rect();
                    // let rect = ui.max_rect();
                    let rect = ui.available_rect_before_wrap();

                    ui.label(format!("count: {}", after_states.len()));

                    ui.allocate_rect(rect, egui::Sense::hover());
                    let painter = ui.painter_at(rect);

                    // let (_, painter) = ui.allocate_painter(rect.size(), egui::Sense::hover());

                    // ui.set_clip_rect(rect);

                    // vertical lines for pos.z
                    {
                        const STEP: f32 = 5.0;
                        for z in ((min_z / STEP).round()) as i32..=(max_z / STEP).round() as i32 {
                            let z = z as f32 * STEP;
                            let screen_x =
                                rect.left() + ((z - min_z) / (max_z - min_z)) * rect.width();
                            painter.line_segment(
                                [
                                    egui::pos2(screen_x, rect.top()),
                                    egui::pos2(screen_x, rect.bottom()),
                                ],
                                egui::Stroke::new(1.0, egui::Color32::from_gray(50)),
                            );
                        }
                    }

                    // horizontal lines for pos.y
                    // after pos.z so that the bright y = 0.0 line is drawn on top
                    {
                        const STEP: f32 = 5.0;
                        for y in ((min_y / STEP).round() as i32)..=(max_y / STEP).round() as i32 {
                            let y = y as f32 * STEP;
                            let screen_y =
                                rect.bottom() - ((y - min_y) / (max_y - min_y)) * rect.height();
                            painter.line_segment(
                                [
                                    egui::pos2(rect.left(), screen_y),
                                    egui::pos2(rect.right(), screen_y),
                                ],
                                egui::Stroke::new(
                                    1.0,
                                    if y == 0.0 {
                                        egui::Color32::from_gray(150)
                                    } else {
                                        egui::Color32::from_gray(50)
                                    },
                                ),
                            );
                        }
                    }

                    let mut dot_at = |state: &State, color: egui::Color32| {
                        let x = rect.left()
                            + ((state.pos.z as f32 - min_z) / (max_z - min_z)) * rect.width();
                        let y = rect.bottom()
                            - ((state.pos.y as f32 - min_y) / (max_y - min_y)) * rect.height();
                        dot_at(ui, x, y, 2.0, color).on_hover_text(format!(
                            "pos.y: {:.06}, pos.z: {:.06}",
                            state.pos.y, state.pos.z
                        ));
                    };

                    for state in &after_states {
                        dot_at(state, egui::Color32::from_rgb(255, 0, 0));
                    }

                    // draw the current after state white
                    if let Some(state) = after_states.last() {
                        dot_at(state, egui::Color32::WHITE);
                    }
                });
        },
    )
}

fn dot_at(ui: &mut egui::Ui, x: f32, y: f32, rad: f32, color: egui::Color32) -> egui::Response {
    let rect_before = ui.max_rect();
    let dot_rect =
        egui::Rect::from_center_size(egui::pos2(x, y), egui::Vec2::splat(2.0 * rad.at_least(4.0)));
    let r = ui.allocate_rect(dot_rect.intersect(ui.max_rect()), egui::Sense::hover());
    ui.painter().circle_filled(egui::pos2(x, y), rad, color);
    assert_eq!(rect_before, ui.max_rect());
    r
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

    let mut dot_at = |x, y: f32, color: egui::Color32| dot_at(ui, x, y, rad, color);

    for (tick, (pitch, state)) in forward(init_vel, pitches, jitter).enumerate() {
        let x = rect.left() + (tick as f32 / pitches.len() as f32) * rect.width();

        // pitch (pink)
        {
            let y = value_to_y(-pitch, 90.0);
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
            let approx_max_grad = 0.0003
                * match goodness_params.energy_or_y {
                    EnergyOrY::Energy => 1.5,
                    EnergyOrY::Y => 20.0,
                };
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
            // vaguely normalize them so blending feels more uniform
            let goodness_left = match energy_or_y {
                EnergyOrY::Energy => state.total_energy(),
                EnergyOrY::Y => state.pos.y / 20.0,
            };
            let goodness_right = state.pos.z / 300.0;
            goodness_left * (1.0 - y_z_blend) + goodness_right * y_z_blend
        }
    }
}
