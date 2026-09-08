use crate::sim::*;
use crate::{inv_lerp_f32, inv_lerp_f64, lerp_f32, lerp_f64};

const PITCH_LO: Pitch = -89.0;
const PITCH_HI: Pitch = 89.0;
/// the unlabeled vertical gridlines.
const PITCH_GRID: [Pitch; 5] = [-60.0, -30.0, 0.0, 30.0, 60.0];

/// height of the curve itself, without the strip of pitch labels below it.
const PLOT_HEIGHT: f32 = 80.0;
/// the strip of pitch labels below each plot.
const AXIS_HEIGHT: f32 = 12.0;

/// roughly one sample per pixel of width, within these bounds.
const MIN_SAMPLES: usize = 181;
const MAX_SAMPLES: usize = 2048;

/// the curve: how much the pitch moves the quantity.
const CURVE_COLOR: egui::Color32 = egui::Color32::from_rgb(100, 238, 100);
/// the horizontal line at zero, which the deltas are measured against.
/// gold, as the current state is drawn gold on the grid.
const ZERO_COLOR: egui::Color32 = egui::Color32::GOLD;

/// a quantity whose change over the flight is plotted.
#[derive(Copy, Clone)]
enum Quantity {
    Vy,
    Vz,
    Ke,
    Pe,
    Te,
    /// total energy after *holding* the pitch, not after one tick of it.
    HeldTe,
}

impl Quantity {
    const ALL: [Self; 6] = [
        Self::Vy,
        Self::Vz,
        Self::Ke,
        Self::Pe,
        Self::Te,
        Self::HeldTe,
    ];

    fn label(self, held_ticks: usize) -> String {
        match self {
            Self::HeldTe => format!("held dte/({held_ticks} ticks)"),
            _ => self.id().to_owned(),
        }
    }

    /// the collapsing header's id.
    /// separate from the label so that dragging the held ticks slider
    /// doesn't collapse the plot it retitles.
    fn id(self) -> &'static str {
        match self {
            Self::Vy => "dvy",
            Self::Vz => "dvz",
            Self::Ke => "dke",
            Self::Pe => "dpe",
            Self::Te => "dte",
            Self::HeldTe => "held dte",
        }
    }

    /// how much flying `pitch` moves the quantity, per tick.
    fn delta_per_tick(self, from: &State, pitch: Pitch, held_ticks: usize) -> f64 {
        let ticks = match self {
            Self::HeldTe => held_ticks,
            _ => 1,
        };
        let rot = Rot { x: pitch, y: 0.0 };
        let mut to = from.clone();
        for _ in 0..ticks {
            to = to.ticked(rot);
        }
        (self.of(&to) - self.of(from)) / ticks as f64
    }

    fn of(self, state: &State) -> f64 {
        match self {
            Self::Vy => state.vel.y,
            Self::Vz => state.vel.z,
            Self::Ke => state.kinetic_energy(),
            Self::Pe => state.potential_energy(),
            Self::Te | Self::HeldTe => state.total_energy(),
        }
    }

    fn range(self) -> (f64, f64) {
        match self {
            Self::Vy => (-0.10, 0.30),
            Self::Vz => (-0.10, 0.10),
            Self::Ke | Self::Pe | Self::Te | Self::HeldTe => (-0.15, 0.15),
        }
    }
}

pub fn ui(ui: &mut egui::Ui, hovered_state: &State, fixed_pitch: Pitch, held_ticks: usize) {
    egui::CollapsingHeader::new("pitch profiles")
        .default_open(true)
        .show(ui, |ui| {
            for quantity in Quantity::ALL {
                egui::CollapsingHeader::new(quantity.label(held_ticks))
                    .id_salt(quantity.id())
                    .default_open(match quantity {
                        Quantity::Vy | Quantity::Vz | Quantity::Te | Quantity::HeldTe => true,
                        Quantity::Ke | Quantity::Pe => false,
                    })
                    .show(ui, |ui| {
                        plot(ui, quantity, hovered_state, fixed_pitch, held_ticks)
                    });
            }
        });
}

