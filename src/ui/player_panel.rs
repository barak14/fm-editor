//! Player detail panel with identity, attributes, positions, traits and contract data.

use eframe::egui;

use crate::fields::{self, Category, Entity};
use crate::player::{self, Absence, GameDate, Injury, ListState, PlayerRecord};
use crate::ratings;
use crate::settings::{format_wage, WagePeriod};
use crate::ui::field_widget::{self, FieldEdit};

#[derive(PartialEq)]
pub enum PlayerTab {
    Overview,
    PositionsTraits,
    Contract,
}

pub fn show(
    schemas: &'static crate::fields::VersionedSchemas,
    today: Option<crate::game_date::GameDate>,
    ui: &mut egui::Ui,
    player: &mut PlayerRecord,
    tab: &mut PlayerTab,
    addr: u64,
    wage_period: WagePeriod,
    face_pack_path: Option<&str>,
) -> Vec<FieldEdit> {
    let mut edits = Vec::new();
    crate::ui::entity_header::show(
        ui,
        player.name.as_deref().unwrap_or("Unknown"),
        "Player",
        player
            .field_u32("p_uid")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "Unknown".into()),
        crate::ui::entity_header::age(schemas, today, &player.raw, "p_dob"),
        player.club_name.as_deref(),
        player.club_addr,
        player.nationality.as_deref(),
    );
    if *tab == PlayerTab::Contract && player.contract_raw.is_none() {
        *tab = PlayerTab::Overview;
    }

    // Sub-tab bar
    ui.horizontal(|ui| {
        ui.selectable_value(tab, PlayerTab::Overview, "Overview");
        ui.selectable_value(tab, PlayerTab::PositionsTraits, "Positions & Traits");
        if player.contract_raw.is_some() {
            ui.selectable_value(tab, PlayerTab::Contract, "Contract");
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| match tab {
        PlayerTab::Overview => show_overview(
            schemas,
            ui,
            player,
            addr,
            &mut edits,
            wage_period,
            face_pack_path,
        ),
        PlayerTab::PositionsTraits => show_positions_traits(schemas, ui, player, addr, &mut edits),
        PlayerTab::Contract => show_contract(schemas, ui, player, &mut edits),
    });

    // Widgets address edits as `addr + schema offset`. A player-coach's person
    // block sits `person_shift` bytes later in memory, so move edits that land
    // in it (DOB, traits, ...) to where the game stores them.
    if player.person_shift != 0 {
        let size = schemas.player_struct_size as u64;
        for edit in &mut edits {
            let rel = edit.addr.wrapping_sub(addr);
            if rel < size {
                edit.addr = addr + player::player_phys_offset(schemas, rel, player.person_shift);
            }
        }
    }

    edits
}

// ---------------------------------------------------------------------------
// Overview
// ---------------------------------------------------------------------------

fn show_overview(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    player: &mut PlayerRecord,
    addr: u64,
    edits: &mut Vec<FieldEdit>,
    wage_period: WagePeriod,
    face_pack_path: Option<&str>,
) {
    // Look up fields once from the schema; passing them by reference into
    // the widget helpers avoids every panel line hardcoding an offset.
    let p_dob = schemas.field("p_dob").unwrap();
    let p_ca = schemas.field("p_ca").unwrap();
    let p_pa = schemas.field("p_pa").unwrap();
    let p_current_rep = schemas.field("p_current_rep").unwrap();
    let p_home_rep = schemas.field("p_home_rep").unwrap();
    let p_world_rep = schemas.field("p_world_rep").unwrap();

    // Face image — shown when a face pack path is configured and the UID is known.
    let face_uid = player.field_u32("p_uid").filter(|&uid| uid > 0);
    if let (Some(path), Some(uid)) = (face_pack_path, face_uid) {
        let img_path = std::path::Path::new(path).join(format!("{uid}.png"));
        let uri = format!("file:///{}", img_path.to_string_lossy().replace('\\', "/"));
        ui.add(egui::Image::new(uri).max_size(egui::vec2(80.0, 80.0)));
        ui.add_space(4.0);
    }

    // Top info row: Personal | Ability | Reputation
    ui.horizontal_wrapped(|ui| {
        // Personal Details
        ui.vertical(|ui| {
            ui.strong("Identity");
            edits.extend(field_widget::dob_edit(ui, &mut player.raw, &p_dob, addr));
            if let Some(f) = schemas.field("p_height") {
                edits.extend(field_widget::raw_u16_edit(
                    ui,
                    &f,
                    &mut player.raw,
                    250,
                    addr,
                ));
            }
            if let Some(birthplace) = &player.birthplace {
                field_widget::info_label(ui, "Place of Birth", birthplace);
            }
        });

        // Appearance (shown in the 3D match), when the schema maps it.
        if schemas.offset_of("p_skin_tone").is_some() {
            ui.separator();
            ui.vertical(|ui| {
                ui.strong("Appearance");
                if let Some(f) = schemas.field("p_ethnicity") {
                    edits.extend(field_widget::choice_u8_edit(
                        ui,
                        "Ethnicity",
                        &mut player.raw,
                        &f,
                        addr,
                        ETHNICITIES,
                    ));
                }
                for (key, max) in [
                    ("p_skin_tone", 19),
                    ("p_hair_colour", 18),
                    ("p_hair_length", 6),
                ] {
                    if let Some(f) = schemas.field(key) {
                        edits.extend(field_widget::raw_u8_edit(
                            ui,
                            &f,
                            &mut player.raw,
                            max,
                            addr,
                        ));
                    }
                }
            });
        }

        ui.separator();

        // Ability
        ui.vertical(|ui| {
            ui.strong("Ability");
            edits.extend(field_widget::ca_pa_edit(
                ui,
                &mut player.raw,
                &p_ca,
                &p_pa,
                addr,
            ));
        });

        ui.separator();

        // Reputation
        ui.vertical(|ui| {
            ui.strong("Reputation");
            if let Some(e) =
                field_widget::raw_u16_edit(ui, &p_current_rep, &mut player.raw, 10000, addr)
            {
                edits.push(e);
            }
            if let Some(e) =
                field_widget::raw_u16_edit(ui, &p_home_rep, &mut player.raw, 10000, addr)
            {
                edits.push(e);
            }
            if let Some(e) =
                field_widget::raw_u16_edit(ui, &p_world_rep, &mut player.raw, 10000, addr)
            {
                edits.push(e);
            }
        });

        ui.separator();

        // Stats
        ui.vertical(|ui| {
            ui.strong("Stats");
            // Each row shows only when the record's schema maps the field.
            if let Some(apps) = player.field_u16("p_senior_apps") {
                field_widget::info_label(ui, "Snr Apps", apps);
            }
            if let (Some(caps), Some(goals)) = (
                player.field_int("p_intl_caps"),
                player.field_int("p_intl_goals"),
            ) {
                field_widget::info_label(ui, "Intl Caps / Goals", format!("{caps} / {goals}"));
            }
            if let (Some(caps), Some(goals)) = (
                player.field_int("p_u21_caps"),
                player.field_int("p_u21_goals"),
            ) {
                field_widget::info_label(ui, "U21 Caps / Goals", format!("{caps} / {goals}"));
            }
            // FM27 has no struct-level weekly wage; fall back to the contract's.
            if let Some(wage) = player
                .field_u32("p_weekly_wage")
                .or_else(|| player.field_u32("p_contract_wage"))
            {
                field_widget::info_label(ui, "Wage", format_wage(wage, wage_period));
            }
        });
    });

    show_player_status(schemas, ui, player, addr, edits);
    show_nationality_ties(schemas, ui, player);

    ui.separator();

    for groups in [
        [
            ("Technical", Category::Technical),
            ("Mental", Category::Mental),
            ("Physical", Category::Physical),
            ("Hidden", Category::Hidden),
        ],
        [
            ("Set Pieces", Category::SetPieces),
            ("Goalkeeping", Category::Goalkeeping),
            ("Footedness", Category::Footedness),
            ("Personality", Category::Personality),
        ],
    ] {
        crate::ui::entity_header::columns(ui, |index, col| {
            let (heading, category) = groups[index];
            field_widget::category_column(
                schemas,
                col,
                heading,
                Entity::Player,
                category,
                &mut player.raw,
                addr,
                edits,
            );
        });
    }
}

// ---------------------------------------------------------------------------
// Positions & Traits
// ---------------------------------------------------------------------------

fn show_positions_traits(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    player: &mut PlayerRecord,
    addr: u64,
    edits: &mut Vec<FieldEdit>,
) {
    // Stack the formation above traits on narrow panels rather than forcing overflow.
    ui.strong("Positions");
    show_positions_formation(schemas, ui, player, addr, edits);
    ui.add_space(12.0);
    ui.strong("Traits");
    show_traits_checkboxes(schemas, ui, player, addr, edits);

    ui.add_space(16.0);
    ui.separator();

    // PA projection — Genie Scout-style, read-only preview. Hidden behind
    // a checkbox so it doesn't clutter the default view. Pure display —
    // never emits FieldEdits / writes to memory.
    let checkbox_id = egui::Id::new("pa_extrap_toggle");
    let mut show_extrap = ui.data_mut(|d| d.get_temp::<bool>(checkbox_id).unwrap_or(false));
    if ui
        .checkbox(&mut show_extrap, "Show PA extrapolation")
        .changed()
    {
        ui.data_mut(|d| d.insert_temp(checkbox_id, show_extrap));
    }

    if show_extrap {
        let ca = player.field_u8("p_ca").unwrap_or(0);
        let pa = player.field_u8("p_pa").unwrap_or(0);
        let projection = ratings::extrapolate_player(player);
        if projection.is_empty() {
            ui.label(
                egui::RichText::new(if pa <= ca {
                    "No growth \u{2014} CA is already at PA."
                } else {
                    "No attributes would grow under the current tier table."
                })
                .small()
                .color(egui::Color32::from_rgb(150, 150, 150)),
            );
        } else {
            ui.strong(format!("PA Projection (CA={ca} \u{2192} PA={pa})"));
            ui.label(
                egui::RichText::new("Read-only preview \u{2014} no changes written to memory")
                    .small()
                    .color(egui::Color32::from_rgb(150, 150, 150)),
            );
            for cat in [
                Category::Technical,
                Category::SetPieces,
                Category::Mental,
                Category::Physical,
                Category::Goalkeeping,
                Category::Hidden,
            ] {
                let cat_rows: Vec<_> = schemas
                    .fields_for_category(Entity::Player, cat)
                    .into_iter()
                    .filter_map(|f| {
                        projection
                            .iter()
                            .find(|(k, _, _)| *k == f.key)
                            .map(|(_, old, new)| (f.label, *old, *new))
                    })
                    .collect();
                if cat_rows.is_empty() {
                    continue;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{cat:?}:")).strong().small());
                    for (label, old, new) in cat_rows {
                        ui.label(
                            egui::RichText::new(format!("{label} {old}\u{2192}{new}"))
                                .small()
                                .color(egui::Color32::LIGHT_GREEN),
                        );
                    }
                });
            }
        }
    }
}

