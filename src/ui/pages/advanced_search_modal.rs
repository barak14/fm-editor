//! Advanced player search modal.
//!
//! Opened from the filter row's "⚙ Advanced" button. Shares `PlayerSearchState`
//! directly — no separate draft — so filter row and modal are always in sync.
//! The caller snapshots state before opening and restores it on `ModalResult::Cancel`.

use eframe::egui;

use super::player_search::{POSITION_OFFSETS, PlayerSearchState};

/// Returned by `show()` each frame.
#[derive(Default, PartialEq)]
pub enum ModalResult {
    #[default]
    None,
    Ok,
    Cancel,
    /// Reset every field displayed in the modal, keeping query and sorting.
    ClearAll,
}

/// Render the Advanced Search modal window.
///
/// `open` is set to false when the modal is dismissed (OK or Cancel).
/// The caller is responsible for snapshotting and restoring state on Cancel.
pub fn show(ctx: &egui::Context, state: &mut PlayerSearchState, open: &mut bool) -> ModalResult {
    let mut result = ModalResult::None;

    egui::Window::new("Advanced Search")
        .open(open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(440.0)
        .show(ctx, |ui| {
            show_general_tab(ui, state);
            if let Some(error) = state.validation_error() {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }

            ui.separator();

            // ── Footer ────────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                if ui.button("Clear All").clicked() {
                    result = ModalResult::ClearAll;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        result = ModalResult::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        result = ModalResult::Cancel;
                    }
                });
            });
        });

    // OK / Cancel close the modal.
    if result == ModalResult::Ok || result == ModalResult::Cancel {
        *open = false;
    }

    result
}

// ── General tab ──────────────────────────────────────────────────────────────

fn show_general_tab(ui: &mut egui::Ui, state: &mut PlayerSearchState) {
    ui.horizontal_wrapped(|ui| {
        ui.vertical(|ui| {
            section_title(ui, "Inclusive ranges");
            for (label, min, max) in [
                ("Age", &mut state.age_min, &mut state.age_max),
                ("CA", &mut state.ca_min, &mut state.ca_max),
                ("PA", &mut state.pa_min, &mut state.pa_max),
            ] {
                ui.horizontal(|ui| {
                    ui.label(format!("{label} ≥"));
                    ui.add(
                        egui::TextEdit::singleline(min)
                            .desired_width(44.0)
                            .hint_text("min"),
                    );
                    ui.label("≤");
                    ui.add(
                        egui::TextEdit::singleline(max)
                            .desired_width(44.0)
                            .hint_text("max"),
                    );
                });
            }
        });
        ui.vertical(|ui| {
            section_title(ui, "Positions");
            ui.label("Any selected position (OR)");
            show_pitch(ui, state);
        });
    });
}

// ── Positions pitch ───────────────────────────────────────────────────────────

/// Formation layout: (bit_index_left, bit_index_center, bit_index_right).
/// None = empty cell. Matches player_panel::show_positions_formation.
/// POSITION_OFFSETS order: 0=GK,1=SW,2=DL,3=DC,4=DR,5=DM,6=ML,7=MC,8=MR,
///                         9=AML,10=AMC,11=AMR,12=ST,13=WBL,14=WBR
const PITCH_ROWS: &[(Option<usize>, Option<usize>, Option<usize>)] = &[
    (None, Some(12), None),        // ST
    (Some(9), Some(10), Some(11)), // AML AMC AMR
    (Some(6), Some(7), Some(8)),   // ML  MC  MR
    (Some(13), Some(5), Some(14)), // WBL DM  WBR
    (Some(2), Some(3), Some(4)),   // DL  DC  DR
    (None, Some(0), None),         // GK
];

fn show_pitch(ui: &mut egui::Ui, state: &mut PlayerSearchState) {
    let pitch_bg = egui::Color32::from_rgb(45, 90, 27);
    let btn_size = egui::vec2(46.0, 26.0);
    let btn_spacing = 4.0;

    let pitch_width = btn_size.x * 3.0 + btn_spacing * 2.0 + 20.0;
    let pitch_height = btn_size.y * 6.0 + btn_spacing * 5.0 + 20.0;

    let (pitch_rect, _) =
        ui.allocate_exact_size(egui::vec2(pitch_width, pitch_height), egui::Sense::hover());

    // Draw green pitch background
    ui.painter()
        .rect_filled(pitch_rect, egui::CornerRadius::same(6), pitch_bg);
    // Pitch inner border
    let inner = pitch_rect.shrink(4.0);
    ui.painter().rect_stroke(
        inner,
        egui::CornerRadius::same(3),
        egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 60),
        ),
        egui::StrokeKind::Outside,
    );

    let origin = pitch_rect.min + egui::vec2(10.0, 10.0);

    for (row_i, &(left, center, right)) in PITCH_ROWS.iter().enumerate() {
        let y = origin.y + row_i as f32 * (btn_size.y + btn_spacing);
        for (col_i, opt_bit) in [left, center, right].iter().enumerate() {
            let x = origin.x + col_i as f32 * (btn_size.x + btn_spacing);
            let rect = egui::Rect::from_min_size(egui::pos2(x, y), btn_size);

            if let Some(bit) = opt_bit {
                let label = POSITION_OFFSETS[*bit].1;
                let selected = state.positions & (1u16 << bit) != 0;

                let btn_resp = ui.put(rect, pos_toggle_button(label, selected));
                if btn_resp.clicked() {
                    state.positions ^= 1u16 << bit;
                }
                btn_resp.on_hover_text(if selected {
                    format!("Remove {label} filter")
                } else {
                    format!("Filter by {label}")
                });
            }
        }
    }

    // SW — shown below the pitch grid as a separate row
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Sweeper:")
                .small()
                .color(egui::Color32::from_rgb(180, 180, 180)),
        );
        let sw_bit = 1usize; // bit 1 = SW
        let selected = state.positions & (1u16 << sw_bit) != 0;
        if ui.add(pos_toggle_button("SW", selected)).clicked() {
            state.positions ^= 1u16 << sw_bit;
        }
    });
}

fn pos_toggle_button(label: &str, selected: bool) -> egui::Button<'static> {
    let label = egui::RichText::new(label.to_string())
        .small()
        .strong()
        .color(if selected {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_rgba_unmultiplied(220, 220, 220, 180)
        });

    let fill = if selected {
        egui::Color32::from_rgb(74, 124, 199)
    } else {
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 100)
    };

    egui::Button::new(label).fill(fill)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .small()
            .strong()
            .color(egui::Color32::from_rgb(160, 160, 160)),
    );
    ui.add_space(2.0);
}
