//! Person-block data shared by staff and players (spoken languages), read
//! through the attached build's `PersonProfileLayout`.

use crate::fields::PersonProfileLayout;
use crate::process::FmProcess;

/// Most language entries accepted from one person or nation vector. The
/// largest observed list is 14 (FM27 staff); anything far above is treated
/// as a corrupt vector rather than read.
const MAX_LANGUAGES: u64 = 64;

/// One language as FM's profile shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    pub name: String,
    pub level: &'static str,
}

fn valid(p: u64) -> bool {
    p > 0x10000 && p < 0x7FFF_FFFF_FFFF
}

/// Read a `(begin, end)` vector at `addr` and return its element addresses
/// for `stride`-byte elements. Empty (0, 0) vectors give an empty list;
/// anything malformed gives `None`.
fn vector_elements(fm: &FmProcess, addr: u64, stride: u64) -> Option<Vec<u64>> {
    let buf = fm.read_bytes(addr, 16).ok()?;
    let begin = u64::from_le_bytes(buf[..8].try_into().ok()?);
    let end = u64::from_le_bytes(buf[8..16].try_into().ok()?);
    if begin == 0 && end == 0 {
        return Some(Vec::new());
    }
    if !valid(begin) || !valid(end) || end < begin || (end - begin) % stride != 0 {
        return None;
    }
    let count = (end - begin) / stride;
    if count > MAX_LANGUAGES {
        return None;
    }
    Some((0..count).map(|i| begin + i * stride).collect())
}

fn language_name(fm: &FmProcess, layout: &PersonProfileLayout, language: u64) -> Option<String> {
    if !valid(language) {
        return None;
    }
    let name_ptr = fm.read_u64(language + layout.language_name as u64).ok()?;
    fm.read_fm_name(name_ptr).ok()
}

/// Languages of the person block at `person`, ordered as FM's Personal tab:
/// the nation's native language first (Fluent) unless the person's own list
/// has it, then the person's languages by level, highest first. Levels FM
/// hides are left out. `None` when the build has no layout or the person's
/// vector cannot be read; a missing nation only drops the native language.
pub fn read_languages(fm: &FmProcess, person: u64) -> Option<Vec<Language>> {
    let layout = fm.schemas().person_profile?;
    if !valid(person) {
        return None;
    }
    let entries = vector_elements(fm, person + layout.languages_vec as u64, 8)?;
    let mut own = Vec::with_capacity(entries.len());
    for slot in entries {
        let entry = fm.read_u64(slot).ok()?;
        if !valid(entry) {
            return None;
        }
        let language = fm.read_u64(entry).ok()?;
        let level = fm.read_u32(entry + layout.language_level as u64).ok()?;
        let name = language_name(fm, layout, language)?;
        own.push((name, level));
    }
    let native = native_language(fm, layout, person);
    Some(order_languages(layout, native, own))
}

fn native_language(fm: &FmProcess, layout: &PersonProfileLayout, person: u64) -> Option<String> {
    let nation = fm.read_u64(person + layout.nation_ptr as u64).ok()?;
    if !valid(nation) {
        return None;
    }
    let entries = vector_elements(
        fm,
        nation + layout.nation_languages_vec as u64,
        layout.nation_language_size as u64,
    )?;
    let first = *entries.first()?;
    language_name(fm, layout, fm.read_u64(first).ok()?)
}

/// Pure ordering step of `read_languages`, split out for tests.
fn order_languages(
    layout: &PersonProfileLayout,
    native: Option<String>,
    mut own: Vec<(String, u32)>,
) -> Vec<Language> {
    // Stable: equal levels keep the person's vector order, as FM does.
    own.sort_by(|a, b| b.1.cmp(&a.1));
    let mut out = Vec::with_capacity(own.len() + 1);
    if let Some(native) = native {
        if !own.iter().any(|(name, _)| *name == native) {
            if let Some(&(_, top)) = layout.language_levels.last() {
                out.push(Language {
                    name: native,
                    level: top,
                });
            }
        }
    }
    for (name, level) in own {
        if let Some(label) = layout.language_level_label(level) {
            out.push(Language { name, level: label });
        }
    }
    out
}

/// "Italian (Fluent), English (Good)" for display.
pub fn format_languages(languages: &[Language]) -> String {
    languages
        .iter()
        .map(|l| format!("{} ({})", l.name, l.level))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::fm27::staff::PERSON_PROFILE;

    fn own(list: &[(&str, u32)]) -> Vec<(String, u32)> {
        list.iter().map(|&(n, l)| (n.to_owned(), l)).collect()
    }

    fn shown(v: &[Language]) -> Vec<(String, &'static str)> {
        v.iter().map(|l| (l.name.clone(), l.level)).collect()
    }

    #[test]
    fn native_language_first_unless_listed() {
        // Flick: Germany's native German is not in his list (FM27 witness).
        let flick = order_languages(
            &PERSON_PROFILE,
            Some("German".into()),
            own(&[("English", 10), ("Spanish", 10)]),
        );
        assert_eq!(
            format_languages(&flick),
            "German (Fluent), English (Fluent), Spanish (Fluent)"
        );
        // Guardiola: Spanish is listed, so it is not prepended; German (5)
        // sorts after the level-10 languages.
        let pep = order_languages(
            &PERSON_PROFILE,
            Some("Spanish".into()),
            own(&[
                ("English", 10),
                ("Italian", 10),
                ("German", 5),
                ("Catalan", 10),
                ("Spanish", 10),
            ]),
        );
        assert_eq!(
            format_languages(&pep),
            "English (Fluent), Italian (Fluent), Catalan (Fluent), Spanish (Fluent), German (Good)"
        );
    }

    #[test]
    fn hidden_levels_and_bands() {
        // Conceição: Dutch 1 and Arabic 1 are hidden; Spanish 9 is Fluent.
        let v = order_languages(
            &PERSON_PROFILE,
            Some("Portuguese".into()),
            own(&[
                ("Italian", 10),
                ("French", 2),
                ("Dutch", 1),
                ("Arabic", 1),
                ("Spanish", 9),
            ]),
        );
        assert_eq!(
            shown(&v),
            vec![
                ("Portuguese".to_owned(), "Fluent"),
                ("Italian".to_owned(), "Fluent"),
                ("Spanish".to_owned(), "Fluent"),
                ("French".to_owned(), "Basic"),
            ]
        );
        for (level, label) in [
            (0, None),
            (1, None),
            (2, Some("Basic")),
            (4, Some("Basic")),
            (5, Some("Good")),
            (6, Some("Good")),
            (7, Some("Fluent")),
            (10, Some("Fluent")),
        ] {
            assert_eq!(
                PERSON_PROFILE.language_level_label(level),
                label,
                "level {level}"
            );
        }
    }

    #[test]
    fn no_nation_keeps_own_list() {
        let v = order_languages(&PERSON_PROFILE, None, own(&[("German", 2)]));
        assert_eq!(format_languages(&v), "German (Basic)");
    }
}