/// Render the 15 position familiarities in a football-formation layout.
/// Layout (3 columns per row):
///   row 0: .   ST  .
///   row 1: AML AMC AMR
///   row 2: ML  MC  MR
///   row 3: WBL DM  WBR
///   row 4: DL  DC  DR
///   row 5: .   SW  .
///   row 6: .   GK  .
fn show_positions_formation(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    player: &mut PlayerRecord,
    addr: u64,
    edits: &mut Vec<FieldEdit>,
) {
    // Rows of (left, center, right) position keys. None = empty cell.
    let rows: &[(Option<&str>, Option<&str>, Option<&str>)] = &[
        (None, Some("pos_st"), None),
        (Some("pos_aml"), Some("pos_amc"), Some("pos_amr")),
        (Some("pos_ml"), Some("pos_mc"), Some("pos_mr")),
        (Some("pos_wbl"), Some("pos_dm"), Some("pos_wbr")),
        (Some("pos_dl"), Some("pos_dc"), Some("pos_dr")),
        (None, Some("pos_sw"), None),
        (None, Some("pos_gk"), None),
    ];

    egui::Grid::new("pos_formation")
        .num_columns(3)
        .spacing([14.0, 6.0])
        .min_col_width(56.0)
        .show(ui, |ui| {
            for (left, center, right) in rows {
                pos_cell(schemas, ui, *left, player, addr, edits);
                pos_cell(schemas, ui, *center, player, addr, edits);
                pos_cell(schemas, ui, *right, player, addr, edits);
                ui.end_row();
            }
        });
}

