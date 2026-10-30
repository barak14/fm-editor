//! Aligned profile field rows with colored numeric editors and attribute grids.

use eframe::egui;

use crate::fields::{Category, Encoding, Entity, Field};
use crate::settings::{Theme, theme_data_id};

use super::navigation;

const VALUE_SIZE: [f32; 2] = [44.0, 22.0];

/// A fixed label slot keeps profile values aligned without centering short labels.
/// Truncated labels automatically show their full text on hover in egui.
fn profile_label(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) -> egui::Response {
    ui.allocate_ui_with_layout(
        egui::vec2(130.0, VALUE_SIZE[1]),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_size(egui::vec2(130.0, VALUE_SIZE[1]));
            ui.add(egui::Label::new(label).truncate())
        },
    )
    .inner
}

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
        profile_label(ui, egui::RichText::new(label).color(grey));
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
        profile_label(ui, egui::RichText::new(label).color(grey));
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
        profile_label(ui, field.label);
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
            ui.set_width(56.0);
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
        ui.set_width(56.0);
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
    schemas: &'static crate::fields::VersionedSchemas,
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
    let fields = schemas.fields_for_category(entity, category);
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
pub fn roles_column(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    heading: &str,
    roles: &[u8; 15],
    skip_zeros: bool,
) {
    ui.strong(heading);
    ui.add_space(6.0);
    let role_fields = schemas.fields_for_category(Entity::Staff, Category::StaffRole);
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
                        profile_label(ui, f.label);
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
        profile_label(ui, label);
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
        profile_label(ui, label);
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
        profile_label(ui, label);
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
    let mut edits_block = date_edit_inner(
        ui,
        "DOB",
        raw,
        doy_offset,
        year_offset,
        entity_addr,
        1900..=2030,
    );
    edits.append(&mut edits_block);
    edits
}

/// Editable `DateDayYear` field labelled `label`, with years limited to
/// `years`. Works on any buffer: pass a sub-record's bytes and address for a
/// sub-record field (e.g. a contract's expiry date).
pub fn date_edit(
    ui: &mut egui::Ui,
    label: &str,
    raw: &mut [u8],
    field: &Field,
    base_addr: u64,
    years: std::ops::RangeInclusive<u16>,
) -> Vec<FieldEdit> {
    let Some(loc) = field.offset.as_ref() else {
        return Vec::new();
    };
    let Ok(doy_offset) = usize::try_from(loc.offset()) else {
        return Vec::new();
    };
    if doy_offset.checked_add(4).is_none_or(|end| end > raw.len()) {
        return Vec::new();
    }
    date_edit_inner(ui, label, raw, doy_offset, doy_offset + 2, base_addr, years)
}

/// Bits of a date's u16 day-of-year word that hold the day; FM keeps flags in
/// the bits above (a FM27 contract-signed date stores 7019 = flags | day 363).
const DOY_MASK: u16 = 0x1FF;

/// `word` with its day-of-year bits replaced by `doy`, keeping FM's flags.
fn replace_doy(word: u16, doy: u16) -> u16 {
    (word & !DOY_MASK) | (doy & DOY_MASK)
}

/// Popup and draft identity follows the record, not its position in a panel.
fn calendar_id(entity_addr: u64, doy_offset: usize) -> egui::Id {
    egui::Id::new(("date_calendar", entity_addr, doy_offset))
}

fn days_in_month(year: u16, month: i32) -> u16 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    }
}

/// Monday-based weekday, using the proleptic Gregorian calendar.
fn first_weekday(year: u16, month: i32) -> usize {
    let y = i64::from(year) - i64::from(month < 3);
    let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let sunday = (y + y.div_euclid(4) - y.div_euclid(100)
        + y.div_euclid(400)
        + offsets[(month - 1) as usize]
        + 1)
    .rem_euclid(7);
    ((sunday + 6) % 7) as usize
}

#[derive(Clone)]
struct CalendarDraft {
    year: u16,
    month: i32,
    year_text: String,
}

impl CalendarDraft {
    fn shift_month(&mut self, delta: i32, years: &std::ops::RangeInclusive<u16>) {
        let index = i32::from(self.year) * 12 + self.month - 1 + delta;
        let year = index.div_euclid(12);
        if year >= i32::from(*years.start()) && year <= i32::from(*years.end()) {
            self.year = year as u16;
            self.month = index.rem_euclid(12) + 1;
            self.year_text = self.year.to_string();
        }
    }
}

