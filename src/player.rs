//! Read player records from fm.exe via ptr_humans.

use crate::fields::{self, Category, Entity, HasSubRecords, SubRecordId};
use crate::process::FmProcess;

/// A decoded player record.
#[derive(Debug, Clone)]
pub struct PlayerRecord {
    pub build: crate::game_version::GameBuild,
    pub addr: u64,
    pub index: usize,
    pub name: Option<String>,
    /// Raw bytes for the full struct, in schema layout (see `read_player_raw`).
    pub raw: Vec<u8>,
    /// Bytes the person block is moved by in memory (player-coaches); edits
    /// to offsets at or past the schema's person offset must add it.
    pub person_shift: usize,
    /// Contract sub-record bytes, when the record's schema defines one.
    pub contract_raw: Option<Vec<u8>>,
    /// Address of the contract sub-record, for edits to contract fields.
    pub contract_addr: Option<u64>,
    /// Names resolved through sub-record chains (`p_current_club`,
    /// `p_nationality`, `p_birthplace`); `None` when unmapped or unresolved.
    pub club_name: Option<String>,
    /// Resolved club object; navigation requires it to match the cached club array.
    pub club_addr: Option<u64>,
    pub nationality: Option<String>,
    pub birthplace: Option<String>,
}

impl HasSubRecords for PlayerRecord {
    fn sub_record(&self, id: SubRecordId) -> Option<&[u8]> {
        match id {
            "contract" => self.contract_raw.as_deref(),
            _ => None,
        }
    }
}

impl PlayerRecord {
    /// Read a field by key, decoded to displayed value (u8).
    pub fn field_u8(&self, key: &str) -> Option<u8> {
        let f = self.build.schemas().field(key)?;
        fields::read_field_u8(self, &self.raw, f)
    }

    /// Read a field by key as u16.
    pub fn field_u16(&self, key: &str) -> Option<u16> {
        let f = self.build.schemas().field(key)?;
        fields::read_field_u16(self, &self.raw, f)
    }