fn pos_cell(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    key: Option<&str>,
    player: &mut PlayerRecord,
    addr: u64,
    edits: &mut Vec<FieldEdit>,
) {
    match key {
        Some(k) => {
            if let Some(f) = schemas.field(k) {
                if let Some(edit) = field_widget::attr_cell_compact(ui, &f, &mut player.raw, addr) {
                    edits.push(edit);
                }
            }
        }
        None => {
            // Empty cell — reserve space so the grid stays aligned.
            ui.allocate_space(egui::vec2(56.0, 38.0));
        }
    }
}

/// Render traits as checkbox groups arranged in 4 columns to mirror the FM26 UI.
fn show_traits_checkboxes(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    player: &mut PlayerRecord,
    addr: u64,
    edits: &mut Vec<FieldEdit>,
) {
    let location = schemas.field("p_traits").and_then(|field| field.offset);
    let Some((_, old_mask)) = trait_value(&player.raw, location) else {
        ui.label("Traits unavailable for this record or game version.");
        return;
    };
    let mut new_mask = old_mask;

    // Column 1
    const COL1: &[(&str, &[u8])] = &[
        ("General", &[25, 14, 18, 17, 42, 45, 19]),
        ("Defending", &[16, 38, 39, 34, 61, 33]),
        ("Ball Control", &[35, 31, 12, 15, 23, 27]),
    ];
    // Column 2
    const COL2: &[(&str, &[u8])] = &[
        ("Shooting", &[10, 9, 11, 43, 22, 24, 57]),
        ("Movement", &[5, 4, 6, 32, 20, 41, 55]),
        ("Set Pieces", &[51, 56, 37, 46, 62, 63]),
    ];
    // Column 3
    const COL3: &[(&str, &[u8])] = &[(
        "Dribbling / Running",
        &[52, 53, 2, 1, 3, 28, 36, 47, 29, 30, 40, 59, 58, 13],
    )];
    // Column 4
    const COL4: &[(&str, &[u8])] = &[("Passing", &[7, 26, 8, 48, 21, 44, 60, 64])];

    let groups = [COL1, COL2, COL3, COL4];
    crate::ui::entity_header::columns(ui, |index, col| {
        render_trait_column(col, groups[index], &mut new_mask);
    });

    if crate::ui::navigation::editing_enabled(ui) && new_mask != old_mask {
        // Re-resolve the mapping and validate the whole range before copying bytes.
        let location = schemas.field("p_traits").and_then(|field| field.offset);
        let Some((traits_offset, current_mask)) = trait_value(&player.raw, location) else {
            return;
        };
        if current_mask != old_mask {
            return;
        }
        let old_bytes = old_mask.to_le_bytes();
        let new_bytes = new_mask.to_le_bytes();
        let Some(bytes) = player.raw.get_mut(traits_offset..traits_offset + 8) else {
            return;
        };
        bytes.copy_from_slice(&new_bytes);

        let mut added = Vec::new();
        let mut removed = Vec::new();
        for bit in 0..64u8 {
            let o = (old_mask >> bit) & 1;
            let n = (new_mask >> bit) & 1;
            if o != n {
                let name = player::ppm_name(bit + 1);
                if n == 1 {
                    added.push(name);
                } else {
                    removed.push(name);
                }
            }
        }
        let desc = match (added.as_slice(), removed.as_slice()) {
            ([a], []) => format!("Added trait: {a}"),
            ([], [r]) => format!("Removed trait: {r}"),
            (a, r) => format!("Traits: +{} -{}", a.len(), r.len()),
        };

        edits.push(FieldEdit {
            addr: addr + traits_offset as u64,
            old_raw: old_bytes.to_vec(),
            new_raw: new_bytes.to_vec(),
            description: desc,
        });
    }
}

fn render_trait_column(ui: &mut egui::Ui, groups: &[(&str, &[u8])], mask: &mut u64) {
    for (heading, ppm_ids) in groups {
        ui.strong(*heading);
        for &id in *ppm_ids {
            let bit = 1u64 << (id - 1);
            let mut checked = (*mask & bit) != 0;
            if ui
                .add_enabled(
                    crate::ui::navigation::editing_enabled(ui),
                    egui::Checkbox::new(&mut checked, player::ppm_name(id)),
                )
                .changed()
            {
                if checked {
                    *mask |= bit;
                } else {
                    *mask &= !bit;
                }
            }
        }
        ui.add_space(6.0);
    }
}

// ---------------------------------------------------------------------------
// Contract / Relations (placeholder)
// ---------------------------------------------------------------------------

