//! FMLE-style colored value widgets and editable attribute grids.

use eframe::egui;

use crate::fields::{self, Category, Encoding, Entity, Field};
use crate::settings::{Theme, theme_data_id};

use super::navigation;

const VALUE_SIZE: [f32; 2] = [44.0, 22.0];

/// Keep disabled controls visible, but never commit their temporary values.
fn number_edit(ui: &mut egui::Ui, value: egui::DragValue<'_>, width: f32) -> bool {
    let enabled = navigation::editing_enabled(ui) && ui.is_enabled();
    let response = ui.add_enabled_ui(enabled, |ui| {
        ui.add_sized([width.max(VALUE_SIZE[0]), VALUE_SIZE[1]], value)
    });
    enabled && response.inner.changed()
}

/// Describes one field edit (for undo + bridge write).
pub struct FieldEdit {
    pub addr: u64,
    pub old_raw: Vec<u8>,
    pub new_raw: Vec<u8>,
    pub description: String,
}

/// Foreground text colour for a 1-20 attribute value, tuned for readability
/// on a dark background. Use `ui.visuals_mut().override_text_color = Some(..)`
/// before drawing the widget, or `ui.colored_label(color, ..)`.
pub fn value_color(val: u8) -> egui::Color32 {
    match val {
        16..=20 => egui::Color32::from_rgb(76, 175, 80), // bright green
        12..=15 => egui::Color32::from_rgb(139, 195, 74), // light green
        8..=11 => egui::Color32::from_rgb(205, 220, 57), // lime
        4..=7 => egui::Color32::from_rgb(255, 152, 0),   // orange
        _ => egui::Color32::from_rgb(244, 67, 54),       // red
    }
}

/// Foreground colour for a 1-20 attribute value in an editable widget.
/// In the Dark theme, values 6-10 render white for improved readability.
pub fn attr_color(val: u8, ui: &egui::Ui) -> egui::Color32 {
    if matches!(val, 6..=10) {
        let theme = ui.ctx().data(|d| d.get_temp::<Theme>(theme_data_id()));
        if matches!(theme, Some(Theme::Dark)) {
            return egui::Color32::WHITE;
        }
    }
    value_color(val)
}

/// Colour for a numeric rating in the 0..=200 range (CA/PA). Uses the same
/// five-band scheme but scaled — handy for the search results grid.
pub fn ca_pa_color(val: u8) -> egui::Color32 {
    // Map 0..=200 to 1..=20 band so the same thresholds apply visually.
    let bucket = ((val as u16 + 9) / 10).clamp(1, 20) as u8;
    value_color(bucket)
}

/// Disabled checkbox placeholder for features that aren't wired up yet.
/// The checkbox renders in its correct FMSE position and label so the
/// layout parity is preserved, but it's not clickable and hovering it
/// shows `reason` as a tooltip (e.g. "Not yet implemented", "Not yet
/// mapped"). Use this instead of a live `ui.checkbox` whenever the
/// backing filter/flag isn't functional, so the UI never silently lies
/// about what it does.
pub fn stub_checkbox(ui: &mut egui::Ui, label: &str, reason: &str) -> egui::Response {
    let mut dummy = false;
    ui.add_enabled(false, egui::Checkbox::new(&mut dummy, label))
        .on_disabled_hover_text(reason)
}

/// Greyed-out, non-interactive field row for fields whose offset isn't mapped yet.
/// Renders in the same label + value style as `attr_edit` so the layout holds
/// its shape, but shows "—" as the value and a tooltip explaining why.
pub fn stub_field(ui: &mut egui::Ui, label: &str, reason: &str) {
    let grey = egui::Color32::from_rgb(110, 110, 110);
    let resp = ui.horizontal(|ui| {
        ui.add_sized(
            [130.0, VALUE_SIZE[1]],
            egui::Label::new(egui::RichText::new(label).color(grey)),
        );
        ui.add_sized(
            VALUE_SIZE,
            egui::Label::new(egui::RichText::new("—").color(grey)),
        )
    });
    resp.inner.on_disabled_hover_text(reason);
}