    /// Read an integer field by key at its encoding's width
    /// (see `fields::read_field_int`).
    pub fn field_int(&self, key: &str) -> Option<u32> {
        let f = self.build.schemas().field(key)?;
        fields::read_field_int(self, &self.raw, f)
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

    /// Read all mapped fields for a category, returning (label, decoded_value).
    pub fn category(&self, category: Category) -> Vec<(&'static str, u8)> {
        self.build.schemas().fields_for_category(Entity::Player, category)
            .iter()
            .filter_map(|f| {
                let val = fields::read_field_u8(self, &self.raw, f)?;
                Some((f.label, val))
            })
            .collect()
    }

    /// Read all 15 positions as (label, value) pairs, skipping value=1.
    pub fn positions(&self) -> Vec<(&'static str, u8)> {
        self.build.schemas().fields_for_category(Entity::Player, Category::Positions)
            .iter()
            .filter_map(|f| {
                let val = fields::read_field_u8(self, &self.raw, f)?;
                Some((f.label, val))
            })
            .collect()
    }

    /// Read personality as (label, value) pairs.
    pub fn personality(&self) -> Vec<(&'static str, u8)> {
        self.category(Category::Personality)
    }

    /// Read the PPM/traits bitmask (u64) from the schema `p_traits` field.
    pub fn traits_mask(&self) -> u64 {
        self.field_u64("p_traits").unwrap_or(0)
    }

    /// Decode PPMs into a list of (ppm_id, name) for all set bits.
    pub fn traits(&self) -> Vec<(u8, &'static str)> {
        let mask = self.traits_mask();
        if mask == 0 {
            return Vec::new();
        }
        let mut result = Vec::new();
        for bit in 0..64u8 {
            if mask & (1u64 << bit) != 0 {
                let ppm_id = bit + 1;
                result.push((ppm_id, ppm_name(ppm_id)));
            }
        }
        result
    }
}

/// Map PPM ID (1-64) to display name.
pub fn ppm_name(id: u8) -> &'static str {
    match id {
        1 => "Runs With Ball Down Left",
        2 => "Runs With Ball Down Right",
        3 => "Runs With Ball Through Centre",
        4 => "Gets Into Opposition Area",
        5 => "Moves Into Channels",
        6 => "Gets Forward Whenever Possible",
        7 => "Plays Short Simple Passes",
        8 => "Tries Killer Balls Often",
        9 => "Shoots From Distance",
        10 => "Shoots With Power",
        11 => "Places Shots",
        12 => "Curls Ball",
        13 => "Likes To Round Keeper",
        14 => "Tries To Break Offside Trap",
        15 => "Uses Outside Of Foot",
        16 => "Marks Opponent Tightly",
        17 => "Winds Up Opponents",
        18 => "Argues With Officials",
        19 => "Plays With Back To Goal",
        20 => "Comes Deep To Get Ball",
        21 => "Plays One-Twos",
        22 => "Likes To Lob Keeper",
        23 => "Dictates Tempo",
        24 => "Attempts Overhead Kicks",
        25 => "Looks For Pass Over Score",
        26 => "Plays No Through Balls",
        27 => "Stops Play",
        28 => "Knocks Ball Past Opponent",
        29 => "Shifts Ball To Right Before Dribble",
        30 => "Shifts Ball To Left Before Dribble",
        31 => "Dwells On Ball",
        32 => "Late Runs Into Box",
        33 => "Plays Way Out Of Trouble",
        34 => "Stays Back At All Times",
        35 => "Avoids Using Weaker Foot",
        36 => "Tries Tricks",
        37 => "Tries Long Range Free Kicks",
        38 => "Dives Into Tackles",
        39 => "Does Not Dive Into Tackles",
        40 => "Cuts Inside From Both Wings",
        41 => "Hugs Line",
        42 => "Gets Crowd Going",
        43 => "Tries First Time Shots",
        44 => "Tries Long Range Passes",
        45 => "Likes Ball Played Into Feet",
        46 => "Hits Free Kicks With Power",
        47 => "Beats Opponent Repeatedly",
        48 => "Switches Ball To Wings",
        49 => "Will Retire At Top",
        50 => "Will Play Football As Long As Possible",
        51 => "Possesses Long Flat Throw",
        52 => "Runs With Ball Often",
        53 => "Runs With Ball Rarely",
        54 => "Attempts To Develop Weaker Foot",
        55 => "Penalty Box Player",
        56 => "Long Throw For Counters",
        57 => "Avoids Long Shots",
        58 => "Cuts Inside From Left Wing",
        59 => "Cuts Inside From Right Wing",
        60 => "Crosses Early",
        61 => "Brings Ball Out Of Defence",
        62 => "Demands To Take Free Kicks",
        63 => "Demands To Take Penalties",
        64 => "Plays Ball With Feet",
        _ => "Unknown PPM",
    }
}

/// Which FM26 traits-tab category a PPM belongs to.
/// Returns `None` for PPMs that don't appear in the traits UI
/// (career preferences, hidden/internal, unknown).
pub fn ppm_category(id: u8) -> Option<&'static str> {
    match id {
        14 | 17 | 18 | 19 | 25 | 42 | 45 => Some("General"),
        16 | 33 | 34 | 38 | 39 | 61 => Some("Defending"),
        12 | 15 | 23 | 27 | 31 | 35 => Some("Ball Control"),
        9 | 10 | 11 | 22 | 24 | 43 | 57 => Some("Shooting"),
        4 | 5 | 6 | 20 | 32 | 41 | 55 => Some("Movement"),
        37 | 46 | 51 | 56 | 62 | 63 => Some("Set Pieces"),
        1 | 2 | 3 | 13 | 28 | 29 | 30 | 36 | 40 | 47 | 52 | 53 | 58 | 59 => {
            Some("Dribbling / Running")
        }
        7 | 8 | 21 | 26 | 44 | 48 | 60 | 64 => Some("Passing"),
        _ => None,
    }
}

pub(crate) fn is_valid_ptr(p: u64) -> bool {
    (0x10000..0x7FFF_FFFF_FFFF).contains(&p)
}