fn show_contract(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    player: &mut PlayerRecord,
    edits: &mut Vec<FieldEdit>,
) {
    ui.strong("Contract");

    // Contract fields live in the contract sub-record: edit its bytes and
    // address. Asking price is in the player struct itself.
    if let (Some(cr), Some(caddr)) = (player.contract_raw.as_mut(), player.contract_addr) {
        if let Some(f) = schemas.field("p_contract_wage") {
            edits.extend(field_widget::raw_u32_edit(ui, &f, cr, caddr));
        }
        if let Some(f) = schemas.field("p_contract_expiry") {
            edits.extend(field_widget::date_edit(
                ui,
                "Expiry",
                cr,
                &f,
                caddr,
                2000..=2060,
            ));
        }
        if let Some(f) = schemas.field("p_squad_status") {
            let code = schemas
                .offset_of("p_squad_status")
                .and_then(|o| cr.get(o).copied());
            match code {
                // FM's own statuses (Out On Loan, ...) are shown, not offered.
                Some(c) if !SQUAD_STATUS_OFFERED.iter().any(|(o, _)| *o == c) => {
                    let name =
                        squad_status_name(c).map_or(format!("code {c}"), |n| format!("{n} ({c})"));
                    field_widget::info_label(ui, "Squad Status", name);
                }
                _ => edits.extend(field_widget::choice_u8_edit(
                    ui,
                    "Squad Status",
                    cr,
                    &f,
                    caddr,
                    SQUAD_STATUS_OFFERED,
                )),
            }
        }
        if let Some(f) = schemas.field("p_squad_number") {
            edits.extend(field_widget::optional_u8_edit(
                ui,
                "Squad Number",
                cr,
                &f,
                caddr,
                99,
                255,
            ));
        }
        show_listing(schemas, ui, cr, caddr, player.game_date, edits);
        if let Some(s) = schemas
            .offset_of("p_contract_signed")
            .and_then(|o| fmt_date_at(cr, o))
        {
            field_widget::info_label(ui, "Signed", s);
        }
    } else {
        ui.label("Contract sub-record unavailable.");
    }
    if let Some(f) = schemas.field("p_asking_price") {
        let addr = player.addr;
        edits.extend(field_widget::optional_money_edit(
            ui,
            "Asking Price",
            &mut player.raw,
            &f,
            addr,
        ));
    }

    ui.add_space(4.0);

    if let Some(apps) = player.field_u16("p_senior_apps") {
        field_widget::info_label(ui, "Senior Apps", apps);
    }

    for (key, label) in [
        ("p_contract_start", "Contract Start"),
        ("p_join_date", "Joined Club"),
        ("p_debut_date", "Senior Debut"),
    ] {
        if let Some(s) = fmt_date_field(schemas, &player.raw, key) {
            field_widget::info_label(ui, label, s);
        }
    }
    show_clauses(schemas, ui, player, edits);
}

/// Medical/list data is loaded by the bridge, never inferred from a cached search row.
fn show_player_status(
    schemas: &'static fields::VersionedSchemas,
    ui: &mut egui::Ui,
    player: &mut PlayerRecord,
    addr: u64,
    edits: &mut Vec<FieldEdit>,
) {
    let Some(layout) = schemas.player_lists else {
        return;
    };
    ui.separator();
    ui.strong("Status");
    if let Some(field) = schemas.field("p_morale").filter(|f| f.offset.is_some()) {
        let labels: Vec<_> = (1..=20).map(|n| (n, schemas.morale_label(n))).collect();
        let options: Vec<_> = labels
            .iter()
            .map(|(n, label)| (*n, label.as_str()))
            .collect();
        edits.extend(field_widget::choice_u8_edit(
            ui,
            "Morale",
            &mut player.raw,
            field,
            addr,
            &options,
        ));
    }
    if let Some(off) = schemas.offset_of("p_value") {
        if let Some(bytes) = player.raw.get(off..off.saturating_add(4)) {
            if let (Ok(bytes), Some(at)) = (bytes.try_into(), addr.checked_add(off as u64)) {
                if let Some(edit) = field_widget::money_edit_at(
                    ui,
                    "Transfer Value",
                    i32::from_le_bytes(bytes),
                    at,
                    false,
                ) {
                    player.raw[off..off + 4].copy_from_slice(&edit.new_raw);
                    edits.push(edit);
                }
            }
        }
    }
    ui.strong("Injuries");
    match &player.injuries {
        ListState::Unmapped => {
            ui.label("Not loaded.");
        }
        ListState::Unreadable => {
            ui.label("Injury data unavailable (read-only).");
        }
        ListState::Items(items) if items.is_empty() => {
            ui.label("None");
        }
        ListState::Items(items) => {
            for injury in items {
                ui.push_id(("injury", injury.addr), |ui| {
                    ui.label(&injury.name);
                    field_widget::info_label(ui, "Injured on", injury.start.display());
                    field_widget::info_label(
                        ui,
                        "Back in light training",
                        injury.start.add_days(injury.light_days).display(),
                    );
                    field_widget::info_label(
                        ui,
                        "Back in full training",
                        injury.start.add_days(injury.full_days).display(),
                    );
                    let edit = heal_edit(injury, player.game_date);
                    if ui
                        .add_enabled(can_edit(ui) && edit.is_some(), egui::Button::new("Heal"))
                        .clicked()
                    {
                        edits.extend(edit);
                    }
                });
            }
        }
    }
    ui.strong("Unavailability");
    match &player.absences {
        ListState::Unmapped => {
            ui.label("Not loaded.");
        }
        ListState::Unreadable => {
            ui.label("Unavailability data unavailable (read-only).");
        }
        ListState::Items(items) if items.is_empty() => {
            ui.label("None");
        }
        ListState::Items(items) => {
            for absence in items {
                ui.push_id(("absence", absence.addr), |ui| {
                    let until = (absence.days != u16::MAX)
                        .then(|| absence.start.add_days(absence.days).display());
                    if absence.kind == layout.absence_ban {
                        ui.label(until.map_or_else(
                            || "Suspended (not date-based)".into(),
                            |end| format!("Suspended from {} until {end}", absence.start.display()),
                        ));
                        let edit = lift_ban_edit(absence, player.game_date);
                        if ui
                            .add_enabled(
                                can_edit(ui) && edit.is_some(),
                                egui::Button::new("Lift ban"),
                            )
                            .clicked()
                        {
                            edits.extend(edit);
                        }
                    } else if absence.kind == layout.absence_holiday {
                        ui.label(until.map_or_else(
                            || "On holiday (not date-based)".into(),
                            |end| format!("On holiday until {end}"),
                        ));
                    } else {
                        let label = layout
                            .absence_kinds
                            .iter()
                            .find(|(kind, _)| *kind == absence.kind)
                            .map_or_else(
                                || format!("Unavailable (code {})", absence.kind),
                                |(_, name)| name.to_string(),
                            );
                        ui.label(until.map_or_else(
                            || format!("{label} (not date-based)"),
                            |end| format!("{label} from {} until {end}", absence.start.display()),
                        ));
                    }
                });
            }
        }
    }
}

fn can_edit(ui: &egui::Ui) -> bool {
    ui.is_enabled() && crate::ui::navigation::editing_enabled(ui)
}