/// Grey, read-only pill rendered in place of an editable widget when the
/// stored byte is outside the widget's valid range. Surfaces the anomaly
/// (so you can see unused/garbage slots during reverse-engineering) without
/// letting egui's DragValue silently clamp and canonicalize game memory.
fn render_out_of_range_label(ui: &mut egui::Ui, label: &str, raw_text: String, width: f32) {
    let grey = egui::Color32::from_rgb(110, 110, 110);
    ui.horizontal(|ui| {
        ui.add_sized(
            [130.0, VALUE_SIZE[1]],
            egui::Label::new(egui::RichText::new(label).color(grey)),
        );
        ui.add_sized(
            [width.max(VALUE_SIZE[0]), VALUE_SIZE[1]],
            egui::Label::new(egui::RichText::new(raw_text).color(grey)),
        );
    });
}

/// Render one attribute: label + colored DragValue.
/// Updates `raw[offset]` in-place and returns a FieldEdit if changed.
///
/// If the decoded value is outside 1..=20 (e.g. unused slots with raw=0 or
/// uninitialized bytes) the widget is rendered read-only as `raw=N` to
/// surface the anomaly without letting DragValue silently clamp the byte.
pub fn attr_edit(
    ui: &mut egui::Ui,
    field: &Field,
    raw: &mut [u8],
    entity_addr: u64,
) -> Option<FieldEdit> {
    let loc = field.offset.as_ref()?;
    let offset = loc.offset() as usize;
    if offset >= raw.len() {
        return None;
    }

    let old_raw_byte = raw[offset];
    let displayed = loc.decode_u8(old_raw_byte);

    // Decoded > 20: genuinely bad data (wrong offset or corrupt byte). Show read-only.
    // Decoded 0: game stores raw 0/1/2 as a below-minimum sentinel (e.g. a striker's GK
    // attributes, or an unset hidden attribute). Floor to 1 — same as FMLE's behaviour.
    if displayed > 20 {
        render_out_of_range_label(ui, field.label, format!("raw={old_raw_byte}"), 44.0);
        return None;
    }

    let mut val = (displayed as i32).max(1);
    let color = attr_color(val as u8, ui);
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.add_sized([130.0, VALUE_SIZE[1]], egui::Label::new(field.label));
        ui.visuals_mut().override_text_color = Some(color);
        changed = number_edit(
            ui,
            egui::DragValue::new(&mut val)
                .update_while_editing(false)
                .range(1..=20)
                .speed(0.1),
            VALUE_SIZE[0],
        );
    });

    // Only emit an edit if the *displayed* value actually changed. This
    // avoids spurious writes from the lossy encode round-trip
    // (decode(encode(v)) != v for most raws).
    if changed {
        let new_displayed = val.clamp(1, 20) as u8;
        if new_displayed != displayed {
            let new_raw_byte = loc.encode_u8(new_displayed);
            raw[offset] = new_raw_byte;
            return Some(FieldEdit {
                addr: entity_addr + offset as u64,
                old_raw: vec![old_raw_byte],
                new_raw: vec![new_raw_byte],
                description: format!("{}: {} \u{2192} {}", field.label, displayed, new_displayed),
            });
        }
    }
    None
}

