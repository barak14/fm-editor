//! Read-only x64 MSVC RTTI discovery from PE image sections.

use std::collections::HashMap;
use std::time::Instant;

use crate::player::is_valid_ptr;

use super::FmProcess;
use super::layout_detect::{Section, module_read, sections};

const SCN_INITIALIZED_DATA: u32 = 0x0000_0040;
const SCN_EXECUTE: u32 = 0x2000_0000;
const MAX_SECTION_SIZE: u64 = 64 * 1024 * 1024;
const TYPE_NAME_OFFSET: u64 = 16;
const COL_SIZE: usize = 24;
const MAX_TYPE_NAME: usize = 1024;

pub(crate) const FINANCE: &str = "db::CLUB_FINANCE";
pub(crate) const PLAYER_COACH: &str = "db::ACTUAL_PLAYER_AND_NON_PLAYER";
pub(crate) const HUMAN_MANAGER: &str = "db::HUMAN_NON_PLAYER";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Vtable {
    pub offset: u32,
    pub rva: u64,
}

#[derive(Default)]
pub(crate) struct Index {
    classes: HashMap<String, Vec<Vtable>>,
}

impl Index {
    pub(crate) fn class(&self, class: &str) -> Option<&[Vtable]> {
        self.classes.get(class).map(Vec::as_slice)
    }
}

pub(crate) fn index(fm: &FmProcess) -> &Index {
    fm.rtti.get_or_init(|| {
        let started = Instant::now();
        let parsed = sections(fm).map(|sections| read_index(fm, &sections));
        let result = match parsed {
            Ok(result) => result,
            Err(error) => {
                fm.log_detection_note(&format!("MSVC RTTI section walk failed: {error}"));
                Index::default()
            }
        };
        fm.log_detection_note(&format!(
            "MSVC RTTI: {} classes in {:.2} s",
            result.classes.len(),
            started.elapsed().as_secs_f64()
        ));
        result
    })
}

fn eligible(section: &Section) -> Option<usize> {
    let size = section.end.checked_sub(section.start)?;
    (section.flags & SCN_INITIALIZED_DATA != 0
        && section.flags & SCN_EXECUTE == 0
        && (1..=MAX_SECTION_SIZE).contains(&size))
    .then_some(size as usize)
}

fn read_index(fm: &FmProcess, sections: &[Section]) -> Index {
    let buffers: Vec<_> = sections
        .iter()
        .filter_map(|section| {
            Some((
                section,
                module_read(fm, section.start, eligible(section)?).ok()?,
            ))
        })
        .collect();
    let sections: Vec<_> = buffers
        .iter()
        .map(|(section, bytes)| (*section, bytes.as_slice()))
        .collect();
    parse(fm.gp_base, &sections)
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn canonical_name(name: &[u8]) -> Option<String> {
    let name = std::str::from_utf8(name.strip_suffix(b"@@")?).ok()?;
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_graphic()) {
        return None;
    }
    // Only simple qualified names can be demangled by reversing their components.
    let parts: Vec<_> = name.split('@').collect();
    if parts.iter().all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    }) {
        return Some(parts.into_iter().rev().collect::<Vec<_>>().join("::"));
    }
    Some(name.to_owned())
}

