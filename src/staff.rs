//! Read and write staff records from fm.exe via ptr_staff.

use crate::fields::{self, Category, Entity, HasSubRecords, SubRecordId};
use crate::process::FmProcess;

/// A decoded staff record.
#[derive(Debug, Clone)]
pub struct StaffRecord {
    pub build: crate::game_version::GameBuild,
    pub addr: u64,
    pub index: usize,
    /// Raw bytes for the full struct, in schema layout (see `read_staff_raw`).
    pub raw: Vec<u8>,
    /// Bytes a human manager's identity fields are moved by in memory (0 for
    /// AI staff); edits to `manager_shifted_fields` must add it.
    pub identity_shift: usize,
}

impl HasSubRecords for StaffRecord {
    fn sub_record(&self, _id: SubRecordId) -> Option<&[u8]> {
        None
    }
}

impl StaffRecord {
    /// Read a field by key, decoded to displayed value (u8).
    pub fn field_u8(&self, key: &str) -> Option<u8> {
        let f = self.build.schemas().field(key)?;
        fields::read_field_u8(self, &self.raw, f)
    }

    /// Read a field by key as u32.
    pub fn field_u32(&self, key: &str) -> Option<u32> {
        let f = self.build.schemas().field(key)?;
        fields::read_field_u32(self, &self.raw, f)
    }

    /// Read a field by key as u64.
    pub fn field_u64(&self, key: &str) -> Option<u64> {
        let f = self.build.schemas().field(key)?;
        fields::read_field_u64(self, &self.raw, f)
    }

    /// CA from the struct.
    pub fn ca(&self) -> u8 {
        self.field_u8("s_ca").unwrap_or(0)
    }

    /// PA from the struct.
    pub fn pa(&self) -> u8 {
        self.field_u8("s_pa").unwrap_or(0)
    }

    /// Staff database ID.
    pub fn id(&self) -> u32 {
        self.field_u32("s_id").unwrap_or(0)
    }

    /// Read all mapped fields for a category, returning (label, decoded_value).
    pub fn category(&self, category: Category) -> Vec<(&'static str, u8)> {
        self.build.schemas().fields_for_category(Entity::Staff, category)
            .iter()
            .filter_map(|f| {
                let val = fields::read_field_u8(self, &self.raw, f)?;
                Some((f.label, val))
            })
            .collect()
    }

    /// All non-personality attributes as (label, displayed, raw).
    pub fn attributes(&self) -> Vec<(&'static str, u8, u8)> {
        let attr_categories = [
            Category::Coaching, Category::Mental, Category::Knowledge, Category::Medical,
            Category::TacticalStyle, Category::BoardRelations,
        ];
        let mut result = Vec::new();
        for cat in attr_categories {
            for f in self.build.schemas().fields_for_category(Entity::Staff, cat) {
                if let Some(loc) = &f.offset {
                    let raw = self.raw[loc.offset() as usize];
                    let displayed = loc.decode_u8(raw);
                    result.push((f.label, displayed, raw));
                }
            }
        }
        result
    }

    /// Read personality as (label, value) pairs.
    pub fn personality(&self) -> Vec<(&'static str, u8)> {
        self.category(Category::Personality)
    }
}