/// Render a compact attribute cell with the label ABOVE a colored value box.
/// Used by the positions formation grid.
pub fn attr_cell_compact(
    ui: &mut egui::Ui,
    field: &Field,
    raw: &mut [u8],
    entity_addr: u64,
) -> Option<FieldEdit> {
    let loc = field.offset.as_ref()?;
    let offset = loc.offset() as usize;
    if offset >= raw.len() {
        return None;
    }

    let old_raw_byte = raw[offset];
    let displayed = loc.decode_u8(old_raw_byte);

    if !(1..=20).contains(&displayed) {
        let grey = egui::Color32::from_rgb(110, 110, 110);
        ui.vertical(|ui| {
            ui.set_min_width(44.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(field.label).color(grey));
                ui.add_sized(
                    VALUE_SIZE,
                    egui::Label::new(egui::RichText::new(format!("r{old_raw_byte}")).color(grey)),
                );
            });
        });
        return None;
    }

    let mut val = displayed as i32;
    let color = attr_color(displayed, ui);
    let mut changed = false;

    ui.vertical(|ui| {
        ui.set_min_width(44.0);
        ui.vertical_centered(|ui| {
            ui.label(field.label);
            ui.visuals_mut().override_text_color = Some(color);
            changed = number_edit(
                ui,
                egui::DragValue::new(&mut val)
                    .update_while_editing(false)
                    .range(1..=20)
                    .speed(0.1),
                VALUE_SIZE[0],
            );
        });
    });

    if changed {
        let new_displayed = val.clamp(1, 20) as u8;
        if new_displayed != displayed {
            let new_raw_byte = loc.encode_u8(new_displayed);
            raw[offset] = new_raw_byte;
            return Some(FieldEdit {
                addr: entity_addr + offset as u64,
                old_raw: vec![old_raw_byte],
                new_raw: vec![new_raw_byte],
                description: format!("{}: {} \u{2192} {}", field.label, displayed, new_displayed),
            });
        }
    }
    None
}

/// Render all fields for a category as a vertical list, dispatching to the
/// correct widget per encoding type. Fields with `offset: None` are silently
/// skipped — they appear automatically when the offset is added to the schema.
pub fn category_column(
    ui: &mut egui::Ui,
    heading: &str,
    entity: Entity,
    category: Category,
    raw: &mut [u8],
    entity_addr: u64,
    edits: &mut Vec<FieldEdit>,
) {
    ui.strong(heading);
    ui.add_space(6.0);
    let fields = fields::fields_for_category(entity, category);
    for f in &fields {
        if let Some(edit) = render_field(ui, f, raw, entity_addr) {
            edits.push(edit);
        }
    }
    ui.add_space(8.0);
}

/// Render a staff-roles sub-record as a 2-column grid (label | rating).
///
/// Iterates all `Category::StaffRole` fields in their declaration order
/// (= FMSE display order). If `skip_zeros` is `true`, roles rated 0 are
/// omitted — use this in the tabbed Overview panel to show only assigned roles.
/// Pass `false` in the manager panel to always show the full list.
pub fn roles_column(ui: &mut egui::Ui, heading: &str, roles: &[u8; 15], skip_zeros: bool) {
    ui.strong(heading);
    ui.add_space(6.0);
    let role_fields = fields::fields_for_category(Entity::Staff, Category::StaffRole);
    egui::Grid::new(heading)
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for f in &role_fields {
                if let Some(loc) = &f.offset {
                    let idx = loc.offset() as usize;
                    if idx < roles.len() {
                        let val = roles[idx];
                        if skip_zeros && val == 0 {
                            continue;
                        }
                        ui.label(f.label);
                        ui.colored_label(value_color(val), format!("{val:>2}"));
                        ui.end_row();
                    }
                }
            }
        });
    ui.add_space(8.0);
}

/// Dispatch to the right widget for a field based on its encoding.
/// Returns `None` for unmapped fields (`offset: None`), pointer/string fields,
/// and out-of-range values that render as read-only labels.
pub fn render_field(
    ui: &mut egui::Ui,
    field: &Field,
    raw: &mut [u8],
    entity_addr: u64,
) -> Option<FieldEdit> {
    let encoding = field.offset.as_ref()?.encoding();
    match encoding {
        Encoding::Direct | Encoding::Encoded => attr_edit(ui, field, raw, entity_addr),
        Encoding::Raw => raw_u16_edit(ui, field, raw, u16::MAX, entity_addr),
        Encoding::RawCapped(max) => raw_u16_edit(ui, field, raw, max, entity_addr),
        Encoding::RawU32 => raw_u32_edit(ui, field, raw, entity_addr),
        Encoding::DateDayYear => dob_edit(ui, raw, field, entity_addr).into_iter().next(),
        // Pointers and strings aren't user-editable via a panel row.
        // RoleIndex fields are rendered by roles_column, not through render_field.
        Encoding::Pointer | Encoding::FmString | Encoding::F32Raw | Encoding::RoleIndex => None,
    }
}

