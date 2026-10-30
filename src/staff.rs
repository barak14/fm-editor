//! Read and write staff records from fm.exe via ptr_staff.

use crate::fields::{self, Category, Entity, HasSubRecords, SubRecordId};
use crate::process::FmProcess;

/// A decoded staff record.
#[derive(Debug, Clone)]
pub struct StaffRecord {
    pub addr: u64,
    pub index: usize,
    /// Raw bytes for the full struct
    pub raw: Vec<u8>,
}

impl HasSubRecords for StaffRecord {
    fn sub_record(&self, _id: SubRecordId) -> Option<&[u8]> {
        None
    }
}

impl StaffRecord {
    /// Read a field by key, decoded to displayed value (u8).
    pub fn field_u8(&self, key: &str) -> Option<u8> {
        let f = fields::field(key)?;
        fields::read_field_u8(self, &self.raw, &f)
    }

    /// Read a field by key as u32.
    pub fn field_u32(&self, key: &str) -> Option<u32> {
        let f = fields::field(key)?;
        fields::read_field_u32(self, &self.raw, &f)
    }

    /// Read a field by key as u64.
    pub fn field_u64(&self, key: &str) -> Option<u64> {
        let f = fields::field(key)?;
        fields::read_field_u64(self, &self.raw, &f)
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
        fields::fields_for_category(Entity::Staff, category)
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
            for f in fields::fields_for_category(Entity::Staff, cat) {
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
    fn ptr_at(raw: &[u8], key: &str) -> u64 {
        let Some(off) = fields::offset_of(key) else { return 0 };
        if off + 8 > raw.len() { return 0; }
        u64::from_le_bytes(raw[off..off + 8].try_into().unwrap())
    }

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
    let raw = fm.read_bytes(addr, fm.schemas().staff_struct_size)?;
    Ok(StaffRecord { addr, index, raw })
}

/// Find a staff record by database ID.
pub fn find_staff_by_id(
    fm: &FmProcess,
    staff_data: u64,
    count: usize,
    target_id: u32,
) -> Result<Option<StaffRecord>, String> {
    let id_field = fields::field("s_id").unwrap();
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
    let f = fields::field(key).ok_or_else(|| format!("unknown field: {key}"))?;
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
        16 => "Head Coach",
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