fn heal_edit(injury: &Injury, date: Option<GameDate>) -> Option<FieldEdit> {
    let elapsed = date?.elapsed_since(injury.start);
    if elapsed >= injury.full_days {
        return None;
    }
    Some(FieldEdit {
        addr: injury.days_addr,
        old_raw: [
            injury.full_days.to_le_bytes(),
            injury.light_days.to_le_bytes(),
        ]
        .concat(),
        new_raw: [
            elapsed.to_le_bytes(),
            injury.light_days.min(elapsed).to_le_bytes(),
        ]
        .concat(),
        description: format!("Heal injury: {}", injury.name),
    })
}

fn lift_ban_edit(absence: &Absence, date: Option<GameDate>) -> Option<FieldEdit> {
    if absence.days == u16::MAX {
        return None;
    }
    let elapsed = date?.elapsed_since(absence.start);
    if elapsed >= absence.days {
        return None;
    }
    Some(FieldEdit {
        addr: absence.days_addr,
        old_raw: absence.days.to_le_bytes().to_vec(),
        new_raw: elapsed.to_le_bytes().to_vec(),
        description: "Lift ban".into(),
    })
}

fn show_nationality_ties(
    schemas: &'static fields::VersionedSchemas,
    ui: &mut egui::Ui,
    player: &PlayerRecord,
) {
    let Some(layout) = schemas.player_lists else {
        return;
    };
    ui.separator();
    ui.strong("Nationality & ties");
    match &player.relations {
        ListState::Unmapped => {
            ui.label("Not loaded.");
        }
        ListState::Unreadable => {
            ui.label("Relations unavailable (read-only).");
        }
        ListState::Items(items) => {
            let nations: Vec<_> = items
                .iter()
                .filter(|r| {
                    r.kind == layout.kind_nation && r.relation_type == layout.type_other_nationality
                })
                .map(|r| r.name.as_str())
                .collect();
            field_widget::info_label(
                ui,
                "Other nationalities",
                if nations.is_empty() {
                    "None".into()
                } else {
                    nations.join(", ")
                },
            );
            let clubs: Vec<_> = items
                .iter()
                .filter(|r| {
                    r.kind == layout.kind_club && r.relation_type == layout.type_favoured_club
                })
                .collect();
            if clubs.is_empty() {
                field_widget::info_label(ui, "Favoured clubs", "None");
            } else {
                ui.label("Favoured clubs:");
                for club in clubs {
                    let reason = layout
                        .favoured_club_reasons
                        .iter()
                        .find(|(code, _)| *code == club.reason)
                        .map_or_else(
                            || format!("Reason {}", club.reason),
                            |(_, name)| name.to_string(),
                        );
                    ui.push_id(("favoured_club", club.addr), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            crate::ui::navigation::club_link(
                                ui,
                                Some(&club.name),
                                Some(club.target_addr),
                            );
                            ui.label(format!("({reason})"))
                                .on_hover_text(format!("Strength: {}", club.strength));
                        });
                    });
                }
            }
        }
    }
}

fn contract_offset(schemas: &'static fields::VersionedSchemas, key: &str) -> Option<usize> {
    match schemas.field(key)?.offset? {
        fields::FieldLocation::SubRecord {
            container: "contract",
            offset,
            ..
        } => usize::try_from(offset).ok(),
        _ => None,
    }
}

fn transfer_listing_edit(
    schemas: &'static fields::VersionedSchemas,
    raw: &[u8],
    base: u64,
    listed: bool,
    date: Option<GameDate>,
) -> Option<FieldEdit> {
    let date = date?;
    let start = contract_offset(schemas, "p_list_date")?;
    let flags = contract_offset(schemas, "p_list_flags")?;
    let code = contract_offset(schemas, "p_list_code")?;
    let end = code.checked_add(1)?;
    if flags < start.checked_add(4)? || flags >= code {
        return None;
    }
    let old = raw.get(start..end)?;
    if (raw[flags] & 1 != 0) == listed {
        return None;
    }
    let mut new = old.to_vec();
    new[..4].copy_from_slice(&if listed { date.raw } else { 0x076C0001u32 }.to_le_bytes());
    new[flags - start] = if listed {
        raw[flags] | 1
    } else {
        raw[flags] & !1
    };
    new[code - start] = if listed { 5 } else { 0 };
    Some(FieldEdit {
        addr: base.checked_add(start as u64)?,
        old_raw: old.to_vec(),
        new_raw: new,
        description: format!("Transfer listed: {listed}"),
    })
}

fn show_listing(
    schemas: &'static fields::VersionedSchemas,
    ui: &mut egui::Ui,
    raw: &mut [u8],
    base: u64,
    date: Option<GameDate>,
    edits: &mut Vec<FieldEdit>,
) {
    let Some(flags_off) = contract_offset(schemas, "p_list_flags") else {
        return;
    };
    let Some(&flags) = raw.get(flags_off) else {
        return;
    };
    // The additional date/code fields distinguish verified editable listings
    // from older schemas whose flag meanings have not been verified.
    if contract_offset(schemas, "p_list_date").is_none()
        || contract_offset(schemas, "p_list_code").is_none()
    {
        return;
    }
    let mut transfer = flags & 1 != 0;
    if ui
        .add_enabled(
            can_edit(ui) && date.is_some(),
            egui::Checkbox::new(&mut transfer, "Transfer listed"),
        )
        .changed()
    {
        if let Some(edit) = transfer_listing_edit(schemas, raw, base, transfer, date) {
            let start = (edit.addr - base) as usize;
            raw[start..start + edit.new_raw.len()].copy_from_slice(&edit.new_raw);
            edits.push(edit);
        }
    }
    let flags = raw[flags_off];
    let mut loan = flags & 2 != 0;
    if ui
        .add_enabled(can_edit(ui), egui::Checkbox::new(&mut loan, "Loan listed"))
        .changed()
    {
        let new = if loan { flags | 2 } else { flags & !2 };
        if let Some(addr) = base.checked_add(flags_off as u64) {
            raw[flags_off] = new;
            edits.push(FieldEdit {
                addr,
                old_raw: vec![flags],
                new_raw: vec![new],
                description: format!("Loan listed: {loan}"),
            });
        }
    }
    if raw[flags_off] & 1 != 0 {
        if let Some(date) =
            contract_offset(schemas, "p_list_date").and_then(|off| fmt_date_at(raw, off))
        {
            field_widget::info_label(ui, "Listed since", date);
        }
    }
}