fn parse(base: u64, sections: &[(&Section, &[u8])]) -> Index {
    let sections: Vec<_> = sections
        .iter()
        .filter_map(|&(section, bytes)| {
            let len = bytes.len().min(eligible(section)?);
            Some((section.start, bytes.get(..len)?))
        })
        .collect();
    let mut types = HashMap::new();
    for &(start, bytes) in &sections {
        for (offset, _) in bytes
            .iter()
            .enumerate()
            .skip(TYPE_NAME_OFFSET as usize)
            .filter(|&(_, &byte)| byte == b'.')
        {
            let Some(tail) = bytes.get(offset..) else {
                continue;
            };
            let Some(name) = tail
                .strip_prefix(b".?AV")
                .or_else(|| tail.strip_prefix(b".?AU"))
            else {
                continue;
            };
            let limit = name.len().min(MAX_TYPE_NAME);
            let Some(end) = name[..limit].iter().position(|&byte| byte == 0) else {
                continue;
            };
            if let Some(name) = canonical_name(&name[..end])
                && let Some(rva) = start.checked_add(offset as u64 - TYPE_NAME_OFFSET)
                && let Ok(rva) = u32::try_from(rva)
            {
                types.insert(rva, name);
            }
        }
    }
    if types.is_empty() {
        return Index::default();
    }
    let mut locators = HashMap::new();
    for &(start, bytes) in &sections {
        for offset in
            (alignment_offset(start, 4)..bytes.len().saturating_sub(COL_SIZE - 1)).step_by(4)
        {
            let col = &bytes[offset..offset + COL_SIZE];
            if col[..4] == [1, 0, 0, 0]
                && let Some(rva) = start.checked_add(offset as u64)
                && u32_at(col, 20).map(u64::from) == Some(rva)
                && let Some(class) = u32_at(col, 12).and_then(|ty| types.get(&ty))
                && let Some(absolute) = base.checked_add(rva).filter(|&ptr| is_valid_ptr(ptr))
                && let Some(subobject) = u32_at(col, 4)
            {
                locators.insert(absolute, (class, subobject));
            }
        }
    }
    let mut index = Index::default();
    if locators.is_empty() {
        return index;
    }
    for &(start, bytes) in &sections {
        let align = alignment_offset(start, 8);
        let Some(aligned_bytes) = bytes.get(align..) else {
            continue;
        };
        let slots = aligned_bytes.chunks_exact(8);
        let with_vtable = slots.len().saturating_sub(1);
        for (slot, word) in slots.take(with_vtable).enumerate() {
            let absolute = u64::from_le_bytes(word.try_into().expect("eight-byte chunk"));
            if let Some(&(name, offset)) = locators.get(&absolute)
                && let Some(rva) = start.checked_add((align + slot * 8 + 8) as u64)
                && base
                    .checked_add(rva)
                    .and_then(|ptr| ptr.checked_add(7))
                    .is_some_and(is_valid_ptr)
            {
                index
                    .classes
                    .entry(name.clone())
                    .or_default()
                    .push(Vtable { offset, rva });
            }
        }
    }
    for class in index.classes.values_mut() {
        class.sort_unstable();
        class.dedup();
    }
    index
}