/// Browsing is local UI state. Only a day button returns a proposed date.
fn calendar_button(
    ui: &mut egui::Ui,
    id: egui::Id,
    year: u16,
    month: i32,
    day: i32,
    years: &std::ops::RangeInclusive<u16>,
) -> Option<(u16, i32, i32)> {
    let enabled = navigation::editing_enabled(ui) && ui.is_enabled();
    let button = ui.add_enabled(enabled, egui::Button::new("Calendar"));
    if !enabled {
        if ui.memory(|memory| memory.is_popup_open(id)) {
            ui.memory_mut(|memory| memory.close_popup());
        }
        ui.data_mut(|data| data.remove::<CalendarDraft>(id));
        return None;
    }
    if button.clicked() {
        // Reopening always starts from the current record, never a cancelled draft.
        ui.data_mut(|data| {
            data.insert_temp(
                id,
                CalendarDraft {
                    year,
                    month,
                    year_text: year.to_string(),
                },
            )
        });
        ui.memory_mut(|memory| memory.toggle_popup(id));
    }
    let mut chosen = None;
    egui::popup_below_widget(
        ui,
        id,
        &button,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            let mut draft = ui
                .data_mut(|data| data.get_temp::<CalendarDraft>(id))
                .unwrap_or(CalendarDraft {
                    year,
                    month,
                    year_text: year.to_string(),
                });
            ui.set_min_width(250.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(draft.year > *years.start(), egui::Button::new("Year -"))
                    .clicked()
                {
                    draft.shift_month(-12, years);
                }
                ui.label("Year");
                let response =
                    ui.add(egui::TextEdit::singleline(&mut draft.year_text).desired_width(48.0));
                if response.changed() {
                    if let Ok(value) = draft.year_text.parse::<u16>() {
                        if years.contains(&value) {
                            draft.year = value;
                        }
                    }
                }
                if ui
                    .add_enabled(draft.year < *years.end(), egui::Button::new("Year +"))
                    .clicked()
                {
                    draft.shift_month(12, years);
                }
            });
            let valid_year = draft
                .year_text
                .parse::<u16>()
                .is_ok_and(|value| years.contains(&value));
            if !valid_year {
                ui.label(format!(
                    "Enter a year from {} to {}",
                    years.start(),
                    years.end()
                ));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        draft.year > *years.start() || draft.month > 1,
                        egui::Button::new("<"),
                    )
                    .clicked()
                {
                    draft.shift_month(-1, years);
                }
                const MONTHS: [&str; 12] = [
                    "January",
                    "February",
                    "March",
                    "April",
                    "May",
                    "June",
                    "July",
                    "August",
                    "September",
                    "October",
                    "November",
                    "December",
                ];
                ui.label(format!(
                    "{} {}",
                    MONTHS[(draft.month - 1) as usize],
                    draft.year
                ));
                if ui
                    .add_enabled(
                        draft.year < *years.end() || draft.month < 12,
                        egui::Button::new(">"),
                    )
                    .clicked()
                {
                    draft.shift_month(1, years);
                }
            });
            egui::Grid::new(id.with("days"))
                .num_columns(7)
                .show(ui, |ui| {
                    for weekday in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
                        ui.label(weekday);
                    }
                    ui.end_row();
                    let start = first_weekday(draft.year, draft.month);
                    let count = days_in_month(draft.year, draft.month) as usize;
                    for cell in 0..(start + count).div_ceil(7) * 7 {
                        if cell < start || cell >= start + count {
                            ui.label("");
                        } else {
                            let date_day = (cell - start + 1) as i32;
                            let selected =
                                draft.year == year && draft.month == month && date_day == day;
                            let text = if selected {
                                format!("[{date_day}]")
                            } else {
                                date_day.to_string()
                            };
                            if ui
                                .add_enabled(
                                    valid_year,
                                    egui::Button::new(text).min_size(egui::vec2(28.0, 22.0)),
                                )
                                .on_hover_text(if selected {
                                    "Current date"
                                } else {
                                    "Choose date"
                                })
                                .clicked()
                            {
                                chosen = Some((draft.year, date_day, draft.month));
                            }
                        }
                        if cell % 7 == 6 {
                            ui.end_row();
                        }
                    }
                });
            if ui.button("Cancel").clicked() || chosen.is_some() {
                ui.memory_mut(|memory| memory.close_popup());
            }
            ui.data_mut(|data| data.insert_temp(id, draft));
        },
    );
    chosen
}

