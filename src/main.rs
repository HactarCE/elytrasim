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
    let mut num_ticks: usize = 300; // like 21.5 delta y
    // let ticks = 310; // like 21.65 delta y
    // let ticks = 400; // like 18 delta y
    // let ticks = 500;

    // TODO: better initialization
    let mut nn = Nn::new_4040();
    let mut show_nn_grad = true;
    let mut show_nn_editor = true;

    let mut init_vel = Vel3::ZERO;
    // the optimal steady state vel
    // let mut init_vel = Vel3::new(0.0, 0.17, 0.2);

    let mut draw_with_jitter = true;
    let mut draw_without_jitter = false;
    // disabling this should show overfitting
    // TODO: check that happens
    let mut resample_jitter_on_optimization_step = true;

    let mut jitter_params = JitterParams {
        init_vel_y_std: 0.1,
        init_vel_z_std: 0.1,
    };

    let mut jitter = Jitter::new(&jitter_params);

    let mut learning_rate = 0.001;
    // this has no effect if resample_jitter_on_optimization_step is disabled
    let mut batch_size = 8;
    let mut decay = 0.0;
    let mut optimizing = false;
    let mut optimization_steps_per_frame: usize = 10;

    // TODO: try out uniform scaling
    let mut min_y = 0.0;
    let mut max_y = 25.0;
    let mut min_z = 300.0;
    let mut max_z = 400.0;
    let mut after_states_without_jitter: Vec<State> = Vec::new();
    let mut after_states_with_jitter: Vec<State> = Vec::new();

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
                                // increase / decrease num_ticks
                                ui.horizontal(|ui| {
                                    ui.label(format!("num ticks: {}", num_ticks));

                                    let mul = if ui.ctx().input(|i| i.modifiers.shift) {
                                        10
                                    } else {
                                        1
                                    };

                                    if ui.button("-").on_hover_text("hold shift for 10x").clicked()
                                    {
                                        num_ticks = num_ticks.saturating_sub(mul);
                                    }
                                    if ui.button("+").on_hover_text("hold shift for 10x").clicked()
                                    {
                                        num_ticks += mul;
                                    }
                                });
                            });

                        egui::CollapsingHeader::new("nn")
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.checkbox(&mut show_nn_grad, "show gradient");
                                ui.checkbox(&mut show_nn_editor, "show editor");

                                // increase / decrease num_terms
                                ui.horizontal(|ui| {
                                    ui.label(format!("num terms: {}", nn.terms.len()));

                                    let mul = if ui.ctx().input(|i| i.modifiers.shift) {
                                        10
                                    } else {
                                        1
                                    };

                                    if ui.button("-").on_hover_text("hold shift for 10x").clicked()
                                    {
                                        nn.terms.truncate(nn.terms.len().saturating_sub(mul));
                                    }
                                    if ui.button("+").on_hover_text("hold shift for 10x").clicked()
                                    {
                                        nn.terms.extend((0..mul).map(|_| Term::new_random()));
                                    }
                                });

                                if ui
                                    .button("randomize")
                                    .on_hover_text("randomize all terms")
                                    .clicked()
                                {
                                    nn.terms
                                        .iter_mut()
                                        .for_each(|term| *term = Term::new_random());
                                }

                                let grad = if show_nn_grad {
                                    nn.get_grad(
                                        goodness_params.build(),
                                        num_ticks,
                                        init_vel + jitter.init_vel,
                                    )
                                } else {
                                    Nn::zero(nn.terms.len())
                                };

                                let mut term_idx = 0;
                                while term_idx < nn.terms.len() {
                                    // TODO: show the pitches that a term contributes

                                    let mut increment = true;
                                    egui::CollapsingHeader::new(format!("term {}", term_idx))
                                        .default_open(false)
                                        .show(ui, |ui| {
                                            fn show_mask(
                                                ui: &mut egui::Ui,
                                                name: &str,
                                                is_tick_mask: bool,
                                                mask: &mut Mask,
                                                grad_mask: &Mask,
                                                show_nn_grad: bool,
                                                show_nn_sliders: bool,
                                            ) {
                                                egui::CollapsingHeader::new(name)
                                                    .default_open(true)
                                                    .show(ui, |ui| {
                                                        // mu
                                                        {
                                                            let mut mu = mask.mu();
                                                            ui.label(format!("mu: {:.06}", mu));
                                                            if show_nn_grad {
                                                                ui.label(format!(
                                                                    "mu grad: {:.06}",
                                                                    grad_mask.mu()
                                                                ));
                                                            }
                                                            if show_nn_sliders && ui
                                                                .add(
                                                                    egui::Slider::new(
                                                                        &mut mu,
                                                                        if is_tick_mask {
                                                                            0.0..=300.0
                                                                        } else {
                                                                            -5.0..=5.0
                                                                        },
                                                                    )
                                                                    .clamping(
                                                                        egui::SliderClamping::Never,
                                                                    ),
                                                                )
                                                                .changed()
                                                            {
                                                                mask.set_mu(mu);
                                                            }
                                                        }

                                                        // sigma
                                                        {
                                                            let mut sigma = mask.sigma();
                                                            ui.label(format!(
                                                                "sigma: {:.06}",
                                                                sigma
                                                            ));
                                                            if show_nn_grad {
                                                                ui.label(format!(
                                                                    "sigma grad: {:.06}",
                                                                    grad_mask.sigma_raw()
                                                                ));
                                                            }
                                                            if show_nn_sliders && ui
                                                                .add(
                                                                    egui::Slider::new(
                                                                        &mut sigma,
                                                                        if is_tick_mask {
                                                                            0.0..=200.0
                                                                        } else {
                                                                            0.0..=3.0
                                                                        },
                                                                    )
                                                                    .clamping(
                                                                        egui::SliderClamping::Never,
                                                                    ),
                                                                )
                                                                .changed()
                                                            {
                                                                sigma = sigma.max(1e-3);
                                                                mask.set_sigma(sigma);
                                                            }
                                                        }

                                                        // rad
                                                        #[cfg(false)]
                                                        {
                                                            let mut rad = mask.rad();
                                                            ui.label(format!("rad: {:.06}", rad));
                                                            if show_nn_sliders && ui
                                                                .add(
                                                                    egui::Slider::new(
                                                                        &mut rad,
                                                                        if is_tick_mask {
                                                                            0.0..=300.0
                                                                        } else {
                                                                            0.0..=5.0
                                                                        },
                                                                    )
                                                                    .clamping(
                                                                        egui::SliderClamping::Never,
                                                                    ),
                                                                )
                                                                .changed()
                                                            {
                                                                rad = rad.max(1e-3);
                                                                mask.set_rad(rad);
                                                            }
                                                        }
                                                    });
                                            }

                                            if show_nn_editor{
                                                if ui.button("randomize").clicked() {
                                                    nn.terms[term_idx] = Term::new_random();
                                                }
                                                if ui.button("delete").clicked() {
                                                    nn.terms.remove(term_idx);
                                                    increment = false;
                                                }
                                                if ui.button("duplicate").clicked() {
                                                    nn.terms.insert(term_idx, nn.terms[term_idx].clone());
                                                }
                                            }

                                            let Term {
                                                masks: [tick_mask, vel_y_mask, vel_z_mask],
                                                pitch_map,
                                            } = &mut nn.terms[term_idx];
                                            let Term {
                                                masks: [grad_tick_mask, grad_vel_y_mask, grad_vel_z_mask],
                                                pitch_map: grad_pitch_map,
                                            } = &grad.terms[term_idx];

                                            egui::CollapsingHeader::new("masks")
                                                .default_open(true)
                                                .show_unindented(ui, |ui| {
                                                show_mask(
                                                    ui,
                                                    "tick mask",
                                                    true,
                                                    tick_mask,
                                                    grad_tick_mask,
                                                    show_nn_grad,
                                                    show_nn_editor,
                                                );
                                                show_mask(
                                                    ui,
                                                    "vel y mask",
                                                    false,
                                                    vel_y_mask,
                                                    grad_vel_y_mask,
                                                    show_nn_grad,
                                                    show_nn_editor,
                                                );
                                                show_mask(
                                                    ui,
                                                    "vel z mask",
                                                    false,
                                                    vel_z_mask,
                                                    grad_vel_z_mask,
                                                    show_nn_grad,
                                                    show_nn_editor,
                                                );
                                            });

                                            egui::CollapsingHeader::new("pitch map")
                                                .default_open(true)
                                                .show(ui, |ui| {
                                                    let Affine {
                                                        weights:
                                                            [tick_coeff, vel_y_coeff, vel_z_coeff],
                                                        bias,
                                                    } = pitch_map;
                                                    let Affine {
                                                        weights:
                                                            [
                                                                grad_tick_coeff,
                                                                grad_vel_y_coeff,
                                                                grad_vel_z_coeff,
                                                            ],
                                                        bias: grad_bias,
                                                    } = grad_pitch_map;

                                                    ui.label(format!(
                                                        "tick coeff: {:.06}",
                                                        tick_coeff
                                                    ));
                                                    if show_nn_grad {
                                                        ui.label(format!(
                                                            "tick coeff grad: {:.06}",
                                                            grad_tick_coeff
                                                        ));
                                                    }
                                                    if show_nn_editor {
                                                        ui.add(
                                                            egui::Slider::new(
                                                                tick_coeff,
                                                                -1.0..=1.0,
                                                            )
                                                            .clamping(egui::SliderClamping::Never),
                                                        );
                                                    }

                                                    ui.label(format!(
                                                        "vel_y coeff: {:.06}",
                                                        vel_y_coeff
                                                    ));
                                                    if show_nn_grad {
                                                        ui.label(format!(
                                                            "vel_y coeff grad: {:.06}",
                                                            grad_vel_y_coeff
                                                        ));
                                                    }
                                                    if show_nn_editor {
                                                        ui.add(
                                                            egui::Slider::new(
                                                                vel_y_coeff,
                                                                -1.0..=1.0,
                                                            )
                                                            .clamping(egui::SliderClamping::Never),
                                                        );
                                                    }

                                                    ui.label(format!(
                                                        "vel_z coeff: {:.06}",
                                                        vel_z_coeff
                                                    ));
                                                    if show_nn_grad {
                                                        ui.label(format!(
                                                            "vel_z coeff grad: {:.06}",
                                                            grad_vel_z_coeff
                                                        ));
                                                    }
                                                    if show_nn_editor {
                                                        ui.add(
                                                            egui::Slider::new(
                                                                vel_z_coeff,
                                                                -1.0..=1.0,
                                                            )
                                                            .clamping(egui::SliderClamping::Never),
                                                        );
                                                    }

                                                    ui.label(format!("bias: {:.06}", bias));
                                                    if show_nn_grad {
                                                        ui.label(format!(
                                                            "bias grad: {:.06}",
                                                            grad_bias
                                                        ));
                                                    }
                                                    if show_nn_editor {
                                                        ui.add(
                                                            egui::Slider::new(bias, -90.0..=90.0)
                                                                .clamping(
                                                                    egui::SliderClamping::Never,
                                                                ),
                                                        );
                                                    }
                                                });
                                        });
                                    if increment {
                                        term_idx += 1;
                                    }
                                }
                            });

                        // jitter
                        egui::CollapsingHeader::new("jitter stuff")
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.checkbox(&mut draw_with_jitter, "draw with jitter");
                                ui.checkbox(&mut draw_without_jitter, "draw without jitter");

                                ui.checkbox(
                                    &mut resample_jitter_on_optimization_step,
                                    "resample jitter on optimization step",
                                );

                                if ui.button("resample all").clicked() {
                                    jitter.resample_all(&jitter_params);
                                }

                                ui.label("init_vel_y:");
                                ui.add(
                                    egui::Slider::new(&mut init_vel.y, -0.2..=0.2)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                ui.label("init_vel_z:");
                                ui.add(
                                    egui::Slider::new(&mut init_vel.z, -0.2..=0.2)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                ui.label("init_vel_y std:");

                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut jitter_params.init_vel_y_std,
                                            0.0..=0.2,
                                        )
                                        .clamping(egui::SliderClamping::Never),
                                    )
                                    .changed()
                                {
                                    jitter.resample_init_vel_y(jitter_params.init_vel_y_std);
                                }

                                ui.label("init_vel_z std:");
                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut jitter_params.init_vel_z_std,
                                            0.0..=0.2,
                                        )
                                        .clamping(egui::SliderClamping::Never),
                                    )
                                    .changed()
                                {
                                    jitter.resample_init_vel_z(jitter_params.init_vel_z_std);
                                }
                            });

                        // optimizer
                        egui::CollapsingHeader::new("optimizer stuff")
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.label("learning rate:");
                                ui.add(
                                    egui::Slider::new(&mut learning_rate, 0.00001..=1.0)
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

                                ui.label("decay:");
                                ui.add(
                                    egui::Slider::new(&mut decay, 0.0..=0.01)
                                        .logarithmic(true)
                                        .clamping(egui::SliderClamping::Never),
                                );

                                let goodness = goodness_params.build();
                                let mut do_optimization_step = || {
                                    nn.decay(1.0 - decay);

                                    // gradient descent
                                    {
                                        let grad = if resample_jitter_on_optimization_step {
                                            // one of the jitters must be the one shown in the ui
                                            // so that with a batch size of 1, you see exactly what's happening.
                                            // TODO: really all of these should be shown on the ui. like the average gradient of them.
                                            let jitters = std::iter::once(jitter.clone())
                                                .chain(
                                                    (1..batch_size)
                                                        .map(|_| Jitter::new(&jitter_params)),
                                                )
                                                .collect_vec();
                                            jitter.resample_all(&jitter_params);

                                            let grads = jitters
                                                .into_par_iter()
                                                .map(|jitter| {
                                                    let mut nn = nn.clone();
                                                    nn.get_grad(
                                                        &goodness,
                                                        num_ticks,
                                                        init_vel + jitter.init_vel,
                                                    )
                                                })
                                                .collect::<Vec<_>>();

                                            grads.into_iter().reduce(|a, b| a + &b).unwrap()
                                                / batch_size as f32
                                        } else {
                                            nn.get_grad(
                                                &goodness,
                                                num_ticks,
                                                init_vel + jitter.init_vel,
                                            )
                                        };

                                        nn.apply_grad(&grad, learning_rate);
                                    }

                                    after_states_without_jitter
                                        .push(nn.forward_last_(num_ticks, init_vel));
                                    after_states_with_jitter
                                        .push(nn.forward_last_(num_ticks, init_vel));
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
                            // with jitter
                            {
                                let after = nn.forward_last_(num_ticks, init_vel + jitter.init_vel);
                                ui.label("with jitter:");
                                ui.label(format!("after vel.y: {:.06}", after.vel.y));
                                ui.label(format!("after vel.z: {:.06}", after.vel.z));
                                ui.strong(format!("after pos.y: {:.06}", after.pos.y));
                                ui.label(format!("after pos.z: {:.06}", after.pos.z));
                            }
                            // without jitter
                            {
                                let after = nn.forward_last_(num_ticks, init_vel);
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
                    let seconds = num_ticks as f32 / TICKS_PER_SECOND as f32;
                    for second in 0..=seconds.ceil() as usize {
                        let x = rect.left()
                            + (second as f32 * TICKS_PER_SECOND as f32 / num_ticks as f32)
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
                        num_ticks,
                        init_vel,
                        &nn,
                        &jitter,
                        4.0,
                    );
                }
                if draw_without_jitter {
                    show_optimizer(
                        ui,
                        &rect,
                        &goodness_params,
                        num_ticks,
                        init_vel,
                        &nn,
                        &Jitter::zero(),
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

                    ui.label(format!("count: {}", after_states_without_jitter.len()));

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

                    // draw the after_states_without_jitter blue
                    for state in &after_states_with_jitter {
                        dot_at(state, egui::Color32::from_rgb(0, 0, 255));
                    }

                    // draw the after_states_without_jitter red
                    for state in &after_states_without_jitter {
                        dot_at(state, egui::Color32::from_rgb(255, 0, 0));
                    }

                    // draw the current after_state_with_jitter gray
                    if let Some(state) = after_states_with_jitter.last() {
                        dot_at(state, egui::Color32::from_gray(200));
                    }

                    // draw the current after_state_without_jitter white
                    if let Some(state) = after_states_without_jitter.last() {
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
    num_ticks: usize,
    init_vel: Vel3,
    nn: &Nn,
    jitter: &Jitter,
    rad: f32,
) {
    let value_to_y = |value: f32, approx_max_value: f32| {
        rect.center().y - (value / approx_max_value) * (rect.height() / 2.0)
    };

    let mut dot_at = |x, y: f32, color: egui::Color32| dot_at(ui, x, y, rad, color);

    for (tick, (pitch, state)) in nn
        .forward_iter(num_ticks, init_vel + jitter.init_vel)
        .enumerate()
    {
        let x = rect.left() + (tick as f32 / num_ticks as f32) * rect.width();

        // pitch (pink)
        {
            let y = value_to_y(-pitch, 90.0);
            dot_at(x, y, egui::Color32::from_rgb(252, 3, 198)).on_hover_text(format!(
                "tick: {}, pitch: {}",
                num_ticks - tick,
                pitch
            ));
        }

        // pitch gradient (purple)
        // actually this just goes to zero, so it's not very interesting
        #[cfg(false)]
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
            dot_at(x, y, egui::Color32::from_rgb(128, 0, 128)).on_hover_text(format!(
                "tick: {}, pitch gradient: {}",
                num_ticks - tick,
                grad
            ));
        }

        // pos.y (dark green)
        {
            let y = value_to_y(state.pos.y as f32, 100.0);
            dot_at(x, y, egui::Color32::from_rgb(0, 100, 0)).on_hover_text(format!(
                "tick: {}, pos.y: {}",
                num_ticks - tick,
                state.pos.y
            ));
        }

        // pos.z (dark blue)
        {
            let y = value_to_y(state.pos.z as f32, 100.0);
            dot_at(x, y, egui::Color32::from_rgb(52, 61, 235)).on_hover_text(format!(
                "tick: {}, pos.z: {}",
                num_ticks - tick,
                state.pos.z
            ));
        }

        // vel.y (light green)
        {
            let y = value_to_y(state.vel.y as f32, 5.0);
            dot_at(x, y, egui::Color32::from_rgb(144, 238, 144)).on_hover_text(format!(
                "tick: {}, vel.y: {}",
                num_ticks - tick,
                state.vel.y
            ));
        }

        // vel.z (light blue)
        {
            let y = value_to_y(state.vel.z as f32, 5.0);
            dot_at(x, y, egui::Color32::from_rgb(52, 165, 235)).on_hover_text(format!(
                "tick: {}, vel.z: {}",
                num_ticks - tick,
                state.vel.z
            ));
        }

        let approx_max_energy = 7.0;
        // kinetic energy (yellow)
        {
            let ke = state.kinetic_energy();
            let y = value_to_y(ke as f32, approx_max_energy);
            dot_at(x, y, egui::Color32::from_rgb(235, 214, 52)).on_hover_text(format!(
                "tick: {}, kinetic energy: {}",
                num_ticks - tick,
                ke
            ));
        }

        // potential energy (red)
        {
            let pe = state.potential_energy();
            let y = value_to_y(pe as f32, approx_max_energy);
            dot_at(x, y, egui::Color32::from_rgb(255, 0, 0)).on_hover_text(format!(
                "tick: {}, potential energy: {}",
                num_ticks - tick,
                pe
            ));
        }

        // total energy (orange)
        {
            let energy = state.total_energy();
            let y = value_to_y(energy as f32, approx_max_energy);
            dot_at(x, y, egui::Color32::from_rgb(235, 143, 52)).on_hover_text(format!(
                "tick: {}, total energy: {}",
                num_ticks - tick,
                energy
            ));
        }
    }
}

pub fn pos_slider(value: &mut f64) -> egui::Slider<'_> {
    egui::Slider::new(value, -100.0..=100.0).clamping(egui::SliderClamping::Never)
}

pub fn vel_slider(value: &mut f64) -> egui::Slider<'_> {
    egui::Slider::new(value, -5.0..=5.0).clamping(egui::SliderClamping::Never)
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