/// Render a u8 raw field (CA, PA, etc.) with a DragValue in the given 0..=max range.
/// Returns a FieldEdit if changed. Label, offset, and encoding come from `field`.
pub fn raw_u8_edit(
    ui: &mut egui::Ui,
    field: &Field,
    raw: &mut [u8],
    max: u8,
    entity_addr: u64,
) -> Option<FieldEdit> {
    let offset = field.offset.as_ref()?.offset() as usize;
    let label = field.label;
    if offset >= raw.len() {
        return None;
    }
    let old = raw[offset];

    if old > max {
        render_out_of_range_label(ui, label, format!("raw={old}"), 50.0);
        return None;
    }

    let mut val = old as i32;
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(label);
        changed = number_edit(
            ui,
            egui::DragValue::new(&mut val)
                .update_while_editing(false)
                .range(0..=max as i32)
                .speed(0.5),
            VALUE_SIZE[0],
        );
    });

    if changed {
        let new_val = val.clamp(0, max as i32) as u8;
        if new_val != old {
            raw[offset] = new_val;
            return Some(FieldEdit {
                addr: entity_addr + offset as u64,
                old_raw: vec![old],
                new_raw: vec![new_val],
                description: format!("{}: {} \u{2192} {}", label, old, new_val),
            });
        }
    }
    None
}

/// Render a u16 raw field (reputation, etc.). Label, offset, and encoding
/// come from `field`.
pub fn raw_u16_edit(
    ui: &mut egui::Ui,
    field: &Field,
    raw: &mut [u8],
    max: u16,
    entity_addr: u64,
) -> Option<FieldEdit> {
    let offset = field.offset.as_ref()?.offset() as usize;
    let label = field.label;
    if offset + 1 >= raw.len() {
        return None;
    }
    let old_bytes = [raw[offset], raw[offset + 1]];
    let old_val = u16::from_le_bytes(old_bytes);

    if old_val > max {
        render_out_of_range_label(ui, label, format!("raw={old_val}"), 60.0);
        return None;
    }

    let mut val = old_val as i32;
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(label);
        changed = number_edit(
            ui,
            egui::DragValue::new(&mut val)
                .update_while_editing(false)
                .range(0..=max as i32)
                .speed(1.0),
            50.0,
        );
    });

    if changed {
        let new_val = val.clamp(0, max as i32) as u16;
        if new_val != old_val {
            let new_bytes = new_val.to_le_bytes();
            raw[offset] = new_bytes[0];
            raw[offset + 1] = new_bytes[1];
            return Some(FieldEdit {
                addr: entity_addr + offset as u64,
                old_raw: old_bytes.to_vec(),
                new_raw: new_bytes.to_vec(),
                description: format!("{}: {} \u{2192} {}", label, old_val, new_val),
            });
        }
    }
    None
}