fn date_edit_inner(
    ui: &mut egui::Ui,
    label: &str,
    raw: &mut [u8],
    doy_offset: usize,
    year_offset: usize,
    entity_addr: u64,
    years: std::ops::RangeInclusive<u16>,
) -> Vec<FieldEdit> {
    let mut edits = Vec::new();

    let old_doy_bytes = [raw[doy_offset], raw[doy_offset + 1]];
    let old_year_bytes = [raw[year_offset], raw[year_offset + 1]];
    let doy_word = u16::from_le_bytes(old_doy_bytes);
    let doy = doy_word & DOY_MASK;
    let year = u16::from_le_bytes(old_year_bytes);

    // Out-of-range date: render read-only so the DragValue doesn't clamp the
    // year into range (overwriting genuine zeros/blanks in memory).
    if !years.contains(&year) || doy == 0 || doy > day_month_to_doy(year, 31, 12) {
        let grey = egui::Color32::from_rgb(110, 110, 110);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(grey));
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
    let (min_year, max_year) = (*years.start() as i32, *years.end() as i32);

    let mut changed = false;
    let id = calendar_id(entity_addr, doy_offset);
    ui.push_id(id, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(label);
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
                    .range(min_year..=max_year)
                    .speed(0.5),
                50.0,
            );

            changed = day_changed || month_changed || year_changed;
            if let Some((new_year, new_day, new_month)) =
                calendar_button(ui, id, year, orig_month, orig_day, &years)
            {
                yr = i32::from(new_year);
                day = new_day;
                month = new_month;
                changed = true;
            }
        });
    });

    if changed {
        let new_year = yr.clamp(min_year, max_year) as u16;
        let new_doy = day_month_to_doy(new_year, day.clamp(1, 31), month.clamp(1, 12));

        let new_doy_bytes = replace_doy(doy_word, new_doy).to_le_bytes();
        let new_year_bytes = new_year.to_le_bytes();

        if new_doy_bytes != old_doy_bytes {
            raw[doy_offset] = new_doy_bytes[0];
            raw[doy_offset + 1] = new_doy_bytes[1];
            edits.push(FieldEdit {
                addr: entity_addr + doy_offset as u64,
                old_raw: old_doy_bytes.to_vec(),
                new_raw: new_doy_bytes.to_vec(),
                description: format!(
                    "{label} day: {}/{} \u{2192} {}/{}",
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
                description: format!("{label} year: {} \u{2192} {}", orig_year, yr),
            });
        }
    }

    edits
}

/// Edit a u8 code chosen from `options` (code, label). A stored code not in
/// `options` is shown read-only, so opening the panel never rewrites it.
pub fn choice_u8_edit(
    ui: &mut egui::Ui,
    label: &str,
    raw: &mut [u8],
    field: &Field,
    base_addr: u64,
    options: &[(u8, &str)],
) -> Option<FieldEdit> {
    let offset = usize::try_from(field.offset.as_ref()?.offset()).ok()?;
    let old = *raw.get(offset)?;
    let Some(&(_, old_name)) = options.iter().find(|(code, _)| *code == old) else {
        info_label(ui, label, format!("code {old}"));
        return None;
    };
    let mut new = old;
    let enabled = navigation::editing_enabled(ui) && ui.is_enabled();
    ui.horizontal(|ui| {
        profile_label(ui, label);
        ui.add_enabled_ui(enabled, |ui| {
            egui::ComboBox::from_id_salt(("choice_u8", label, base_addr, offset))
                .selected_text(old_name)
                .show_ui(ui, |ui| {
                    for &(code, name) in options {
                        ui.selectable_value(&mut new, code, name);
                    }
                });
        });
    });
    if !enabled || new == old {
        return None;
    }
    raw[offset] = new;
    let new_name = options
        .iter()
        .find(|(code, _)| *code == new)
        .map_or("?", |(_, n)| n);
    Some(FieldEdit {
        addr: base_addr + offset as u64,
        old_raw: vec![old],
        new_raw: vec![new],
        description: format!("{label}: {old_name} \u{2192} {new_name}"),
    })
}

