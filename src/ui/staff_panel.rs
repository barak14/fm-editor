//! Staff detail panel with a persistent identity header and scrollable details.

use eframe::egui;

use crate::bridge::StaffView;
use crate::fields::{Category, Entity};
use crate::ui::entity_header;
use crate::ui::field_widget::{self, FieldEdit};

#[derive(PartialEq)]
pub enum StaffTab {
    Overview,
    Contract,
}

pub fn show(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    staff: &mut StaffView,
    tab: &mut StaffTab,
) -> Vec<FieldEdit> {
    let mut edits = Vec::new();
    header(schemas, ui, staff, "Staff");

    let has_contract = staff.contract_raw.is_some();
    if *tab == StaffTab::Contract && !has_contract {
        *tab = StaffTab::Overview;
    }
    ui.horizontal(|ui| {
        ui.selectable_value(tab, StaffTab::Overview, "Overview");
        if has_contract {
            ui.selectable_value(tab, StaffTab::Contract, "Contract");
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| match tab {
        StaffTab::Overview => show_overview(schemas, ui, staff, &mut edits),
        StaffTab::Contract => show_contract(schemas, ui, staff, &mut edits),
    });

    edits
}

fn header(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    staff: &StaffView,
    kind: &str,
) {
    entity_header::show(
        ui,
        staff.name.as_deref().unwrap_or("Unknown"),
        kind,
        staff.record.id(),
        entity_header::age(schemas, &staff.record.raw, "s_dob"),
        staff.club_name.as_deref(),
        staff.club_addr,
        staff.nationality.as_deref(),
    );
}

fn identity(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    staff: &mut StaffView,
    edits: &mut Vec<FieldEdit>,
) {
    ui.strong("Identity");
    if let Some(dob) = schemas.field("s_dob") {
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
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    staff: &mut StaffView,
    groups: &[(&str, Category)],
    edits: &mut Vec<FieldEdit>,
) {
    for &(heading, category) in groups {
        field_widget::category_column(
            schemas,
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

fn roles(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    staff: &StaffView,
    skip_zeros: bool,
) {
    if let Some(roles) = &staff.roles {
        field_widget::roles_column(schemas, ui, "Roles (read-only)", roles, skip_zeros);
    } else {
        ui.strong("Roles (read-only)");
        ui.label("No role sub-record.");
    }
}

fn show_overview(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    staff: &mut StaffView,
    edits: &mut Vec<FieldEdit>,
) {
    let count = entity_header::column_count(ui.available_width());
    for chunk in [0, 1, 2].chunks(count) {
        ui.columns(chunk.len(), |cols| {
            for (col, group) in cols.iter_mut().zip(chunk) {
                match group {
                    0 => identity(schemas, col, staff, edits),
                    1 => categories(
                        schemas,
                        col,
                        staff,
                        &[("Ability", Category::Ability)],
                        edits,
                    ),
                    _ => categories(
                        schemas,
                        col,
                        staff,
                        &[("Reputation", Category::Reputation)],
                        edits,
                    ),
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
                categories(schemas, col, staff, groups[group], edits);
                if group == 3 {
                    roles(schemas, col, staff, true);
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
pub fn show_manager(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    staff: &mut StaffView,
) -> Vec<FieldEdit> {
    let mut edits = Vec::new();
    let addr = staff.record.addr;
    header(schemas, ui, staff, "Human Manager");
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
                        identity(schemas, col, staff, &mut edits);
                    }
                    categories(schemas, col, staff, groups[group], &mut edits);
                    if group == 0 {
                        if staff.contract_raw.is_some() {
                            show_contract(schemas, col, staff, &mut edits);
                            col.add_space(4.0);
                        }
                        roles(schemas, col, staff, false);
                    }
                }
            });
        }
    });

    // Redirect only identity fields stored at the human manager's shifted offsets.
    let schema = schemas;
    for edit in &mut edits {
        let rel = edit.addr.wrapping_sub(addr);
        if schema.is_manager_shifted_offset(rel) {
            edit.addr = edit.addr.wrapping_add(schema.human_manager_shift as u64);
        }
    }

    edits
}

fn show_contract(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    staff: &mut StaffView,
    edits: &mut Vec<FieldEdit>,
) {
    ui.strong("Contract");
    contract_rows(schemas, ui, staff, edits);
}

/// Job and club (read-only), wage and expiry (editable) from the staff
/// contract sub-record. Edits address the contract itself, so the human
/// manager's shifted-identity redirect in `show_manager` leaves them alone.
fn contract_rows(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    staff: &mut StaffView,
    edits: &mut Vec<FieldEdit>,
) {
    let (Some(cr), Some(caddr)) = (staff.contract_raw.as_mut(), staff.contract_addr) else {
        return;
    };
    if let Some(job) = schemas.offset_of("s_job").and_then(|o| cr.get(o).copied()) {
        let text = match crate::staff::job_name(job) {
            Some(name) => format!("{name} ({job})"),
            None => format!("code {job}"),
        };
        field_widget::info_label(ui, "Job", text);
    }
    if let Some(club) = &staff.club_name {
        field_widget::info_label(ui, "Club", club);
    }
    if let Some(f) = schemas.field("s_weekly_wage") {
        edits.extend(field_widget::raw_u32_edit(ui, f, cr, caddr));
    }
    if let Some(f) = schemas.field("s_contract_expiry") {
        edits.extend(field_widget::date_edit(ui, "Expiry", cr, f, caddr, 2000..=2060));
    }
}
