//! Name editing by re-pointing a person's first, last or common name at
//! another existing shared name record, chosen through a search box.
//!
//! Name records are shared (one "Frank" record serves every Frank), so the
//! editor never changes a record's text: it only swaps the person's 8-byte
//! record pointer, through the normal acknowledged-write and Undo path.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;

use crate::fields::Field;
use crate::person::{search_names, NameRecord};
use crate::ui::field_widget::FieldEdit;

/// Most search results listed under a name slot.
const MAX_RESULTS: usize = 12;

/// Name index state for the attached save, owned by the app.
#[derive(Default)]
pub struct NamePicker {
    index: Option<Arc<Vec<NameRecord>>>,
    by_addr: HashMap<u64, usize>,
    loading: bool,
    error: Option<String>,
    /// Set by a panel when the user asks for names; the app sends the load.
    wants_load: bool,
    /// (entity address, slot key) whose search box is open.
    open: Option<(u64, &'static str)>,
    query: String,
}

impl NamePicker {
    /// Forget everything tied to the previous save or attachment.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// True once when a panel asked for the index and no load is running.
    pub fn take_load_request(&mut self) -> bool {
        let start = self.wants_load && !self.loading && self.index.is_none();
        self.wants_load = false;
        if start {
            self.loading = true;
            self.error = None;
        }
        start
    }

    pub fn receive(&mut self, result: Result<Arc<Vec<NameRecord>>, String>) {
        self.loading = false;
        match result {
            Ok(index) => {
                self.by_addr = index.iter().enumerate().map(|(i, r)| (r.addr, i)).collect();
                self.index = Some(index);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn text_of(&self, addr: u64) -> Option<&str> {
        let index = self.index.as_ref()?;
        self.by_addr.get(&addr).map(|&i| index[i].text.as_str())
    }
}

/// "Names" section: one row per slot. Returns the edits the user picked.
/// `base_addr` is the entity address; `raw` its schema-layout buffer. The
/// caller maps edits to physical addresses (player-coach and human-manager
/// shifts) exactly as for every other field.
pub fn names_section(
    schemas: &'static crate::fields::VersionedSchemas,
    ui: &mut egui::Ui,
    picker: &mut NamePicker,
    slots: &[(&'static str, &'static str); 3],
    raw: &mut [u8],
    base_addr: u64,
) -> Vec<FieldEdit> {
    let mut edits = Vec::new();
    let fields: Vec<(&'static Field, &'static str, &'static str)> = slots
        .iter()
        .filter_map(|&(key, label)| Some((schemas.field(key)?, key, label)))
        .filter(|(f, _, _)| f.offset.is_some())
        .collect();
    if fields.is_empty() || schemas.person_profile.is_none() {
        return edits;
    }
    ui.strong("Names");
    if picker.index.is_none() {
        ui.horizontal(|ui| {
            if picker.loading {
                ui.spinner();
                ui.label("Reading name records\u{2026}");
            } else if ui
                .button("Show and change names")
                .on_hover_text("Reads every shared name record once for this save (a few seconds).")
                .clicked()
            {
                picker.wants_load = true;
            }
        });
        if let Some(e) = &picker.error {
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Names unavailable: {e}"));
        }
        return edits;
    }
    for (field, key, label) in fields {
        if let Some(edit) = slot_row(ui, picker, field, key, label, raw, base_addr) {
            edits.push(edit);
        }
    }
    edits
}

fn read_ptr(raw: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(raw.get(offset..end)?.try_into().ok()?))
}

fn slot_row(
    ui: &mut egui::Ui,
    picker: &mut NamePicker,
    field: &'static Field,
    key: &'static str,
    label: &'static str,
    raw: &mut [u8],
    base_addr: u64,
) -> Option<FieldEdit> {
    let offset = usize::try_from(field.offset?.offset()).ok()?;
    let old = read_ptr(raw, offset)?;
    let current = match old {
        0 => "\u{2014}".to_owned(),
        p => picker
            .text_of(p)
            .map(str::to_owned)
            .unwrap_or_else(|| "(unknown record)".into()),
    };
    let slot = (base_addr, key);
    let is_open = picker.open == Some(slot);
    let mut picked: Option<(u64, String)> = None;
    ui.push_id(("name_slot", base_addr, key), |ui| {
        ui.horizontal(|ui| {
            crate::ui::field_widget::profile_label(ui, label);
            ui.label(&current);
            if ui
                .small_button("\u{1F50D}")
                .on_hover_text("Pick another existing name")
                .clicked()
            {
                if is_open {
                    picker.open = None;
                } else {
                    picker.open = Some(slot);
                    picker.query.clear();
                }
            }
            // FM leaves the common name unset for most people.
            if key.ends_with("common_sub") && old != 0 && ui.small_button("Clear").clicked() {
                picked = Some((0, "\u{2014}".into()));
            }
        });
        if picker.open == Some(slot) {
            let response = ui.add(
                egui::TextEdit::singleline(&mut picker.query)
                    .hint_text("Search existing names")
                    .desired_width(220.0),
            );
            if !is_open {
                response.request_focus();
            }
            if let Some(index) = picker.index.clone() {
                let hits = search_names(&index, &picker.query, MAX_RESULTS);
                if hits.is_empty() && !picker.query.trim().is_empty() {
                    ui.weak("No existing name matches. New names cannot be created.");
                }
                for r in hits {
                    let text = format!("{}  (used by {})", r.text, r.uses);
                    if ui.selectable_label(r.addr == old, text).clicked() && r.addr != old {
                        picked = Some((r.addr, r.text.clone()));
                    }
                }
            }
        }
    });
    let (new, new_text) = picked?;
    picker.open = None;
    let end = offset.checked_add(8)?;
    let new_raw = new.to_le_bytes().to_vec();
    let edit = FieldEdit {
        addr: base_addr.checked_add(offset as u64)?,
        old_raw: raw.get(offset..end)?.to_vec(),
        new_raw: new_raw.clone(),
        description: format!("{label}: {current} \u{2192} {new_text}"),
    };
    raw[offset..end].copy_from_slice(&new_raw);
    Some(edit)
}