/// Edit a u8 number where `none` means "no value" (e.g. squad number 255).
/// The editor shows 0 as "None"; values run 1..=`max`.
pub fn optional_u8_edit(
    ui: &mut egui::Ui,
    label: &str,
    raw: &mut [u8],
    field: &Field,
    base_addr: u64,
    max: u8,
    none: u8,
) -> Option<FieldEdit> {
    let offset = usize::try_from(field.offset.as_ref()?.offset()).ok()?;
    let old = *raw.get(offset)?;
    if old != none && (old == 0 || old > max) {
        info_label(ui, label, format!("raw={old}"));
        return None;
    }
    let mut val: i32 = if old == none { 0 } else { old as i32 };
    let mut changed = false;
    ui.horizontal(|ui| {
        profile_label(ui, label);
        changed = number_edit(
            ui,
            egui::DragValue::new(&mut val)
                .update_while_editing(false)
                .range(0..=max as i32)
                .speed(0.2)
                .custom_formatter(|n, _| {
                    if n == 0.0 {
                        "None".into()
                    } else {
                        format!("{n:.0}")
                    }
                }),
            VALUE_SIZE[0],
        );
    });
    if !changed {
        return None;
    }
    let new = if val <= 0 {
        none
    } else {
        val.min(max as i32) as u8
    };
    if new == old {
        return None;
    }
    raw[offset] = new;
    let show = |v: u8| {
        if v == none {
            "None".to_string()
        } else {
            v.to_string()
        }
    };
    Some(FieldEdit {
        addr: base_addr + offset as u64,
        old_raw: vec![old],
        new_raw: vec![new],
        description: format!("{label}: {} \u{2192} {}", show(old), show(new)),
    })
}

