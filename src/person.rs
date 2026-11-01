//! Person-block data shared by staff and players (spoken languages and the
//! shared name records behind first, last and common names), read through
//! the attached build's `PersonProfileLayout`.

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

/// One shared name record. People point at these instead of owning their
/// name strings, so a name is changed by pointing the person at another
/// existing record, never by editing a record's text (that would rename
/// everyone who shares it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameRecord {
    pub addr: u64,
    pub text: String,
    /// People in the player and staff arrays found pointing at this record
    /// when the index was built.
    pub uses: u32,
}

/// Name-slot keys per entity, in display order: first, last, common.
pub const PLAYER_NAME_SLOTS: [(&str, &str); 3] = [
    ("p_name_first_sub", "First name"),
    ("p_name_last_sub", "Last name"),
    ("p_name_common_sub", "Common name"),
];
pub const STAFF_NAME_SLOTS: [(&str, &str); 3] = [
    ("s_name_first_sub", "First name"),
    ("s_name_last_sub", "Last name"),
    ("s_name_common_sub", "Common name"),
];

/// Text of the name record at `record`, validated like every other pointer.
pub fn read_name_record_text(fm: &FmProcess, record: u64) -> Option<String> {
    if !valid(record) {
        return None;
    }
    let text = fm.read_u64(record).ok()?;
    fm.read_fm_name(text).ok()
}

/// Collect every name record that a player or staff member points at,
/// deduplicated and sorted by text (then by use count, most used first).
/// `None` when the build has no name layout or the arrays cannot be read.
/// Unreadable people or records are skipped, never guessed.
pub fn build_name_index(
    fm: &FmProcess,
    players: (u64, usize),
    staff: (u64, usize),
) -> Option<Vec<NameRecord>> {
    let schema = fm.schemas();
    schema.person_profile?;
    let slots = |keys: &[(&str, &str); 3]| -> Option<Vec<usize>> {
        keys.iter().map(|(k, _)| schema.offset_of(k)).collect()
    };
    let player_slots = slots(&PLAYER_NAME_SLOTS)?;
    let staff_slots = slots(&STAFF_NAME_SLOTS)?;
    let mut uses: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
    let mut add = |base: u64, offsets: &[usize]| {
        for &off in offsets {
            if let Some(r) = base
                .checked_add(off as u64)
                .and_then(|a| fm.read_u64(a).ok())
            {
                if valid(r) {
                    *uses.entry(r).or_default() += 1;
                }
            }
        }
    };
    let pointers = |(data, count): (u64, usize)| -> Option<Vec<u64>> {
        let bytes = fm.read_bytes(data, count.checked_mul(8)?).ok()?;
        Some(
            bytes
                .chunks_exact(8)
                .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        )
    };
    for p in pointers(players)? {
        if !valid(p) {
            continue;
        }
        let shift = crate::player::person_shift(fm, p);
        let offsets: Vec<usize> = player_slots
            .iter()
            .map(|&o| crate::player::player_phys_offset(schema, o as u64, shift) as usize)
            .collect();
        add(p, &offsets);
    }
    for s in pointers(staff)? {
        if !valid(s) {
            continue;
        }
        let shift = crate::staff::identity_shift(fm, s);
        let offsets: Vec<usize> = staff_slots.iter().map(|&o| o + shift).collect();
        add(s, &offsets);
    }
    let mut out: Vec<NameRecord> = uses
        .into_iter()
        .filter_map(|(addr, uses)| {
            Some(NameRecord {
                addr,
                text: read_name_record_text(fm, addr)?,
                uses,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        a.text
            .cmp(&b.text)
            .then(b.uses.cmp(&a.uses))
            .then(a.addr.cmp(&b.addr))
    });
    Some(out)
}

/// Records matching `query` (case-insensitive substring), exact matches
/// first, then prefix matches, then the rest; at most `limit`. Several
/// records can hold the same text (FM keeps them per name group); only the
/// most used one is offered, so each spelling appears once.
pub fn search_names<'a>(index: &'a [NameRecord], query: &str, limit: usize) -> Vec<&'a NameRecord> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, &NameRecord)> = index
        .iter()
        .filter_map(|r| {
            let t = r.text.to_lowercase();
            let rank = if t == q {
                0
            } else if t.starts_with(&q) {
                1
            } else if t.contains(&q) {
                2
            } else {
                return None;
            };
            Some((rank, r))
        })
        .collect();
    hits.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(b.1.uses.cmp(&a.1.uses))
            .then(a.1.text.cmp(&b.1.text))
    });
    let mut seen = std::collections::HashSet::new();
    hits.into_iter()
        .filter(|(_, r)| seen.insert(r.text.as_str()))
        .take(limit)
        .map(|(_, r)| r)
        .collect()
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
    fn name_search_ranks_exact_then_prefix_then_substring() {
        let rec = |addr, text: &str, uses| NameRecord {
            addr,
            text: text.into(),
            uses,
        };
        let index = vec![
            rec(1, "Ann", 3),
            rec(2, "Hannah", 9),
            rec(3, "Anna", 2),
            rec(4, "Anna", 7),
            rec(5, "Bob", 1),
        ];
        let got: Vec<u64> = search_names(&index, "ANN", 10)
            .iter()
            .map(|r| r.addr)
            .collect();
        assert_eq!(got, vec![1, 4, 2]);
        assert_eq!(search_names(&index, "ann", 2).len(), 2);
        assert!(search_names(&index, "  ", 10).is_empty());
    }

    #[test]
    fn no_nation_keeps_own_list() {
        let v = order_languages(&PERSON_PROFILE, None, own(&[("German", 2)]));
        assert_eq!(format_languages(&v), "German (Basic)");
    }
}