/// Render a u32 raw field (wages, budgets, etc.). No max clamping beyond u32.
/// Label and offset come from `field` (encoding must be `RawU32`).
pub fn raw_u32_edit(
    ui: &mut egui::Ui,
    field: &Field,
    raw: &mut [u8],
    entity_addr: u64,
) -> Option<FieldEdit> {
    let offset = field.offset.as_ref()?.offset() as usize;
    let label = field.label;
    if offset + 3 >= raw.len() {
        return None;
    }
    let old_bytes = [
        raw[offset],
        raw[offset + 1],
        raw[offset + 2],
        raw[offset + 3],
    ];
    let old_val = u32::from_le_bytes(old_bytes);
    let mut val = old_val as i64;
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(label);
        changed = number_edit(
            ui,
            egui::DragValue::new(&mut val)
                .update_while_editing(false)
                .range(0..=u32::MAX as i64)
                .speed(100.0),
            70.0,
        );
    });

    if changed {
        let new_val = val.clamp(0, u32::MAX as i64) as u32;
        if new_val != old_val {
            let new_bytes = new_val.to_le_bytes();
            raw[offset..offset + 4].copy_from_slice(&new_bytes);
            return Some(FieldEdit {
                addr: entity_addr + offset as u64,
                old_raw: old_bytes.to_vec(),
                new_raw: new_bytes.to_vec(),
                description: format!("{}: {} \u{2192} {}", label, old_val, new_val),
            });
        }
    }
    None
}

/// Render CA and PA side by side on one line. Each field provides its own
/// offset (both raw u8 in 0..=200).
pub fn ca_pa_edit(
    ui: &mut egui::Ui,
    raw: &mut [u8],
    ca_field: &Field,
    pa_field: &Field,
    entity_addr: u64,
) -> Vec<FieldEdit> {
    let mut edits = Vec::new();
    let ca_offset = match ca_field.offset.as_ref() {
        Some(l) => l.offset() as usize,
        None => return edits,
    };
    let pa_offset = match pa_field.offset.as_ref() {
        Some(l) => l.offset() as usize,
        None => return edits,
    };
    if ca_offset >= raw.len() || pa_offset >= raw.len() {
        return edits;
    }
    let old_ca = raw[ca_offset];
    let old_pa = raw[pa_offset];
    let grey = egui::Color32::from_rgb(110, 110, 110);

    ui.horizontal(|ui| {
        // CA
        ui.label("CA");
        if old_ca > 200 {
            ui.add_sized(
                VALUE_SIZE,
                egui::Label::new(egui::RichText::new(format!("r{old_ca}")).color(grey)),
            );
        } else {
            let mut ca = old_ca as i32;
            let changed = number_edit(
                ui,
                egui::DragValue::new(&mut ca)
                    .update_while_editing(false)
                    .range(0..=200)
                    .speed(0.5),
                VALUE_SIZE[0],
            );
            if changed {
                let new_val = ca.clamp(0, 200) as u8;
                if new_val != old_ca {
                    raw[ca_offset] = new_val;
                    edits.push(FieldEdit {
                        addr: entity_addr + ca_offset as u64,
                        old_raw: vec![old_ca],
                        new_raw: vec![new_val],
                        description: format!("CA: {} \u{2192} {}", old_ca, new_val),
                    });
                }
            }
        }

        // PA
        ui.label("PA");
        if old_pa > 200 {
            ui.add_sized(
                VALUE_SIZE,
                egui::Label::new(egui::RichText::new(format!("r{old_pa}")).color(grey)),
            );
        } else {
            let mut pa = old_pa as i32;
            let changed = number_edit(
                ui,
                egui::DragValue::new(&mut pa)
                    .update_while_editing(false)
                    .range(0..=200)
                    .speed(0.5),
                VALUE_SIZE[0],
            );
            if changed {
                let new_val = pa.clamp(0, 200) as u8;
                if new_val != old_pa {
                    raw[pa_offset] = new_val;
                    edits.push(FieldEdit {
                        addr: entity_addr + pa_offset as u64,
                        old_raw: vec![old_pa],
                        new_raw: vec![new_val],
                        description: format!("PA: {} \u{2192} {}", old_pa, new_val),
                    });
                }
            }
        }
    });

    edits
}