/// Edit an i32 amount (pounds) where -1 means "not set" (asking price).
/// The editor shows -1 as "Not set"; amounts run 0..=i32::MAX.
pub fn optional_money_edit(
    ui: &mut egui::Ui,
    label: &str,
    raw: &mut [u8],
    field: &Field,
    base_addr: u64,
) -> Option<FieldEdit> {
    let offset = usize::try_from(field.offset.as_ref()?.offset()).ok()?;
    let old_bytes: [u8; 4] = raw.get(offset..offset + 4)?.try_into().ok()?;
    let old = i32::from_le_bytes(old_bytes);
    if old < -1 {
        info_label(ui, label, format!("raw={old}"));
        return None;
    }
    let mut val = old as i64;
    let mut changed = false;
    ui.horizontal(|ui| {
        profile_label(ui, label);
        changed = number_edit(
            ui,
            egui::DragValue::new(&mut val)
                .update_while_editing(false)
                .range(-1..=i32::MAX as i64)
                .speed(10_000.0)
                .custom_formatter(|n, _| {
                    if n < 0.0 {
                        "Not set".into()
                    } else {
                        format!("£{n:.0}")
                    }
                })
                .custom_parser(|s| {
                    let t = s.trim().trim_start_matches('£').replace(',', "");
                    if t.is_empty() || t.eq_ignore_ascii_case("not set") {
                        Some(-1.0)
                    } else {
                        t.parse().ok()
                    }
                }),
            90.0,
        );
    });
    if !changed {
        return None;
    }
    let new = val.clamp(-1, i32::MAX as i64) as i32;
    if new == old {
        return None;
    }
    let new_bytes = new.to_le_bytes();
    raw[offset..offset + 4].copy_from_slice(&new_bytes);
    let show = |v: i32| {
        if v < 0 {
            "Not set".to_string()
        } else {
            format!("£{v}")
        }
    };
    Some(FieldEdit {
        addr: base_addr + offset as u64,
        old_raw: old_bytes.to_vec(),
        new_raw: new_bytes.to_vec(),
        description: format!("{label}: {} \u{2192} {}", show(old), show(new)),
    })
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
        profile_label(ui, label);
        ui.label(format!("{value}"));
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn calendar_math_and_range_boundaries() {
        for (year, february) in [(1900, 28), (2000, 29), (2024, 29), (2100, 28)] {
            assert_eq!(days_in_month(year, 2), february);
            for month in 1..=12 {
                for day in 1..=i32::from(days_in_month(year, month)) {
                    let doy = day_month_to_doy(year, day, month);
                    assert_eq!(doy_to_day_month(year, doy), (day, month));
                }
            }
        }
        assert_eq!(first_weekday(2000, 1), 5); // Saturday
        assert_eq!(first_weekday(2024, 2), 3); // Thursday
        assert_eq!(first_weekday(2026, 6), 0); // Monday
        let mut draft = CalendarDraft {
            year: 1900,
            month: 1,
            year_text: "1900".into(),
        };
        draft.shift_month(-1, &(1900..=2030));
        assert_eq!((draft.year, draft.month), (1900, 1));
        draft.shift_month(12, &(1900..=2030));
        assert_eq!((draft.year, draft.month), (1901, 1));
        draft.year = 2030;
        draft.month = 12;
        draft.shift_month(1, &(1900..=2030));
        assert_eq!((draft.year, draft.month), (2030, 12));
    }

    fn date_frame(
        ctx: &egui::Context,
        raw: &mut [u8],
        addr: u64,
        enabled: bool,
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, Vec<FieldEdit>) {
        let mut edits = Vec::new();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        edits = date_edit_inner(ui, "DOB", raw, 0, 2, addr, 1900..=2030);
                    });
                });
            },
        );
        (output, edits)
    }

    fn click_text(output: &egui::FullOutput, text: &str) -> Vec<egui::Event> {
        let texts = text_shapes(&output.shapes);
        let shape = texts
            .iter()
            .find(|shape| shape.galley.text() == text)
            .unwrap_or_else(|| panic!("missing {text}"));
        let pos = shape.pos + shape.galley.size() / 2.0;
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn calendar_browsing_cancel_and_selection_use_existing_edits() {
        let ctx = egui::Context::default();
        navigation::set_editing_enabled(&ctx, true);
        let mut raw = [0, 0, 208, 7];
        raw[..2].copy_from_slice(&(0x1A00u16 | 1).to_le_bytes());
        let original = raw;
        let (output, edits) = date_frame(&ctx, &mut raw, 100, true, vec![]);
        assert!(edits.is_empty());
        let (_, edits) = date_frame(&ctx, &mut raw, 100, true, click_text(&output, "Calendar"));
        assert!(edits.is_empty());
        let (output, _) = date_frame(&ctx, &mut raw, 100, true, vec![]);
        assert!(ctx.memory(|m| m.is_popup_open(calendar_id(100, 0))));
        let (_, edits) = date_frame(&ctx, &mut raw, 100, true, click_text(&output, ">"));
        assert!(edits.is_empty());
        assert_eq!(raw, original);
        let (output, _) = date_frame(&ctx, &mut raw, 100, true, vec![]);
        let (_, edits) = date_frame(&ctx, &mut raw, 100, true, click_text(&output, "Cancel"));
        assert!(edits.is_empty());
        assert_eq!(raw, original);
        let (output, _) = date_frame(&ctx, &mut raw, 100, true, vec![]);
        date_frame(&ctx, &mut raw, 100, true, click_text(&output, "Calendar"));
        let (output, _) = date_frame(&ctx, &mut raw, 100, true, vec![]);
        assert!(
            text_shapes(&output.shapes)
                .iter()
                .any(|s| s.galley.text() == "[1]")
        );
        let (_, edits) = date_frame(&ctx, &mut raw, 100, true, click_text(&output, "2"));
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].addr, 100);
        assert_eq!(edits[0].old_raw, original[..2]);
        assert_eq!(edits[0].new_raw, (0x1A00u16 | 2).to_le_bytes());
        assert_eq!(&raw[2..], &original[2..]);
    }

    #[test]
    fn calendar_identity_gating_and_invalid_dates() {
        assert_ne!(calendar_id(100, 0), calendar_id(200, 0));
        assert_ne!(calendar_id(100, 0), calendar_id(100, 4));
        for (editing, ancestor) in [(false, true), (true, false), (true, true)] {
            let ctx = egui::Context::default();
            navigation::set_editing_enabled(&ctx, editing);
            let id = calendar_id(100, 0);
            ctx.memory_mut(|m| m.open_popup(id));
            ctx.data_mut(|d| {
                d.insert_temp(
                    id,
                    CalendarDraft {
                        year: 2020,
                        month: 2,
                        year_text: "2020".into(),
                    },
                )
            });
            let mut raw = [1, 0, 208, 7];
            let original = raw;
            let (_, edits) = date_frame(&ctx, &mut raw, 200, ancestor, vec![]);
            assert!(edits.is_empty());
            assert_eq!(raw, original);
            assert!(
                ctx.data_mut(|d| d.get_temp::<CalendarDraft>(calendar_id(200, 0)))
                    .is_none()
            );
            let (_, edits) = date_frame(&ctx, &mut raw, 100, ancestor, vec![]);
            assert!(edits.is_empty());
            if !editing || !ancestor {
                assert!(!ctx.memory(|m| m.is_popup_open(id)));
            }
        }
        for (doy, year) in [(0u16, 2000u16), (366, 1900), (511, 2000), (1, 0), (1, 2031)] {
            let ctx = egui::Context::default();
            navigation::set_editing_enabled(&ctx, true);
            let mut raw = [0; 4];
            raw[..2].copy_from_slice(&doy.to_le_bytes());
            raw[2..].copy_from_slice(&year.to_le_bytes());
            let original = raw;
            let (output, edits) = date_frame(&ctx, &mut raw, 100, true, vec![]);
            assert!(edits.is_empty());
            assert_eq!(raw, original);
            assert!(
                !text_shapes(&output.shapes)
                    .iter()
                    .any(|s| s.galley.text() == "Calendar")
            );
        }
    }

    use super::replace_doy;

    #[test]
    fn replace_doy_keeps_flag_bits() {
        // Saka's contract-signed word: flags 0x1A00 | day 363 (0x16B).
        assert_eq!(replace_doy(7019, 181), 0x1A00 | 181);
        // A plain date has no flags to keep.
        assert_eq!(replace_doy(181, 1), 1);
    }

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

    fn text_shapes(shapes: &[egui::epaint::ClippedShape]) -> Vec<&egui::epaint::TextShape> {
        fn collect<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::epaint::TextShape>) {
            match shape {
                egui::Shape::Text(text) => out.push(text),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for shape in shapes {
            collect(&shape.shape, &mut out);
        }
        out
    }

    #[test]
    fn profile_values_align_and_render_without_edits() {
        const LABELS: [&str; 3] = [
            "Rep",
            "Home Reputation",
            "An exceptionally long reputation label that must be truncated",
        ];
        // Exercise each editor and its read-only fallback, with editing enabled.
        for kind in 0..10 {
            let ctx = egui::Context::default();
            navigation::set_editing_enabled(&ctx, true);
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for label in LABELS {
                        let mut field = field(Encoding::Direct, 0);
                        field.label = label;
                        let mut raw = if matches!(kind, 1 | 3 | 5) {
                            [250, 0, 0, 0]
                        } else {
                            [10, 0, 0, 0]
                        };
                        let original = raw;
                        let edit = match kind {
                            0 | 1 => attr_edit(ui, &field, &mut raw, 0),
                            2 | 3 => raw_u8_edit(ui, &field, &mut raw, 200, 0),
                            4 | 5 => raw_u16_edit(ui, &field, &mut raw, 200, 0),
                            6 => raw_u32_edit(ui, &field, &mut raw, 0),
                            7 => optional_u8_edit(ui, label, &mut raw, &field, 0, 99, 255),
                            8 => optional_money_edit(ui, label, &mut raw, &field, 0),
                            9 => {
                                info_label(ui, label, 10);
                                None
                            }
                            _ => unreachable!(),
                        };
                        assert!(edit.is_none(), "rendering kind {kind} emitted an edit");
                        assert_eq!(raw, original);
                    }
                });
            });
            let texts = text_shapes(&output.shapes);
            let value_text = if matches!(kind, 1 | 3 | 5) {
                "raw=250"
            } else if kind == 8 {
                "£10"
            } else {
                "10"
            };
            let values: Vec<_> = texts
                .iter()
                .filter(|t| t.galley.text() == value_text)
                .collect();
            assert_eq!(values.len(), LABELS.len(), "kind {kind}");
            for pair in values.windows(2) {
                assert_eq!(pair[0].pos.x, pair[1].pos.x, "kind {kind}");
                assert!(
                    (pair[1].pos.y - pair[0].pos.y - 25.0).abs() < 0.1,
                    "kind {kind}: rows must retain a consistent 22px height plus spacing"
                );
            }
            let labels: Vec<_> = LABELS
                .iter()
                .map(|label| texts.iter().find(|t| t.galley.text() == *label).unwrap())
                .collect();
            assert_eq!(labels[0].pos.x, labels[1].pos.x);
            assert_eq!(labels[0].pos.x, labels[2].pos.x);
            assert!(labels[2].galley.elided);
            assert!(labels[2].galley.size().x <= 130.0);
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
