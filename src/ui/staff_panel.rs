//! Staff detail panel with a persistent identity header and scrollable details.

use eframe::egui;

use crate::bridge::StaffView;
use crate::fields::{self, Category, Entity};
use crate::ui::entity_header;
use crate::ui::field_widget::{self, FieldEdit};

#[derive(PartialEq)]
pub enum StaffTab {
    Overview,
    Contract,
}

pub fn show(ui: &mut egui::Ui, staff: &mut StaffView, tab: &mut StaffTab) -> Vec<FieldEdit> {
    let mut edits = Vec::new();
    header(ui, staff, "Staff");

    let has_contract = staff.contract_raw.is_some();
    if *tab == StaffTab::Contract && !has_contract {
        *tab = StaffTab::Overview;
    }
    ui.horizontal(|ui| {
        ui.selectable_value(tab, StaffTab::Overview, "Overview");
        if has_contract {
            ui.selectable_value(tab, StaffTab::Contract, "Contract (read-only)");
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| match tab {
        StaffTab::Overview => show_overview(ui, staff, &mut edits),
        StaffTab::Contract => show_contract(ui, staff),
    });

    edits
}

fn header(ui: &mut egui::Ui, staff: &StaffView, kind: &str) {
    entity_header::show(
        ui,
        staff.name.as_deref().unwrap_or("Unknown"),
        kind,
        staff.record.id(),
        entity_header::age(&staff.record.raw, "s_dob"),
        staff.club_name.as_deref(),
        staff.club_addr,
        staff.nationality.as_deref(),
    );
}

fn identity(ui: &mut egui::Ui, staff: &mut StaffView, edits: &mut Vec<FieldEdit>) {
    ui.strong("Identity");
    if let Some(dob) = fields::field("s_dob") {
        edits.extend(field_widget::dob_edit(
            ui,
            &mut staff.record.raw,
            &dob,
            staff.record.addr,
        ));
    }
    if let Some(birthplace) = &staff.birthplace {
        field_widget::info_label(ui, "Place of Birth", birthplace);
    }
    ui.add_space(4.0);
}

fn categories(
    ui: &mut egui::Ui,
    staff: &mut StaffView,
    groups: &[(&str, Category)],
    edits: &mut Vec<FieldEdit>,
) {
    for &(heading, category) in groups {
        field_widget::category_column(
            ui,
            heading,
            Entity::Staff,
            category,
            &mut staff.record.raw,
            staff.record.addr,
            edits,
        );
        ui.add_space(4.0);
    }
}

fn roles(ui: &mut egui::Ui, staff: &StaffView, skip_zeros: bool) {
    if let Some(roles) = &staff.roles {
        field_widget::roles_column(ui, "Roles (read-only)", roles, skip_zeros);
    } else {
        ui.strong("Roles (read-only)");
        ui.label("No role sub-record.");
    }
}

fn show_overview(ui: &mut egui::Ui, staff: &mut StaffView, edits: &mut Vec<FieldEdit>) {
    let count = entity_header::column_count(ui.available_width());
    for chunk in [0, 1, 2].chunks(count) {
        ui.columns(chunk.len(), |cols| {
            for (col, group) in cols.iter_mut().zip(chunk) {
                match group {
                    0 => identity(col, staff, edits),
                    1 => categories(col, staff, &[("Ability", Category::Ability)], edits),
                    _ => categories(col, staff, &[("Reputation", Category::Reputation)], edits),
                }
            }
        });
    }
    ui.separator();

    let groups: &[&[(&str, Category)]] = &[
        &[
            ("Coaching", Category::Coaching),
            ("Medical", Category::Medical),
        ],
        &[
            ("Mental", Category::Mental),
            ("Tactical Style", Category::TacticalStyle),
        ],
        &[
            ("Knowledge", Category::Knowledge),
            ("Board Relations", Category::BoardRelations),
        ],
        &[("Personality", Category::Personality)],
    ];
    for chunk in [0, 1, 2, 3].chunks(count) {
        ui.columns(chunk.len(), |cols| {
            for (col, &group) in cols.iter_mut().zip(chunk) {
                categories(col, staff, groups[group], edits);
                if group == 3 {
                    roles(col, staff, true);
                }
            }
        });
    }
}

/// Single-page layout for the human manager.
///
/// The buffer is patched by `load_human_manager` to expose identity fields at
/// standard staff offsets. Redirect edits for those fields to their live,
/// shifted addresses after collecting the widgets' edits.
pub fn show_manager(ui: &mut egui::Ui, staff: &mut StaffView) -> Vec<FieldEdit> {
    let mut edits = Vec::new();
    let addr = staff.record.addr;
    header(ui, staff, "Human Manager");
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let count = entity_header::column_count(ui.available_width());
        let groups: &[&[(&str, Category)]] = &[
            &[
                ("Ability", Category::Ability),
                ("Reputation", Category::Reputation),
            ],
            &[
                ("Coaching", Category::Coaching),
                ("Personality", Category::Personality),
                ("Medical", Category::Medical),
            ],
            &[
                ("Mental", Category::Mental),
                ("Knowledge", Category::Knowledge),
            ],
            &[
                ("Tactical Style", Category::TacticalStyle),
                ("Board Relations", Category::BoardRelations),
            ],
        ];
        for chunk in [0, 1, 2, 3].chunks(count) {
            ui.columns(chunk.len(), |cols| {
                for (col, &group) in cols.iter_mut().zip(chunk) {
                    if group == 0 {
                        identity(col, staff, &mut edits);
                    }
                    categories(col, staff, groups[group], &mut edits);
                    if group == 0 {
                        if staff.contract_raw.is_some() {
                            show_contract(col, staff);
                            col.add_space(4.0);
                        }
                        roles(col, staff, false);
                    }
                }
            });
        }
    });

    // Redirect only identity fields stored at the human manager's shifted offsets.
    let schema = fields::active_schema();
    for edit in &mut edits {
        let rel = edit.addr.wrapping_sub(addr);
        if schema.is_manager_shifted_offset(rel) {
            edit.addr = edit.addr.wrapping_add(schema.human_manager_shift as u64);
        }
    }

    edits
}

fn show_contract(ui: &mut egui::Ui, staff: &StaffView) {
    ui.strong("Contract (read-only)");
    contract_rows(ui, staff);
}

/// Job, club, wage and expiry from the staff contract sub-record.
fn contract_rows(ui: &mut egui::Ui, staff: &StaffView) {
    let Some(cr) = &staff.contract_raw else {
        return;
    };
    if let Some(job) = fields::offset_of("s_job").and_then(|o| cr.get(o).copied()) {
        let text = match crate::staff::job_name(job) {
            Some(name) => format!("{name} ({job})"),
            None => format!("code {job}"),
        };
        field_widget::info_label(ui, "Job", text);
    }
    if let Some(club) = &staff.club_name {
        field_widget::info_label(ui, "Club", club);
    }
    if let Some(wage) = fields::offset_of("s_weekly_wage")
        .and_then(|o| Some(u32::from_le_bytes(cr.get(o..o + 4)?.try_into().ok()?)))
    {
        field_widget::info_label(ui, "Wage", format!("£{wage} p/w"));
    }
    if let Some(s) = fields::offset_of("s_contract_expiry")
        .and_then(|o| crate::ui::player_panel::fmt_date_at(cr, o))
    {
        field_widget::info_label(ui, "Expiry", s);
    }
}