/// Render an editable DOB as three fields: Day / Month / Year.
/// `dob_field` must have `Encoding::DateDayYear` — the widget reads the u16
/// day-of-year at `field.offset` and the u16 year at `field.offset + 2`.
pub fn dob_edit(
    ui: &mut egui::Ui,
    raw: &mut [u8],
    dob_field: &Field,
    entity_addr: u64,
) -> Vec<FieldEdit> {
    let mut edits = Vec::new();
    let loc = match dob_field.offset.as_ref() {
        Some(l) => l,
        None => return edits,
    };
    let Ok(doy_offset) = usize::try_from(loc.offset()) else {
        return edits;
    };
    let Some(end) = doy_offset.checked_add(4) else {
        return edits;
    };
    let year_offset = doy_offset + 2;
    if end > raw.len() {
        return edits;
    }
    let mut edits_block = dob_edit_inner(ui, raw, doy_offset, year_offset, entity_addr);
    edits.append(&mut edits_block);
    edits
}

fn dob_edit_inner(
    ui: &mut egui::Ui,
    raw: &mut [u8],
    doy_offset: usize,
    year_offset: usize,
    entity_addr: u64,
) -> Vec<FieldEdit> {
    let mut edits = Vec::new();

    let old_doy_bytes = [raw[doy_offset], raw[doy_offset + 1]];
    let old_year_bytes = [raw[year_offset], raw[year_offset + 1]];
    let doy = u16::from_le_bytes(old_doy_bytes);
    let year = u16::from_le_bytes(old_year_bytes);

    // Out-of-range DOB: render read-only so the DragValue doesn't clamp
    // year to 1900 (overwriting genuine zeros/blanks in memory).
    if !(1900..=2030).contains(&year) || doy == 0 || doy > 366 {
        let grey = egui::Color32::from_rgb(110, 110, 110);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("DOB").color(grey));
            ui.label(egui::RichText::new(format!("doy={doy} year={year}")).color(grey));
        });
        return edits;
    }

    // Convert day-of-year to (day, month)
    let (mut day, mut month) = doy_to_day_month(year, doy);
    let mut yr = year as i32;

    let orig_day = day;
    let orig_month = month;
    let orig_year = yr;

    ui.horizontal(|ui| {
        ui.label("DOB");
        let day_changed = number_edit(
            ui,
            egui::DragValue::new(&mut day)
                .update_while_editing(false)
                .range(1..=31)
                .speed(0.2),
            VALUE_SIZE[0],
        );
        ui.label("/");
        let month_changed = number_edit(
            ui,
            egui::DragValue::new(&mut month)
                .update_while_editing(false)
                .range(1..=12)
                .speed(0.2),
            VALUE_SIZE[0],
        );
        ui.label("/");
        let year_changed = number_edit(
            ui,
            egui::DragValue::new(&mut yr)
                .update_while_editing(false)
                .range(1900..=2030)
                .speed(0.5),
            50.0,
        );

        if day_changed || month_changed || year_changed {
            let new_year = yr.clamp(1900, 2030) as u16;
            let new_doy = day_month_to_doy(new_year, day.clamp(1, 31), month.clamp(1, 12));

            let new_doy_bytes = new_doy.to_le_bytes();
            let new_year_bytes = new_year.to_le_bytes();

            if new_doy_bytes != old_doy_bytes {
                raw[doy_offset] = new_doy_bytes[0];
                raw[doy_offset + 1] = new_doy_bytes[1];
                edits.push(FieldEdit {
                    addr: entity_addr + doy_offset as u64,
                    old_raw: old_doy_bytes.to_vec(),
                    new_raw: new_doy_bytes.to_vec(),
                    description: format!(
                        "DOB day: {}/{} \u{2192} {}/{}",
                        orig_day, orig_month, day, month
                    ),
                });
            }
            if new_year_bytes != old_year_bytes {
                raw[year_offset] = new_year_bytes[0];
                raw[year_offset + 1] = new_year_bytes[1];
                edits.push(FieldEdit {
                    addr: entity_addr + year_offset as u64,
                    old_raw: old_year_bytes.to_vec(),
                    new_raw: new_year_bytes.to_vec(),
                    description: format!("DOB year: {} \u{2192} {}", orig_year, yr),
                });
            }
        }
    });

    edits
}