fn show_clauses(
    schemas: &'static fields::VersionedSchemas,
    ui: &mut egui::Ui,
    player: &PlayerRecord,
    edits: &mut Vec<FieldEdit>,
) {
    let Some(layout) = schemas.player_lists else {
        return;
    };
    ui.separator();
    ui.strong("Bonuses & clauses");
    match &player.clauses {
        ListState::Unmapped => {
            ui.label("Not loaded.");
        }
        ListState::Unreadable => {
            ui.label("Clause data unavailable (read-only).");
        }
        ListState::Items(items) if items.is_empty() => {
            ui.label("None");
        }
        ListState::Items(items) => {
            for clause in items {
                let label = layout
                    .clause_types
                    .iter()
                    .find(|(code, _)| *code == clause.clause_type)
                    .map_or_else(
                        || format!("Clause type 0x{:02X}", clause.clause_type),
                        |(_, name)| name.to_string(),
                    );
                if layout.clause_amount_types.contains(&clause.clause_type) {
                    ui.push_id(("clause", clause.addr), |ui| {
                        edits.extend(field_widget::money_edit_at(
                            ui,
                            &label,
                            clause.amount,
                            clause.amount_addr,
                            false,
                        ));
                    });
                } else {
                    let value = if layout.clause_percent_types.contains(&clause.clause_type) {
                        format!("{}%", clause.value)
                    } else if layout.clause_year_types.contains(&clause.clause_type) {
                        format!(
                            "{} {}",
                            clause.value,
                            if clause.value == 1 { "year" } else { "years" }
                        )
                    } else {
                        format!("amount={}, value={}", clause.amount, clause.value)
                    };
                    field_widget::info_label(ui, &label, value);
                }
            }
        }
    }
}

/// Squad statuses offered for editing: one code per distinct label. Codes
/// 6, 8, 19 and 21 duplicate a label; 0 ("-"), 23 (Scouting Required) and
/// 24 (Out On Loan) are FM's own and not offered. A stored code outside
/// this list is shown read-only.
const SQUAD_STATUS_OFFERED: &[(u8, &str)] = &[
    (1, "Star Player"),
    (2, "Important Player"),
    (3, "Regular Starter"),
    (4, "Squad Player"),
    (5, "Impact Sub"),
    (7, "Fringe Player"),
    (9, "Emergency Backup"),
    (10, "Breakthrough Prospect"),
    (11, "Future Prospect"),
    (12, "Hot Prospect"),
    (13, "Youngster"),
    (14, "Regular"),
    (15, "First-Choice Goalkeeper"),
    (16, "Cup Goalkeeper"),
    (17, "Domestic Cup Goalkeeper"),
    (18, "Continental Cup Goalkeeper"),
    (20, "Backup"),
    (22, "Surplus to Requirements"),
];

/// FM's ethnicity codes (`p_ethnicity`).
const ETHNICITIES: &[(u8, &str)] = &[
    (0, "Northern European"),
    (1, "Mediterranean/Hispanic"),
    (2, "North African/Middle Eastern"),
    (3, "African/Caribbean"),
    (4, "Asian"),
    (5, "South East Asian"),
    (6, "Pacific Islander"),
    (7, "Native American"),
    (8, "Native Australian"),
    (9, "Mixed Race"),
    (10, "East Asian"),
];

/// FM's label for a squad-status code (contract "agreed playing time").
/// Codes from FM's Squad Status list; several codes share a label.
fn squad_status_name(code: u8) -> Option<&'static str> {
    Some(match code {
        1 => "Star Player",
        2 => "Important Player",
        3 => "Regular Starter",
        4 | 6 => "Squad Player",
        5 => "Impact Sub",
        7 | 8 => "Fringe Player",
        9 | 21 => "Emergency Backup",
        10 => "Breakthrough Prospect",
        11 => "Future Prospect",
        12 => "Hot Prospect",
        13 => "Youngster",
        14 => "Regular",
        15 => "First-Choice Goalkeeper",
        16 | 19 => "Cup Goalkeeper",
        17 => "Domestic Cup Goalkeeper",
        18 => "Continental Cup Goalkeeper",
        20 => "Backup",
        22 => "Surplus to Requirements",
        23 => "Scouting Required",
        24 => "Out On Loan",
        _ => return None,
    })
}

/// Read a DateDayYear field from `raw` by schema key and format as "DD Mon YYYY".
fn fmt_date_field(
    schemas: &'static crate::fields::VersionedSchemas,
    raw: &[u8],
    key: &str,
) -> Option<String> {
    let off = schemas.field(key)?.offset?.offset() as usize;
    fmt_date_at(raw, off)
}

