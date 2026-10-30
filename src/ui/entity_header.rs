//! Shared, non-scrolling identity and relationship header for detail panels.
use crate::fields::FieldLocation;
use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    name: &str,
    kind: &str,
    uid: impl std::fmt::Display,
    age: Option<u8>,
    club: Option<&str>,
    club_addr: Option<u64>,
    nation: Option<&str>,
) {
    ui.heading(name);
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("{kind} · UID: {uid}"));
        if let Some(age) = age {
            ui.label(format!("Age: {age}"));
        }
    });
    ui.horizontal_wrapped(|ui| {
        if kind != "Club" {
            crate::ui::navigation::club_link(ui, club, club_addr);
            ui.separator();
        }
        ui.label(format!(
            "Nation: {}",
            nation.filter(|s| !s.trim().is_empty()).unwrap_or("Unknown")
        ))
        .on_hover_text("Nation pages are not yet supported.");
    });
    ui.add_space(8.0);
}

/// Approximate age, using the schema's reference year, only for a valid mapped DOB.
pub fn age(schemas: &'static crate::fields::VersionedSchemas, raw: &[u8], key: &str) -> Option<u8> {
    let FieldLocation::Direct { offset, .. } = schemas.field(key)?.offset? else {
        return None;
    };
    age_at(raw, offset as usize, schemas.age_reference_year)
}

fn age_at(raw: &[u8], offset: usize, reference_year: u16) -> Option<u8> {
    let bytes = raw.get(offset..offset.checked_add(4)?)?;
    let day = u16::from_le_bytes([bytes[0], bytes[1]]);
    let year = u16::from_le_bytes([bytes[2], bytes[3]]);
    if !(1..=366).contains(&day) || year < 1900 || year > reference_year {
        return None;
    }
    u8::try_from(reference_year.checked_sub(year)?)
        .ok()
        .filter(|age| *age <= 120)
}

/// Keep desktop density without asking egui for negative-width columns.
pub fn column_count(width: f32) -> usize {
    if width >= 960.0 {
        4
    } else if width >= 480.0 {
        2
    } else {
        1
    }
}

/// Render four conceptual groups in rows of four, two, or one.
pub fn columns(ui: &mut egui::Ui, mut render: impl FnMut(usize, &mut egui::Ui)) {
    let count = column_count(ui.available_width());
    for start in (0..4).step_by(count) {
        ui.columns(count, |cols| {
            for (index, col) in cols.iter_mut().enumerate() {
                render(start + index, col);
            }
        });
        ui.add_space(12.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn responsive_columns_preserve_dense_and_narrow_layouts() {
        for (width, expected) in [
            (0.0, 1),
            (20.0, 1),
            (479.0, 1),
            (480.0, 2),
            (959.0, 2),
            (960.0, 4),
            (1400.0, 4),
        ] {
            assert_eq!(column_count(width), expected);
        }
    }
    #[test]
    fn age_requires_complete_valid_dob() {
        assert_eq!(age_at(&[], 0, 2026), None);
        assert_eq!(age_at(&[1, 0, 208, 7], usize::MAX, 2026), None);
        assert_eq!(age_at(&[1, 0, 208, 7], 0, 2026), Some(26));
        assert_eq!(age_at(&[0, 0, 208, 7], 0, 2026), None);
        assert_eq!(age_at(&[1, 0, 255, 255], 0, 2026), None);
    }
}