fn plot(
    ui: &mut egui::Ui,
    quantity: Quantity,
    hovered_state: &State,
    fixed_pitch: Pitch,
    held_ticks: usize,
) {
    let (range_lo, range_hi) = quantity.range();
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), PLOT_HEIGHT + AXIS_HEIGHT),
        egui::Sense::hover(),
    );
    let plot_rect = egui::Rect::from_min_max(
        response.rect.min,
        egui::pos2(response.rect.max.x, response.rect.max.y - AXIS_HEIGHT),
    );

    let x_of_pitch = |pitch: Pitch| {
        lerp_f32(
            plot_rect.left(),
            plot_rect.right(),
            inv_lerp_f32(PITCH_LO, PITCH_HI, pitch),
        )
    };
    let y_of_value = |value: f64| {
        lerp_f64(
            plot_rect.bottom() as f64,
            plot_rect.top() as f64,
            inv_lerp_f64(range_lo, range_hi, value),
        ) as f32
    };

    let weak_color = ui.visuals().weak_text_color();
    let grid_stroke =
        egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color);
    let font = egui::FontId::monospace(9.0);

    painter.rect_filled(plot_rect, 2.0, ui.visuals().extreme_bg_color);
    for pitch in PITCH_GRID {
        painter.vline(x_of_pitch(pitch), plot_rect.y_range(), grid_stroke);
    }

    // the pitch the rest of the panel is reporting on, in the pitch color.
    // the slider it comes from is unclamped, so it can sit off the sweep.
    if (PITCH_LO..=PITCH_HI).contains(&fixed_pitch) {
        painter.vline(
            x_of_pitch(fixed_pitch),
            plot_rect.y_range(),
            (1., crate::PINK),
        );
    }

    // no change: above it the pitch gains, below it the pitch loses.
    let zero_y = y_of_value(0.0);
    painter.extend(egui::Shape::dashed_line(
        &[
            egui::pos2(plot_rect.left(), zero_y),
            egui::pos2(plot_rect.right(), zero_y),
        ],
        egui::Stroke::new(1.0_f32, ZERO_COLOR),
        4.0,
        4.0,
    ));

    let samples = (plot_rect.width().round() as usize).clamp(MIN_SAMPLES, MAX_SAMPLES);
    let points = (0..=samples)
        .map(|i| {
            let fraction = i as f32 / samples as f32;
            let pitch = lerp_f32(PITCH_LO, PITCH_HI, fraction);
            let delta = quantity.delta_per_tick(hovered_state, pitch, held_ticks);
            egui::pos2(
                lerp_f32(plot_rect.left(), plot_rect.right(), fraction),
                y_of_value(delta.clamp(range_lo, range_hi)),
            )
        })
        .collect::<Vec<_>>();
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(1.5_f32, CURVE_COLOR),
    ));

    painter.rect_stroke(plot_rect, 2.0, grid_stroke, egui::StrokeKind::Inside);

    // the y extents, and the zero line's own label.
    painter.text(
        plot_rect.right_top() + egui::vec2(-3.0, 1.0),
        egui::Align2::RIGHT_TOP,
        format!("{range_hi:.2}"),
        font.clone(),
        weak_color,
    );
    painter.text(
        plot_rect.right_bottom() + egui::vec2(-3.0, -1.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{range_lo:.2}"),
        font.clone(),
        weak_color,
    );
    painter.text(
        egui::pos2(plot_rect.left() + 3.0, zero_y),
        // keep the label off the edge when the line is near the top
        if zero_y < plot_rect.center().y {
            egui::Align2::LEFT_TOP
        } else {
            egui::Align2::LEFT_BOTTOM
        },
        "0",
        font.clone(),
        ZERO_COLOR,
    );

    // the pitch axis
    for (pitch, align) in [
        (PITCH_LO, egui::Align2::LEFT_TOP),
        (0., egui::Align2::CENTER_TOP),
        (PITCH_HI, egui::Align2::RIGHT_TOP),
    ] {
        painter.text(
            egui::pos2(x_of_pitch(pitch), plot_rect.bottom() + 2.0),
            align,
            format!("{pitch:.0}"),
            font.clone(),
            weak_color,
        );
    }
}