/// Format a DateDayYear value at byte offset `off` in `buf` as "DD Mon YYYY".
pub(crate) fn fmt_date_at(buf: &[u8], off: usize) -> Option<String> {
    if off + 4 > buf.len() {
        return None;
    }
    // Low 9 bits hold the day; FM keeps flags above them.
    let doy = u16::from_le_bytes(buf[off..off + 2].try_into().ok()?) & 0x1FF;
    let year = u16::from_le_bytes(buf[off + 2..off + 4].try_into().ok()?);
    if year == 0 || doy == 0 || doy > 366 || !(1900..=2050).contains(&year) {
        return None;
    }
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let mdays: [u16; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mnames = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut rem = doy;
    for (i, &d) in mdays.iter().enumerate() {
        if rem <= d {
            return Some(format!("{rem} {} {year}", mnames[i]));
        }
        rem -= d;
    }
    None
}

/// Trait masks must be mapped directly into this buffer, never into a sub-record.
fn trait_value(raw: &[u8], location: Option<fields::FieldLocation>) -> Option<(usize, u64)> {
    let fields::FieldLocation::Direct { offset, .. } = location? else {
        return None;
    };
    let offset = usize::try_from(offset).ok()?;
    let bytes = raw.get(offset..offset.checked_add(8)?)?;
    Some((offset, u64::from_le_bytes(bytes.try_into().ok()?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture_player(build: crate::game_version::GameBuild) -> PlayerRecord {
        PlayerRecord {
            build,
            addr: 0x20000,
            index: 0,
            name: None,
            raw: vec![0; build.schemas().player_struct_size],
            person_shift: 0,
            contract_raw: None,
            contract_addr: None,
            club_name: None,
            club_addr: None,
            nationality: None,
            birthplace: None,
            game_date: None,
            injuries: ListState::Unmapped,
            absences: ListState::Unmapped,
            relations: ListState::Unmapped,
            clauses: ListState::Unmapped,
        }
    }

    #[test]
    fn heal_and_lift_ban_only_shorten_durations() {
        let start = GameDate::from_raw((2025u32 << 16) | 131).unwrap(); // 11 May
        let now = GameDate::from_raw(0x07EA1AC2).unwrap(); // 13 Jul 2026, flags retained
        let mut injury = Injury {
            addr: 0x30000,
            name: "a patella dislocation".into(),
            start,
            full_days: 498,
            light_days: 468,
            days_addr: 0x30028,
        };
        let edit = heal_edit(&injury, Some(now)).unwrap();
        assert_eq!(edit.addr, injury.days_addr);
        assert_eq!(
            edit.old_raw,
            [498u16.to_le_bytes(), 468u16.to_le_bytes()].concat()
        );
        assert_eq!(
            edit.new_raw,
            [428u16.to_le_bytes(), 428u16.to_le_bytes()].concat()
        );
        injury.light_days = 200;
        assert_eq!(
            heal_edit(&injury, Some(now)).unwrap().new_raw,
            [428u16.to_le_bytes(), 200u16.to_le_bytes()].concat()
        );
        assert!(heal_edit(&injury, None).is_none());
        injury.full_days = 428;
        assert!(heal_edit(&injury, Some(now)).is_none());
        injury.full_days = 498;
        assert_eq!(
            heal_edit(
                &injury,
                Some(GameDate::from_raw((2024u32 << 16) | 366).unwrap())
            )
            .unwrap()
            .new_raw,
            [0; 4]
        );
        let mut absence = Absence {
            addr: 0x40000,
            kind: 0x19,
            start,
            days: 595,
            days_addr: 0x40016,
        };
        assert_eq!(
            lift_ban_edit(&absence, Some(now)).unwrap().new_raw,
            428u16.to_le_bytes()
        );
        assert!(lift_ban_edit(&absence, None).is_none());
        absence.days = 428;
        assert!(lift_ban_edit(&absence, Some(now)).is_none());
        absence.days = u16::MAX;
        assert!(lift_ban_edit(&absence, Some(now)).is_none());
    }

    #[test]
    fn transfer_listing_bytes_preserve_flags_padding_and_raw_game_date() {
        let schemas = crate::game_version::GameBuild::Fm27Steam.schemas();
        let now = GameDate::from_raw(0x07EA1AC2).unwrap();
        let date = schemas.offset_of("p_list_date").unwrap();
        let flags = schemas.offset_of("p_list_flags").unwrap();
        let code = schemas.offset_of("p_list_code").unwrap();
        let mut raw = vec![0xA5; 0xC4];
        raw[flags] = 0xA2;
        let edit = transfer_listing_edit(schemas, &raw, 0x50000, true, Some(now)).unwrap();
        assert_eq!(edit.addr, 0x50000 + date as u64);
        assert_eq!(edit.new_raw.len(), code + 1 - date);
        assert_eq!(&edit.new_raw[..4], &now.raw.to_le_bytes());
        assert_eq!(&edit.new_raw[4..flags - date], &raw[date + 4..flags]);
        assert_eq!(edit.new_raw[flags - date], 0xA3);
        assert_eq!(edit.new_raw[code - date], 5);
        raw[date..code + 1].copy_from_slice(&edit.new_raw);
        assert!(transfer_listing_edit(schemas, &raw, 0x50000, true, Some(now)).is_none());
        let off = transfer_listing_edit(schemas, &raw, 0x50000, false, Some(now)).unwrap();
        assert_eq!(&off.new_raw[..4], &0x076C0001u32.to_le_bytes());
        assert_eq!(off.new_raw[flags - date], 0xA2);
        assert_eq!(off.new_raw[code - date], 0);
        assert!(transfer_listing_edit(schemas, &raw, 0x50000, false, None).is_none());
        assert!(transfer_listing_edit(schemas, &raw[..code], 0x50000, false, Some(now)).is_none());
        assert!(transfer_listing_edit(schemas, &raw, u64::MAX, false, Some(now)).is_none());
    }

    fn collect_text(shape: &egui::Shape, text: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => text.push(t.galley.text().to_owned()),
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    collect_text(s, text);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn fm24_fm26_new_player_sections_render_nothing() {
        use crate::game_version::GameBuild;
        for build in [GameBuild::Fm24Steam, GameBuild::Fm26Steam] {
            let ctx = egui::Context::default();
            let mut player = fixture_player(build);
            let mut edits = vec![];
            let mut raw = vec![0; 0xC4];
            let output = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show_player_status(build.schemas(), ui, &mut player, 0x20000, &mut edits);
                    show_nationality_ties(build.schemas(), ui, &player);
                    show_clauses(build.schemas(), ui, &player, &mut edits);
                    show_listing(build.schemas(), ui, &mut raw, 0x30000, None, &mut edits);
                });
            });
            let mut text = vec![];
            for s in output.shapes {
                collect_text(&s.shape, &mut text);
            }
            assert!(text.is_empty(), "{build:?}: {text:?}");
            assert!(edits.is_empty());
        }
    }

    #[test]
    fn fm27_player_sections_distinguish_unloaded_unreadable_and_empty_without_writes() {
        use crate::game_version::GameBuild;
        for width in [1200.0, 280.0] {
            for enabled in [false, true] {
                for state in [0, 1, 2] {
                    let ctx = egui::Context::default();
                    crate::ui::navigation::set_editing_enabled(&ctx, enabled);
                    let mut player = fixture_player(GameBuild::Fm27Steam);
                    player.raw[player.build.schemas().offset_of("p_morale").unwrap()] = 15;
                    player.injuries = match state {
                        0 => ListState::Unmapped,
                        1 => ListState::Unreadable,
                        _ => ListState::Items(vec![]),
                    };
                    player.absences = match state {
                        0 => ListState::Unmapped,
                        1 => ListState::Unreadable,
                        _ => ListState::Items(vec![]),
                    };
                    let original = player.raw.clone();
                    let mut edits = vec![];
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 700.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                show_player_status(
                                    player.build.schemas(),
                                    ui,
                                    &mut player,
                                    0x20000,
                                    &mut edits,
                                );
                            });
                        },
                    );
                    let mut text = vec![];
                    for s in output.shapes {
                        collect_text(&s.shape, &mut text);
                    }
                    let text = text.join(" ");
                    assert!(text.contains("Level 15"), "{text}");
                    assert!(
                        text.contains(match state {
                            0 => "Not loaded",
                            1 => "unavailable",
                            _ => "None",
                        }),
                        "{text}"
                    );
                    assert!(edits.is_empty());
                    assert_eq!(player.raw, original);
                }
            }
        }
    }

    fn direct(offset: u64) -> Option<fields::FieldLocation> {
        Some(fields::FieldLocation::Direct {
            offset,
            encoding: fields::Encoding::Raw,
        })
    }
    #[test]
    fn formation_stays_compact_and_rendering_preserves_values() {
        fn text_centers(
            shape: &egui::Shape,
            centers: &mut std::collections::HashMap<String, egui::Pos2>,
        ) {
            match shape {
                egui::Shape::Text(text) => {
                    centers.insert(
                        text.galley.text().to_owned(),
                        text.pos + text.galley.size() * 0.5,
                    );
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        text_centers(shape, centers);
                    }
                }
                _ => {}
            }
        }

        let build = crate::game_version::GameBuild::Fm26Steam;
        let schemas = build.schemas();
        for invalid_right_cell in [false, true] {
            for width in [1200.0, 280.0] {
                let ctx = egui::Context::default();
                crate::ui::navigation::set_editing_enabled(&ctx, true);
                let mut player = PlayerRecord {
                    build,
                    addr: 0x20000,
                    index: 0,
                    name: None,
                    raw: vec![0; 1024],
                    person_shift: 0,
                    contract_raw: None,
                    contract_addr: None,
                    club_name: None,
                    club_addr: None,
                    nationality: None,
                    birthplace: None,
                    game_date: None,
                    injuries: ListState::Unmapped,
                    absences: ListState::Unmapped,
                    relations: ListState::Unmapped,
                    clauses: ListState::Unmapped,
                };
                for field in schemas.fields_for_category(Entity::Player, Category::Positions) {
                    let loc = field.offset.as_ref().unwrap();
                    player.raw[loc.offset() as usize] = loc.encode_u8(10);
                }
                if invalid_right_cell {
                    let loc = schemas.field("pos_amr").unwrap().offset.unwrap();
                    player.raw[loc.offset() as usize] = 255;
                }
                let original = player.raw.clone();
                let mut centers = std::collections::HashMap::new();
                // Grid column sizes are retained across frames; check both the
                // first layout and subsequent layouts with cached geometry.
                for _ in 0..3 {
                    let mut edits = Vec::new();
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 700.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                show_positions_formation(
                                    schemas,
                                    ui,
                                    &mut player,
                                    0x20000,
                                    &mut edits,
                                );
                            });
                        },
                    );
                    assert!(edits.is_empty());
                    assert_eq!(player.raw, original);
                    centers.clear();
                    for shape in &output.shapes {
                        text_centers(&shape.shape, &mut centers);
                    }
                    let aml = centers["AML"];
                    let amc = centers["AMC"];
                    let amr = centers["AMR"];
                    // Text origins are pixel-rounded independently, so their
                    // painted centers can differ by one pixel.
                    let close = |a: f32, b: f32| assert!((a - b).abs() <= 1.0, "{a} != {b}");
                    close(amc.x - aml.x, 70.0);
                    close(amr.x - amc.x, 70.0);
                    close(aml.y, amc.y);
                    close(amc.y, amr.y);
                    assert!(aml.x < 50.0, "formation must stay near the left edge");
                    assert!(amr.x < 200.0, "formation must not stretch to page width");
                    let dc = centers["DC"];
                    let sw = centers["SW"];
                    let gk = centers["GK"];
                    close(sw.x, dc.x);
                    close(sw.x, gk.x);
                    assert!(dc.y < sw.y && sw.y < gk.y);
                    assert!(!centers.contains_key("Sweeper:"));
                    if invalid_right_cell {
                        close(centers["r255"].x, amr.x);
                    }
                }
            }
        }
    }

    #[test]
    fn traits_require_mapping_and_complete_buffer() {
        assert_eq!(trait_value(&[0; 16], None), None);
        assert_eq!(trait_value(&[0; 7], direct(0)), None);
        assert_eq!(trait_value(&[0; 16], direct(9)), None);
        assert_eq!(trait_value(&[0; 16], direct(u64::MAX)), None);
        assert_eq!(
            trait_value(
                &[0; 16],
                Some(fields::FieldLocation::SubRecord {
                    container: "contract",
                    offset: 0,
                    encoding: fields::Encoding::Raw,
                })
            ),
            None
        );
        assert_eq!(
            trait_value(&[1, 0, 0, 0, 0, 0, 0, 0], direct(0)),
            Some((0, 1))
        );
        assert_eq!(trait_value(&[0; 16], direct(8)), Some((8, 0)));
    }

    #[test]
    fn fmt_date_masks_flag_bits() {
        // Day word 7019 = flags | day 363 → 29 Dec 2025.
        let buf = [7019u16.to_le_bytes(), 2025u16.to_le_bytes()].concat();
        assert_eq!(fmt_date_at(&buf, 0).as_deref(), Some("29 Dec 2025"));
    }
}