/// Read a staff member's name from pre-fetched raw bytes.
///
/// ~72% of staff have a null `s_name` FmString; their names are in sub-record
/// candidate blocks at `s_name+0x10` (first name) and `s_name+0x18` (last name).
/// Witnesses: Frank Dauwen, Pascal De Vreese, Zvonimir Boban, René Henriksen,
///            Michael Madsen, Kern Lyhne, Danny Hesp, Mauro Tassotti, …
pub fn read_staff_name(fm: &FmProcess, raw: &[u8]) -> Option<String> {
    let ptr_at = |raw: &[u8], key: &str| -> u64 {
        let Some(off) = fm.schemas().offset_of(key) else { return 0 };
        if off + 8 > raw.len() { return 0; }
        u64::from_le_bytes(raw[off..off + 8].try_into().unwrap())
    };

    fn first_and_last(s: String) -> String {
        let parts: Vec<&str> = s.split_whitespace().collect();
        match parts.len() {
            0 => s,
            1 => parts[0].to_string(),
            _ => format!("{} {}", parts[0], parts[parts.len() - 1]),
        }
    }

    // FM shows the common name whenever the database sets one ("Luis Enrique").
    if let Some(c) = crate::player::read_first_name_from_sub(fm, ptr_at(raw, "s_name_common_sub")) {
        return Some(c);
    }

    // Otherwise prefer the first/last name sub-records: they hold the name FM
    // displays ("Kylian" + "Mbappé"). The full-name FmString ("Kylian Mbappé
    // Lottin") can't be split reliably, so it is only the fallback.
    let first = crate::player::read_first_name_from_sub(fm, ptr_at(raw, "s_name_first_sub"));
    let last  = crate::player::read_first_name_from_sub(fm, ptr_at(raw, "s_name_last_sub"));
    if let (Some(f), Some(l)) = (&first, &last) { return Some(format!("{f} {l}")); }

    let name_ptr = ptr_at(raw, "s_name");
    if let Ok(n) = fm.read_fm_name(name_ptr) { return Some(first_and_last(n)); }

    first.or(last)
}

/// Read the staff pointer array bounds.
pub fn staff_array(fm: &FmProcess) -> Result<(u64, u64, usize), String> {
    let schema = fm.schemas();
    fm.read_ptr_array(schema.rva_staff_data, schema.rva_staff_end)
}

/// Read a staff struct at a known address.
pub fn read_staff_at(fm: &FmProcess, addr: u64, index: usize) -> Result<StaffRecord, String> {
    let (raw, identity_shift) = read_staff_raw(fm, addr)?;
    Ok(StaffRecord { build: fm.build, addr, index, raw, identity_shift })
}

/// Identity shift of the staff object at `addr`: the schema's
/// `human_manager_shift` for an object of the human-manager class, else 0.
pub fn identity_shift(fm: &FmProcess, addr: u64) -> usize {
    let schema = fm.schemas();
    let rva = schema.human_manager_vtable_rva;
    if rva != 0 && fm.read_u64(addr).is_ok_and(|vt| vt == fm.gp_base + rva) {
        schema.human_manager_shift
    } else {
        0
    }
}

/// True for the staff entry FM creates for the player's platform account
/// (FM27 on Steam: "(<account name>)", no club, CA/PA 100/200). It is an
/// object of the human-manager class, like the real human manager, but it
/// is not a person in the game world: it has no date of birth (FM's empty
/// date, year 1900), no nationality and no contract. The real human manager
/// always has a date of birth, so the empty date is what tells them apart.
///
/// `identity_shift` is the record's shift (non-zero only for the
/// human-manager class); `dob_year` is the year of its schema `s_dob`.
pub fn is_account_placeholder(identity_shift: usize, dob_year: u16) -> bool {
    identity_shift != 0 && (dob_year == 0 || dob_year == 1900)
}

/// Read a staff struct in schema layout, plus its identity shift.
pub fn read_staff_raw(fm: &FmProcess, addr: u64) -> Result<(Vec<u8>, usize), String> {
    let shift = identity_shift(fm, addr);
    Ok((read_staff_raw_shifted(fm, addr, shift)?, shift))
}

/// Read a staff struct whose identity fields sit `shift` bytes later, copying
/// them back to their schema offsets so readers see one layout.
pub fn read_staff_raw_shifted(fm: &FmProcess, addr: u64, shift: usize) -> Result<Vec<u8>, String> {
    let schema = fm.schemas();
    let size = schema.staff_struct_size;
    let mut raw = fm.read_bytes(addr, size + shift)?;
    if shift != 0 {
        for &(off, len) in schema.manager_shifted_fields {
            raw.copy_within(off + shift..off + shift + len, off);
        }
        raw.truncate(size);
    }
    Ok(raw)
}

