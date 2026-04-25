mod sim;

use sim::*;

pub const TICK_DURATION: std::time::Duration = std::time::Duration::from_millis(50); // 20 per second

fn main() -> eframe::Result {
    let mut entity = Entity {
        pos: Vec3::ZERO,
        vel: Vec3::ZERO,
        rot: Rot { x: 0.0, y: 0.0 },
    };

    let mut running = false;
    let mut next_tick = std::time::Instant::now();

    eframe::run_ui_native(
        "Eltyra Sim",
        eframe::NativeOptions::default(),
        move |ui, _frame| {
            let now = std::time::Instant::now();

            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui.group(|ui| {
                    ui.checkbox(&mut running, "Running");
                    if ui.button("Step").clicked() {
                        entity.travel();
                        running = false;
                    } else if running {
                        if next_tick <= now {
                            entity.travel();
                            next_tick += TICK_DURATION;
                        }
                        ui.request_repaint_after(next_tick.saturating_duration_since(now));
                    }
                    if ui.button("Reset").clicked() {
                        entity = Entity::default();
                    }
                });

                ui.group(|ui| {
                    ui.strong("Position");
                    ui.label("X");
                    ui.add(pos_slider(&mut entity.pos.x));
                    ui.label("Y");
                    ui.add(pos_slider(&mut entity.pos.y));
                    ui.label("Z");
                    ui.add(pos_slider(&mut entity.pos.z));
                });

                ui.group(|ui| {
                    ui.strong("Velocity");
                    ui.label(format!("X = {:.3}", entity.vel.x * 20.0));
                    ui.add(vel_slider(&mut entity.vel.x));
                    ui.label(format!("Y = {:.3}", entity.vel.y * 20.0));
                    ui.add(vel_slider(&mut entity.vel.y));
                    ui.label(format!("Z = {:.3}", entity.vel.z * 20.0));
                    ui.add(vel_slider(&mut entity.vel.z));
                });

                ui.group(|ui| {
                    ui.strong("Rotation");
                    ui.label("X");
                    ui.add(
                        egui::Slider::new(&mut entity.rot.x, -180.0..=180.0)
                            .clamping(egui::SliderClamping::Never),
                    );
                    ui.label("Y");
                    ui.add(
                        egui::Slider::new(&mut entity.rot.y, -90.0..=90.0)
                            .clamping(egui::SliderClamping::Never),
                    );
                });
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