fn doy_to_day_month(year: u16, doy: u16) -> (i32, i32) {
    if doy == 0 {
        return (1, 1);
    }
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let month_days: [u16; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut remaining = doy;
    for (i, &days) in month_days.iter().enumerate() {
        if remaining <= days {
            return (remaining as i32, (i + 1) as i32);
        }
        remaining -= days;
    }
    (1, 1)
}

fn day_month_to_doy(year: u16, day: i32, month: i32) -> u16 {
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let month_days: [u16; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let m = (month - 1).clamp(0, 11) as usize;
    let mut doy: u16 = 0;
    for i in 0..m {
        doy += month_days[i];
    }
    doy + (day.clamp(1, month_days[m] as i32) as u16)
}

/// Read-only label for a numeric value.
pub fn info_label(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Display) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.label(format!("{value}"));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::FieldLocation;

    fn field(encoding: Encoding, offset: u64) -> Field {
        Field {
            key: "test",
            label: "Test value",
            entity: Entity::Player,
            category: Category::General,
            offset: Some(FieldLocation::Direct { offset, encoding }),
        }
    }

    #[test]
    fn dob_requires_mapping_and_complete_buffer() {
        let ctx = egui::Context::default();
        navigation::set_editing_enabled(&ctx, true);
        let mut raw = [1, 0, 208];
        let original = raw;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut unmapped = field(Encoding::DateDayYear, 0);
                unmapped.offset = None;
                assert!(dob_edit(ui, &mut raw, &unmapped, 0).is_empty());
                assert!(dob_edit(ui, &mut raw, &field(Encoding::DateDayYear, 0), 0).is_empty());
                assert!(
                    dob_edit(ui, &mut raw, &field(Encoding::DateDayYear, u64::MAX), 0).is_empty()
                );
            });
        });
        assert_eq!(raw, original);
    }
    #[test]
    fn disabled_editors_preserve_buffers_and_emit_no_edits() {
        // Default previews, explicitly disabled editing, and a disabled ancestor.
        for availability in [None, Some(false), Some(true)] {
            let ctx = egui::Context::default();
            if let Some(enabled) = availability {
                navigation::set_editing_enabled(&ctx, enabled);
            }
            let mut raw = [0, 10, 20, 0];
            let original = raw;
            let mut dob = [1, 0, 208, 7]; // 1 January 2000
            let original_dob = dob;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add_enabled_ui(availability != Some(true), |ui| {
                        // Zero is a below-minimum sentinel for attr_edit: displaying
                        // it as 1 must not canonicalize it into a write.
                        let attribute = field(Encoding::Direct, 0);
                        assert!(attr_edit(ui, &attribute, &mut raw, 0).is_none());
                        assert!(
                            attr_cell_compact(ui, &field(Encoding::Direct, 1), &mut raw, 0)
                                .is_none()
                        );
                        assert!(raw_u8_edit(ui, &attribute, &mut raw, 200, 0).is_none());
                        assert!(
                            raw_u16_edit(ui, &field(Encoding::Raw, 0), &mut raw, u16::MAX, 0)
                                .is_none()
                        );
                        assert!(
                            raw_u32_edit(ui, &field(Encoding::RawU32, 0), &mut raw, 0).is_none()
                        );
                        assert!(
                            ca_pa_edit(ui, &mut raw, &attribute, &field(Encoding::Direct, 1), 0)
                                .is_empty()
                        );
                        assert!(
                            dob_edit(ui, &mut dob, &field(Encoding::DateDayYear, 0), 0).is_empty()
                        );
                    });
                });
            });
            assert_eq!(raw, original);
            assert_eq!(dob, original_dob);
        }
    }
}