/// Find a staff record by database ID.
pub fn find_staff_by_id(
    fm: &FmProcess,
    staff_data: u64,
    count: usize,
    target_id: u32,
) -> Result<Option<StaffRecord>, String> {
    let id_field = fm.schemas().field("s_id").unwrap();
    let id_offset = id_field.offset.unwrap().offset();
    let id_bytes = target_id.to_le_bytes();

    for i in 0..count {
        let ptr = fm.read_u64(staff_data + i as u64 * 8)?;
        if ptr < 0x10000 || ptr > 0x7FFFFFFFFFFF {
            continue;
        }
        let chunk = fm.read_bytes(ptr + id_offset, 4)?;
        if chunk == id_bytes {
            return read_staff_at(fm, ptr, i).map(Some);
        }
    }
    Ok(None)
}

/// Write a staff attribute value (displayed 1-20 scale) by field key.
pub fn write_staff_field(
    fm: &FmProcess,
    staff_addr: u64,
    key: &str,
    displayed_value: u8,
) -> Result<(), String> {
    let f = fm.schemas().field(key).ok_or_else(|| format!("unknown field: {key}"))?;
    let loc = f.offset.ok_or_else(|| format!("field {key} has no mapped offset"))?;
    let raw = loc.encode_u8(displayed_value);
    fm.write_u8(staff_addr + loc.offset(), raw)
}
/// FM's job title for a staff contract job code (`s_job`). A player-coach
/// adds 1 to the coaching code (e.g. 35 = goalkeeping coach who also plays).
pub fn job_name(code: u8) -> Option<&'static str> {
    Some(match code {
        1 => "Player",
        2 => "Coach",
        4 => "Chairperson",
        6 => "Director",
        12 => "Physio",
        14 => "Scout",
        16 => "Manager",
        20 => "Assistant Coach",
        22 => "Set Piece Coach",
        26 => "Fitness Coach",
        34 => "Goalkeeping Coach",
        40 => "Head of Sports Science",
        44 => "Chief Scout",
        48 => "Sports Scientist",
        50 => "Head Physio",
        58 => "Recruitment Analyst",
        60 => "Performance Analyst",
        64 => "Head of Youth Development",
        86 => "Loan Manager",
        88 => "Technical Director",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_version::GameBuild;

    #[test]
    fn account_placeholder_is_a_human_manager_class_entry_without_birth_date() {
        // FM27 witnesses: the Steam profile entry (DOB 1 Jan 1900) and the
        // human manager (DOB in 1980), both with the 0x340 identity shift.
        assert!(is_account_placeholder(0x340, 1900));
        assert!(!is_account_placeholder(0x340, 1980));
        // Ordinary staff are never placeholders, whatever their date.
        assert!(!is_account_placeholder(0, 1900));
    }

    #[test]
    fn records_decode_with_their_own_build() {
        let versions = [GameBuild::Fm24Steam, GameBuild::Fm27Steam];
        let mut raw = vec![0; versions.iter().map(|v| v.schemas().staff_struct_size).max().unwrap()];
        let ca_offsets = versions.map(|v| v.schemas().offset_of("s_ca").unwrap());
        assert_ne!(ca_offsets[0], ca_offsets[1]);
        for (offset, value) in ca_offsets.into_iter().zip([123, 187]) {
            raw[offset] = value;
        }
        let fm24 = StaffRecord { build: GameBuild::Fm24Steam, addr: 0, index: 0, raw, identity_shift: 0 };
        let fm27 = StaffRecord { build: GameBuild::Fm27Steam, ..fm24.clone() };
        assert_eq!(fm24.ca(), 123);
        assert_eq!(fm27.ca(), 187);
        assert_eq!(fm24.ca(), 123);
    }
}