/// Read the first (best) name entry from a name-component sub-record.
///
/// `sub_ptr` is the u64 pointer to the 5×24-byte candidate block.
/// Layout of each 24-byte entry: `[u64 ptr][u32 id][u16 nation][u16 type][u32 flags][u32 pad]`
/// where `ptr` points directly at `[u32 name_len][UTF-8 name_bytes]`.
pub(crate) fn read_first_name_from_sub(fm: &FmProcess, sub_ptr: u64) -> Option<String> {
    if !is_valid_ptr(sub_ptr) {
        return None;
    }
    let comp_ptr = fm.read_u64(sub_ptr).ok()?;
    if !is_valid_ptr(comp_ptr) {
        return None;
    }
    // Single RPM: 4-byte length prefix + up to 64 bytes of name data.
    let buf = fm.read_bytes(comp_ptr, 4 + 64).ok()?;
    let name_len = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
    if name_len == 0 || name_len > 64 {
        return None;
    }
    String::from_utf8(buf[4..4 + name_len].to_vec()).ok()
}

/// Read a player's name from pre-fetched raw bytes, the way FM displays it:
///   1. Common name sub-record (`p_name_common_sub`) when set — "Lamine Yamal"
///   2. Block 1: first/last sub-records, then the `p_name` full-name FmString
///   3. Block 2: `p_name2` FmString + sub-records (FM26 fallback; unmapped on FM24 and FM27)
/// Keys the process's schema leaves unmapped are skipped.
pub fn read_player_name(fm: &FmProcess, raw: &[u8]) -> Option<String> {
    let ptr_at = |raw: &[u8], key: &str| -> u64 {
        let Some(off) = fm.schemas().offset_of(key) else {
            return 0;
        };
        if off + 8 > raw.len() {
            return 0;
        }
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

    // FM shows the common name whenever the database sets one ("Cristiano
    // Ronaldo" rather than first "Cristiano Ronaldo" + last "Aveiro").
    if let Some(c) = read_first_name_from_sub(fm, ptr_at(raw, "p_name_common_sub")) {
        return Some(c);
    }

    // Within each block, prefer the first/last name sub-records: they hold
    // the name FM displays ("Kylian" + "Mbappé"). The full-name FmString
    // ("Kylian Mbappé Lottin") can't be split reliably into first and last,
    // so it is only the fallback.
    // Block 1
    let f1 = read_first_name_from_sub(fm, ptr_at(raw, "p_name_first_sub"));
    let l1 = read_first_name_from_sub(fm, ptr_at(raw, "p_name_last_sub"));
    if let (Some(f), Some(l)) = (&f1, &l1) {
        return Some(format!("{f} {l}"));
    }

    if let Ok(n) = fm.read_fm_name(ptr_at(raw, "p_name")) {
        return Some(first_and_last(n));
    }

    // Block 2
    let name2_ptr = ptr_at(raw, "p_name2");
    if is_valid_ptr(name2_ptr) {
        if let Ok(n) = fm.read_fm_name(name2_ptr) {
            return Some(first_and_last(n));
        }
    }

    let f2 = read_first_name_from_sub(fm, ptr_at(raw, "p_name2_first_sub")).or(f1);
    let l2 = read_first_name_from_sub(fm, ptr_at(raw, "p_name2_last_sub")).or(l1);
    match (f2, l2) {
        (Some(f), Some(l)) => Some(format!("{f} {l}")),
        (Some(f), None) => Some(f),
        (None, Some(l)) => Some(l),
        (None, None) => None,
    }
}

/// Read the player/humans pointer array bounds.
pub fn humans_array(fm: &FmProcess) -> Result<(u64, u64, usize), String> {
    let schema = fm.schemas();
    fm.read_ptr_array(schema.rva_humans_data, schema.rva_humans_end)
}

/// Bytes the person block of the player at `addr` is moved by: the schema's
/// `player_coach_shift` for a player-coach object, 0 otherwise.
pub fn person_shift(fm: &FmProcess, addr: u64) -> usize {
    let schema = fm.schemas();
    let rva = schema.player_coach_vtable_rva;
    if rva != 0 && fm.read_u64(addr).is_ok_and(|vt| vt == fm.gp_base + rva) {
        schema.player_coach_shift
    } else {
        0
    }
}

/// Physical offset of schema offset `off` in a player object whose person
/// block is moved by `shift` (see `person_shift`).
pub fn player_phys_offset(schemas: &'static fields::VersionedSchemas, off: u64, shift: usize) -> u64 {
    let person = schemas.player_person_offset as u64;
    if shift != 0 && off >= person { off + shift as u64 } else { off }
}

/// Read a player struct in schema layout, plus its person shift. For a
/// player-coach the moved person block is copied back to its schema
/// offsets, so every reader sees one layout; edits must map offsets back
/// with `player_phys_offset`.
pub fn read_player_raw(fm: &FmProcess, addr: u64) -> Result<(Vec<u8>, usize), String> {
    let schema = fm.schemas();
    let size = schema.player_struct_size;
    let shift = person_shift(fm, addr);
    let mut raw = fm.read_bytes(addr, size + shift)?;
    if shift != 0 {
        let person = schema.player_person_offset;
        raw.copy_within(person + shift.., person);
        raw.truncate(size);
    }
    Ok((raw, shift))
}

/// Read a player struct at a known address.
pub fn read_player_at(fm: &FmProcess, addr: u64, index: usize) -> Result<PlayerRecord, String> {
    if addr < 0x10000 || addr > 0x7FFFFFFFFFFF {
        return Err(format!("invalid player pointer: 0x{addr:X}"));
    }
    let (raw, person_shift) = read_player_raw(fm, addr)?;

    let name = read_player_name(fm, &raw);

    // Sub-record chains start from pointers in the player object; person
    // pointers (contract, nation, birth city) move with a player-coach.
    let subs = fm.schemas().player_sub_records;
    let phys = |off: u64| player_phys_offset(fm.schemas(), off, person_shift);
    let contract_addr = fields::resolve_sub_ptr_mapped(fm, addr, "contract", subs, &phys);
    let contract_raw = contract_addr.and_then(|ptr| {
        let size = subs.iter().find(|s| s.id == "contract")?.size;
        fm.read_bytes(ptr, size).ok()
    });

    let club_name = fields::read_sub_fm_string(fm, addr, "p_current_club", subs, &phys);
    let club_addr = fields::resolve_sub_ptr_mapped(fm, addr, "club", subs, &phys);
    let nationality = fields::read_sub_fm_string(fm, addr, "p_nationality", subs, &phys);
    let birthplace = fields::read_sub_fm_string(fm, addr, "p_birthplace", subs, &phys);

    Ok(PlayerRecord {
        build: fm.build,
        addr, index, name, raw, person_shift,
        contract_raw, contract_addr, club_name, club_addr, nationality, birthplace,
    })
}

/// Compute a player's age in whole years from the DOB year (u16 half of
/// the `p_dob` DateDayYear field). Returns `None` if the year is unset (0).
pub fn player_age(rec: &PlayerRecord) -> Option<u16> {
    let dob_year_offset = rec.build.schemas().field("p_dob")?.offset?.offset() as usize + 2;
    if rec.raw.len() < dob_year_offset + 2 {
        return None;
    }
    let year = u16::from_le_bytes(
        rec.raw[dob_year_offset..dob_year_offset + 2]
            .try_into()
            .unwrap(),
    );
    if year == 0 {
        return None;
    }
    Some(rec.build.schemas().age_reference_year.saturating_sub(year))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_version::GameBuild;

    #[test]
    fn records_decode_with_their_own_build() {
        let versions = [GameBuild::Fm24Steam, GameBuild::Fm27Steam];
        let mut raw = vec![0; versions.iter().map(|v| v.schemas().player_struct_size).max().unwrap()];
        let ca_offsets = versions.map(|v| v.schemas().offset_of("p_ca").unwrap());
        assert_ne!(ca_offsets[0], ca_offsets[1]);
        for (offset, value) in ca_offsets.into_iter().zip([123u16, 187]) {
            raw[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        for version in versions {
            let offset = version.schemas().offset_of("p_dob").unwrap() + 2;
            let year = version.schemas().age_reference_year - 25;
            raw[offset..offset + 2].copy_from_slice(&year.to_le_bytes());
        }
        let fm24 = PlayerRecord {
            build: GameBuild::Fm24Steam,
            addr: 0,
            index: 0,
            name: None,
            raw,
            person_shift: 0,
            contract_raw: None,
            contract_addr: None,
            club_name: None,
            club_addr: None,
            nationality: None,
            birthplace: None,
        };
        let fm27 = PlayerRecord { build: GameBuild::Fm27Steam, ..fm24.clone() };
        assert_eq!(fm24.field_u16("p_ca"), Some(123));
        assert_eq!(fm27.field_u16("p_ca"), Some(187));
        assert_eq!(player_age(&fm24), Some(25));
        assert_eq!(player_age(&fm27), Some(25));
        assert_eq!(fm24.field_u16("p_ca"), Some(123));
    }
}