fn alignment_offset(start: u64, alignment: u64) -> usize {
    (start.wrapping_neg() % alignment) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_section(start: u64, size: u64) -> Section {
        Section {
            name: "arbitrary".into(),
            start,
            end: start + size,
            flags: SCN_INITIALIZED_DATA,
        }
    }

    fn put(bytes: &mut [u8], offset: usize, value: &[u8]) {
        bytes[offset..offset + value.len()].copy_from_slice(value);
    }

    #[test]
    fn parses_classes_across_sections_and_rejects_invalid_locators() {
        let base = 0x1234_0000_u64;
        let mut types = [0; 192];
        put(&mut types, 16, b".?AVCLUB_FINANCE@db@@\0");
        put(&mut types, 80, b".?AUHelper@db@outer@@\0");
        put(&mut types, 144, b".?AVInvalid@db\0");
        let mut tables = [0; 256];
        for (offset, signature, subobject, td, own_rva) in [
            (0, 1_u32, 0_u32, 0x1000_u32, 0x2000_u32),
            (24, 1, 0x440, 0x1040, 0x2018),
            (48, 1, 0x468, 0x1040, 0x2030),
            (72, 1, 0, 0x1000, 0x2000),
            (96, 0, 0, 0x1000, 0x2060),
            (120, 1, 0, 0x1080, 0x2078),
        ] {
            put(&mut tables, offset, &signature.to_le_bytes());
            put(&mut tables, offset + 4, &subobject.to_le_bytes());
            put(&mut tables, offset + 12, &td.to_le_bytes());
            put(&mut tables, offset + 20, &own_rva.to_le_bytes());
            put(
                &mut tables,
                160 + offset / 3,
                &(base + 0x2000 + offset as u64).to_le_bytes(),
            );
        }
        put(&mut tables, 213, &(base + 0x2000).to_le_bytes());
        put(&mut tables, 248, &(base + 0x2000).to_le_bytes());
        let data = data_section(0x1000, types.len() as u64);
        let vtables = data_section(0x2000, tables.len() as u64);
        let excluded = Section {
            flags: SCN_INITIALIZED_DATA | SCN_EXECUTE,
            ..data_section(0x3000, 256)
        };
        let too_big = data_section(0x4000, MAX_SECTION_SIZE + 1);
        let uninitialized = Section {
            flags: 0,
            ..data_section(0x5000, 256)
        };
        let mut false_slot = [0; 16];
        put(&mut false_slot, 0, &(base + 0x2000).to_le_bytes());
        let index = parse(
            base,
            &[
                (&data, &types),
                (&vtables, &tables),
                (&excluded, &false_slot),
                (&too_big, &false_slot),
                (&uninitialized, &false_slot),
            ],
        );
        assert_eq!(index.classes.len(), 2);
        assert_eq!(
            index.class(FINANCE),
            Some(
                [Vtable {
                    offset: 0,
                    rva: 0x20a8
                }]
                .as_slice()
            )
        );
        assert_eq!(
            index.class("outer::db::Helper"),
            Some(
                [
                    Vtable {
                        offset: 0x440,
                        rva: 0x20b0
                    },
                    Vtable {
                        offset: 0x468,
                        rva: 0x20b8
                    },
                ]
                .as_slice()
            )
        );
        assert!(
            parse(base, &[(&data, &types), (&vtables, &tables[..16])])
                .classes
                .is_empty()
        );
        assert!(
            parse(u64::MAX, &[(&data, &types), (&vtables, &tables)])
                .classes
                .is_empty()
        );
    }

    #[test]
    #[ignore = "needs the installed FM27 and FM26 Steam DLLs"]
    fn installed_steam_modules() {
        fn from_file(bytes: &[u8]) -> Index {
            let pe = u32_at(bytes, 0x3c).unwrap() as usize;
            assert_eq!(&bytes[pe..pe + 4], b"PE\0\0");
            let u16_at =
                |offset| u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
            let count = u16_at(pe + 6);
            let optional = pe + 24;
            let table = optional + u16_at(pe + 20);
            assert_eq!(u16_at(optional), 0x20b);
            let image_base =
                u64::from_le_bytes(bytes[optional + 24..optional + 32].try_into().unwrap());
            let sections: Vec<_> = bytes[table..table + count * 40]
                .chunks_exact(40)
                .filter_map(|header| {
                    let start = u64::from(u32_at(header, 12)?);
                    let size = u64::from(u32_at(header, 8)?);
                    let section = Section {
                        flags: u32_at(header, 36)?,
                        ..data_section(start, size)
                    };
                    let raw_start = u32_at(header, 20)? as usize;
                    let raw_len = (u32_at(header, 16)? as usize).min(eligible(&section)?);
                    Some((section, bytes.get(raw_start..raw_start + raw_len)?))
                })
                .collect();
            parse(
                image_base,
                &sections
                    .iter()
                    .map(|(section, data)| (section, *data))
                    .collect::<Vec<_>>(),
            )
        }

        let steam = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
            .join(".local/share/Steam/steamapps/common");
        // FM27 Steam 27.0.0+2493426: installed DLL, with finance and player-coach corroborated by
        // Jackhamerp24/Football-Manager-27-Editor offsets/27.0.0+2493426-steam.json (80f2d27, 2026-11-23), not checked live here.
        let fm27: &[(&str, &[(u32, u64)])] = &[
            ("db::ACTUAL_PLAYER", &[(0, 0x4D29228)]),
            (PLAYER_COACH, &[(0, 0x4F77A08)]),
            (FINANCE, &[(0, 0x4CF2CC8)]),
            ("db::CLUB_FINANCE_BASE", &[(0, 0x4CF27F8)]),
            ("db::CLUB", &[(0, 0x4CDC1F8)]),
            ("db::STADIUM", &[(0, 0x4DEFC18)]),
            (
                "db::ACTUAL_NON_PLAYER",
                &[(256, 0x4D8DE78), (296, 0x4D8DE28)],
            ),
            (HUMAN_MANAGER, &[(1088, 0x4CC28D8), (1128, 0x4CC2888)]),
        ];
        // FM26 Steam 26.3.2+2329565: installed DLL, measured 2026-11-23, not checked live here.
        let fm26: &[(&str, &[(u32, u64)])] = &[
            (FINANCE, &[(0, 0x456D2E8)]),
            (PLAYER_COACH, &[(0, 0x4785308)]),
            ("db::CLUB", &[(0, 0x44BA518)]),
        ];
        for (version, expected) in [(27, fm27), (26, fm26)] {
            let path = steam.join(format!(
                "Football Manager {version}/fm_Data/Plugins/x86_64/game_plugin.dll"
            ));
            let index = from_file(&std::fs::read(path).unwrap());
            println!("FM{version}: {} RTTI classes", index.classes.len());
            for &(class, vtables) in expected {
                let actual: Vec<_> = index
                    .class(class)
                    .unwrap_or_default()
                    .iter()
                    .map(|v| (v.offset, v.rva))
                    .collect();
                for &want in vtables {
                    assert_eq!(
                        actual
                            .iter()
                            .copied()
                            .filter(|&(offset, _)| offset == want.0)
                            .collect::<Vec<_>>(),
                        [want],
                        "FM{version} {class}",
                    );
                }
                println!("  {class}: {actual:X?}");
            }
        }
    }
}
