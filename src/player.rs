//! Read player records from fm.exe via ptr_humans.

use crate::fields::{self, Category, Entity, HasSubRecords, SubRecordId};
use crate::process::FmProcess;

// All list reads go through these checked primitives, including nested pointers.
type MemoryRead<'a> = dyn Fn(u64, usize) -> Option<Vec<u8>> + 'a;

fn field_bytes<const N: usize>(bytes: &[u8], offset: usize) -> Option<[u8; N]> {
    bytes.get(offset..offset.checked_add(N)?)?.try_into().ok()
}

fn le_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(field_bytes(bytes, offset)?))
}

fn le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(field_bytes(bytes, offset)?))
}

fn le_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(field_bytes(bytes, offset)?))
}

fn range_addr(base: u64, offset: usize, size: usize) -> Option<u64> {
    if !is_valid_ptr(base) {
        return None;
    }
    let addr = base.checked_add(u64::try_from(offset).ok()?)?;
    let last = addr.checked_add(u64::try_from(size.checked_sub(1)?).ok()?)?;
    (is_valid_ptr(addr) && is_valid_ptr(last)).then_some(addr)
}

fn checked_read(read: &MemoryRead<'_>, addr: u64, size: usize) -> Option<Vec<u8>> {
    range_addr(addr, 0, size)?;
    let bytes = read(addr, size)?;
    (bytes.len() == size).then_some(bytes)
}

fn pointer_at(read: &MemoryRead<'_>, base: u64, offset: usize) -> Option<u64> {
    let addr = range_addr(base, offset, 8)?;
    let ptr = le_u64(&checked_read(read, addr, 8)?, 0)?;
    is_valid_ptr(ptr).then_some(ptr)
}

fn name_at(read: &MemoryRead<'_>, object: u64, offset: usize) -> Option<String> {
    let ptr = pointer_at(read, object, offset)?;
    let len = usize::try_from(le_u32(&checked_read(read, ptr, 4)?, 0)?).ok()?;
    if !(1..=256).contains(&len) {
        return None;
    }
    let addr = range_addr(ptr, 4, len)?;
    String::from_utf8(checked_read(read, addr, len)?).ok()
}

/// Validate a complete std::vector header before calculating or reading its span.
fn parse_vector(bytes: &[u8], stride: usize, max: usize) -> Option<(u64, usize)> {
    let begin = le_u64(bytes, 0)?;
    let end = le_u64(bytes, 8)?;
    let cap = le_u64(bytes, 16)?;
    if stride == 0 {
        return None;
    }
    if begin == 0 && end == 0 && (cap == 0 || is_valid_ptr(cap)) {
        return Some((0, 0));
    }
    if !is_valid_ptr(begin) || !is_valid_ptr(end) || !is_valid_ptr(cap) || begin > end || end > cap
    {
        return None;
    }
    let len = usize::try_from(end.checked_sub(begin)?).ok()?;
    if len % stride != 0 || len / stride > max {
        return None;
    }
    Some((begin, len / stride))
}

fn read_vector(
    read: &MemoryRead<'_>,
    object: u64,
    offset: usize,
    stride: usize,
    max: usize,
) -> Option<(u64, Vec<u8>)> {
    let header = range_addr(object, offset, 24)?;
    let (begin, count) = parse_vector(&checked_read(read, header, 24)?, stride, max)?;
    if count == 0 {
        return Some((begin, Vec::new()));
    }
    Some((
        begin,
        checked_read(read, begin, count.checked_mul(stride)?)?,
    ))
}

fn parse_injury(
    bytes: &[u8],
    addr: u64,
    layout: &fields::PlayerListLayout,
    name: String,
) -> Option<Injury> {
    range_addr(addr, 0, layout.injury_size)?;
    bytes.get(..layout.injury_size)?;
    let start = GameDate::from_raw(le_u32(bytes, layout.injury_start)?)?;
    let full_days = le_u16(bytes, layout.injury_full_days)?;
    let light_days = le_u16(bytes, layout.injury_light_days)?;
    if name.is_empty() || light_days > full_days || full_days > 3000 {
        return None;
    }
    // Healing writes both adjacent u16 fields in one acknowledged operation.
    if layout.injury_light_days != layout.injury_full_days.checked_add(2)? {
        return None;
    }
    let days_addr = range_addr(addr, layout.injury_full_days, 4)?;
    Some(Injury {
        addr,
        name,
        start,
        full_days,
        light_days,
        days_addr,
    })
}

fn parse_absence(bytes: &[u8], addr: u64, layout: &fields::PlayerListLayout) -> Option<Absence> {
    range_addr(addr, 0, layout.absence_size)?;
    bytes.get(..layout.absence_size)?;
    Some(Absence {
        addr,
        kind: *bytes.get(layout.absence_kind)?,
        start: GameDate::from_raw(le_u32(bytes, layout.absence_start)?)?,
        days: le_u16(bytes, layout.absence_days)?,
        days_addr: range_addr(addr, layout.absence_days, 2)?,
    })
}

fn parse_relation(
    bytes: &[u8],
    addr: u64,
    layout: &fields::PlayerListLayout,
    name: String,
) -> Option<Relation> {
    range_addr(addr, 0, layout.relation_size)?;
    bytes.get(..layout.relation_size)?;
    let target_addr = le_u64(bytes, layout.relation_target)?;
    if !is_valid_ptr(target_addr) || name.is_empty() {
        return None;
    }
    Some(Relation {
        addr,
        target_addr,
        name,
        kind: *bytes.get(layout.relation_kind)?,
        relation_type: *bytes.get(layout.relation_type)?,
        strength: *bytes.get(layout.relation_strength)?,
        reason: *bytes.get(layout.relation_reason)?,
    })
}

fn parse_clause(bytes: &[u8], addr: u64, layout: &fields::PlayerListLayout) -> Option<Clause> {
    range_addr(addr, 0, layout.clause_size)?;
    bytes.get(..layout.clause_size)?;
    Some(Clause {
        addr,
        amount_addr: range_addr(addr, layout.clause_amount, 4)?,
        amount: i32::from_le_bytes(field_bytes(bytes, layout.clause_amount)?),
        value: i16::from_le_bytes(field_bytes(bytes, layout.clause_value)?),
        clause_type: *bytes.get(layout.clause_type)?,
    })
}

fn list_state<T>(items: Option<Vec<T>>) -> ListState<T> {
    match items {
        Some(items) => ListState::Items(items),
        None => ListState::Unreadable,
    }
}

fn read_injuries(
    read: &MemoryRead<'_>,
    medical: Option<u64>,
    l: &fields::PlayerListLayout,
) -> Option<Vec<Injury>> {
    let (_, pointers) = read_vector(read, medical?, l.injuries_vec, 8, 16)?;
    pointers
        .chunks_exact(8)
        .map(|entry| {
            let addr = le_u64(entry, 0)?;
            let bytes = checked_read(read, addr, l.injury_size)?;
            let type_ptr = le_u64(&bytes, l.injury_type_ptr)?;
            let name = name_at(read, type_ptr, l.injury_type_name)?;
            parse_injury(&bytes, addr, l, name)
        })
        .collect()
}

fn read_absences(
    read: &MemoryRead<'_>,
    medical: Option<u64>,
    l: &fields::PlayerListLayout,
) -> Option<Vec<Absence>> {
    let (_, pointers) = read_vector(read, medical?, l.absences_vec, 8, 16)?;
    pointers
        .chunks_exact(8)
        .map(|entry| {
            let addr = le_u64(entry, 0)?;
            parse_absence(&checked_read(read, addr, l.absence_size)?, addr, l)
        })
        .collect()
}

fn read_relations(
    read: &MemoryRead<'_>,
    object: Option<u64>,
    l: &fields::PlayerListLayout,
) -> Option<Vec<Relation>> {
    let (base, bytes) = read_vector(read, object?, l.relations_vec, l.relation_size, 64)?;
    let mut items = Vec::new();
    for (i, entry) in bytes.chunks_exact(l.relation_size).enumerate() {
        let kind = *entry.get(l.relation_kind)?;
        let relation_type = *entry.get(l.relation_type)?;
        let name_offset = if kind == l.kind_club && relation_type == l.type_favoured_club {
            l.club_name
        } else if kind == l.kind_nation && relation_type == l.type_other_nationality {
            l.nation_name
        } else {
            continue;
        };
        let target = le_u64(entry, l.relation_target)?;
        let name = name_at(read, target, name_offset)?;
        let addr = range_addr(base, i.checked_mul(l.relation_size)?, l.relation_size)?;
        items.push(parse_relation(entry, addr, l, name)?);
    }
    Some(items)
}

fn read_clauses(
    read: &MemoryRead<'_>,
    contract: Option<u64>,
    l: &fields::PlayerListLayout,
) -> Option<Vec<Clause>> {
    let (base, bytes) = read_vector(read, contract?, l.clauses_vec, l.clause_size, 32)?;
    bytes
        .chunks_exact(l.clause_size)
        .enumerate()
        .map(|(i, entry)| {
            let addr = range_addr(base, i.checked_mul(l.clause_size)?, l.clause_size)?;
            parse_clause(entry, addr, l)
        })
        .collect()
}

type PlayerLists = (
    ListState<Injury>,
    ListState<Absence>,
    ListState<Relation>,
    ListState<Clause>,
);

fn read_player_lists(
    fm: &FmProcess,
    addr: u64,
    shift: usize,
    contract: Option<u64>,
) -> PlayerLists {
    let Some(l) = fm.schemas().player_lists else {
        return (
            ListState::Unmapped,
            ListState::Unmapped,
            ListState::Unmapped,
            ListState::Unmapped,
        );
    };
    let read = |a, n| fm.read_bytes(a, n).ok();
    let root_pointer = |off: u64| {
        if off >= fm.schemas().player_person_offset as u64 {
            off.checked_add(shift as u64)?;
        }
        let physical = player_phys_offset(fm.schemas(), off, shift);
        pointer_at(&read, addr, usize::try_from(physical).ok()?)
    };
    let medical = root_pointer(l.medical_ptr);
    let relations = root_pointer(l.relations_ptr);
    (
        list_state(read_injuries(&read, medical, l)),
        list_state(read_absences(&read, medical, l)),
        list_state(read_relations(&read, relations, l)),
        list_state(read_clauses(&read, contract, l)),
    )
}

/// A packed FM date. Keep the raw value: its day half also contains game flags.
#[derive(Clone, Copy, Debug)]
pub struct GameDate {
    pub raw: u32,
    pub year: u16,
    pub doy: u16,
}

impl GameDate {
    pub fn from_raw(raw: u32) -> Option<Self> {
        let year = (raw >> 16) as u16;
        let doy = (raw & 0x1FF) as u16;
        ((1900..=2200).contains(&year) && (1..=366).contains(&doy)).then_some(Self {
            raw,
            year,
            doy,
        })
    }

    fn ordinal(self) -> i64 {
        let y = i64::from(self.year) - 1;
        365 * y + y / 4 - y / 100 + y / 400 + i64::from(self.doy)
    }

    pub fn elapsed_since(self, start: Self) -> u16 {
        (self.ordinal() - start.ordinal()).clamp(0, i64::from(u16::MAX)) as u16
    }

    pub fn add_days(self, days: u16) -> Self {
        let mut year = self.year;
        let mut doy = u32::from(self.doy) + u32::from(days);
        while doy > u32::from(year_days(year)) && year < u16::MAX {
            doy -= u32::from(year_days(year));
            year += 1;
        }
        let doy = doy.min(u32::from(year_days(year))) as u16;
        Self {
            raw: (u32::from(year) << 16) | (self.raw & 0xFE00) | u32::from(doy),
            year,
            doy,
        }
    }

    pub fn display(self) -> String {
        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let lengths = [
            31,
            if year_days(self.year) == 366 { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        let mut day = self.doy;
        for (month, length) in months.iter().zip(lengths) {
            if day <= length {
                return format!("{day:02} {month} {}", self.year);
            }
            day -= length;
        }
        // FM accepts day 366 even for a non-leap year; normalize it explicitly.
        format!("01 Jan {}", u32::from(self.year) + 1)
    }
}

fn year_days(year: u16) -> u16 {
    if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
        366
    } else {
        365
    }
}

/// Unmapped and failed reads must not look like a confirmed empty list.
#[derive(Clone, Debug)]
pub enum ListState<T> {
    Unmapped,
    Unreadable,
    Items(Vec<T>),
}

#[derive(Clone, Debug)]
pub struct Injury {
    pub addr: u64,
    pub name: String,
    pub start: GameDate,
    pub full_days: u16,
    pub light_days: u16,
    pub days_addr: u64,
}

#[derive(Clone, Debug)]
pub struct Absence {
    pub addr: u64,
    pub kind: u8,
    pub start: GameDate,
    pub days: u16,
    pub days_addr: u64,
}

#[derive(Clone, Debug)]
pub struct Relation {
    pub addr: u64,
    pub target_addr: u64,
    pub name: String,
    pub kind: u8,
    pub relation_type: u8,
    pub strength: u8,
    pub reason: u8,
}

#[derive(Clone, Debug)]
pub struct Clause {
    pub addr: u64,
    pub amount_addr: u64,
    pub amount: i32,
    pub value: i16,
    pub clause_type: u8,
}
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
    pub game_date: Option<GameDate>,
    pub injuries: ListState<Injury>,
    pub absences: ListState<Absence>,
    pub relations: ListState<Relation>,
    pub clauses: ListState<Clause>,
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
        self.build
            .schemas()
            .fields_for_category(Entity::Player, category)
            .iter()
            .filter_map(|f| {
                let val = fields::read_field_u8(self, &self.raw, f)?;
                Some((f.label, val))
            })
            .collect()
    }

    /// Read all 15 positions as (label, value) pairs, skipping value=1.
    pub fn positions(&self) -> Vec<(&'static str, u8)> {
        self.build
            .schemas()
            .fields_for_category(Entity::Player, Category::Positions)
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
    p > 0x10000 && p < 0x7FFF_FFFF_FFFF
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
    let vtable =
        checked_read(&|a, n| fm.read_bytes(a, n).ok(), addr, 8).and_then(|bytes| le_u64(&bytes, 0));
    if rva != 0
        && fm
            .gp_base
            .checked_add(rva)
            .is_some_and(|expected| vtable == Some(expected))
    {
        schema.player_coach_shift
    } else {
        0
    }
}

/// Physical offset of schema offset `off` in a player object whose person
/// block is moved by `shift` (see `person_shift`).
pub fn player_phys_offset(
    schemas: &'static fields::VersionedSchemas,
    off: u64,
    shift: usize,
) -> u64 {
    let person = schemas.player_person_offset as u64;
    if shift != 0 && off >= person {
        off + shift as u64
    } else {
        off
    }
}

/// Read a player struct in schema layout, plus its person shift. For a
/// player-coach the moved person block is copied back to its schema
/// offsets, so every reader sees one layout; edits must map offsets back
/// with `player_phys_offset`.
pub fn read_player_raw(fm: &FmProcess, addr: u64) -> Result<(Vec<u8>, usize), String> {
    let schema = fm.schemas();
    let size = schema.player_struct_size;
    let shift = person_shift(fm, addr);
    let total = size.checked_add(shift).ok_or("player size overflow")?;
    let mut raw = checked_read(&|a, n| fm.read_bytes(a, n).ok(), addr, total)
        .ok_or("unreadable player record")?;
    if shift != 0 {
        let person = schema.player_person_offset;
        let source = person
            .checked_add(shift)
            .filter(|&start| start <= total && person <= size)
            .ok_or("invalid player person range")?;
        raw.copy_within(source.., person);
        raw.truncate(size);
    }
    Ok((raw, shift))
}

/// Read a player struct at a known address.
pub fn read_player_at(fm: &FmProcess, addr: u64, index: usize) -> Result<PlayerRecord, String> {
    if !is_valid_ptr(addr) {
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
        checked_read(&|a, n| fm.read_bytes(a, n).ok(), ptr, size)
    });

    let club_name = fields::read_sub_fm_string(fm, addr, "p_current_club", subs, &phys);
    let club_addr = fields::resolve_sub_ptr_mapped(fm, addr, "club", subs, &phys);
    let nationality = fields::read_sub_fm_string(fm, addr, "p_nationality", subs, &phys);
    let birthplace = fields::read_sub_fm_string(fm, addr, "p_birthplace", subs, &phys);
    let game_date = (fm.schemas().rva_current_date != 0)
        .then(|| fm.gp_base.checked_add(fm.schemas().rva_current_date))
        .flatten()
        .and_then(|ptr| checked_read(&|a, n| fm.read_bytes(a, n).ok(), ptr, 4))
        .and_then(|bytes| GameDate::from_raw(le_u32(&bytes, 0)?));
    let (injuries, absences, relations, clauses) =
        read_player_lists(fm, addr, person_shift, contract_addr);

    Ok(PlayerRecord {
        build: fm.build,
        addr,
        index,
        name,
        raw,
        person_shift,
        contract_raw,
        contract_addr,
        club_name,
        club_addr,
        nationality,
        birthplace,
        game_date,
        injuries,
        absences,
        relations,
        clauses,
    })
}

/// Compute age on the attached game's date, retaining the historical year-only
/// calculation when no current date is available.
pub fn player_age(rec: &PlayerRecord, today: Option<crate::game_date::GameDate>) -> Option<u16> {
    let offset = rec.build.schemas().field("p_dob")?.offset?.offset() as usize;
    let year_offset = offset.checked_add(2)?;
    let bytes = rec.raw.get(year_offset..year_offset.checked_add(2)?)?;
    let year = u16::from_le_bytes([bytes[0], bytes[1]]);
    if let Some(today) = today {
        let day_bytes = rec.raw.get(offset..year_offset)?;
        let day = u16::from_le_bytes([day_bytes[0], day_bytes[1]]);
        return crate::game_date::age_on((day, year), today).map(u16::from);
    }
    if year == 0 {
        return None;
    }
    Some(rec.build.schemas().age_reference_year.saturating_sub(year))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_version::GameBuild;
    fn layout() -> &'static fields::PlayerListLayout {
        GameBuild::Fm27Steam.schemas().player_lists.unwrap()
    }

    fn date(year: u16, doy: u16) -> GameDate {
        GameDate::from_raw((u32::from(year) << 16) | u32::from(doy)).unwrap()
    }

    fn vector(begin: u64, end: u64, cap: u64) -> Vec<u8> {
        [begin.to_le_bytes(), end.to_le_bytes(), cap.to_le_bytes()].concat()
    }

    fn fixture_read(
        memory: &std::collections::BTreeMap<u64, Vec<u8>>,
        addr: u64,
        size: usize,
    ) -> Option<Vec<u8>> {
        let (&base, bytes) = memory.range(..=addr).next_back()?;
        let offset = usize::try_from(addr.checked_sub(base)?).ok()?;
        Some(bytes.get(offset..offset.checked_add(size)?)?.to_vec())
    }

    fn fixture_name(
        memory: &mut std::collections::BTreeMap<u64, Vec<u8>>,
        object: u64,
        offset: usize,
        string: u64,
        name: &str,
    ) {
        let mut bytes = vec![0; offset + 8];
        bytes[offset..offset + 8].copy_from_slice(&string.to_le_bytes());
        memory.insert(object, bytes);
        memory.insert(
            string,
            [
                (name.len() as u32).to_le_bytes().to_vec(),
                name.as_bytes().to_vec(),
            ]
            .concat(),
        );
    }

    #[test]
    fn nested_injury_read_requires_readable_name_and_every_record_valid() {
        let l = layout();
        let mut memory = std::collections::BTreeMap::new();
        let medical = 0x20000;
        memory.insert(
            medical + l.injuries_vec as u64,
            vector(0x30000, 0x30008, 0x30010),
        );
        memory.insert(0x30000, 0x40000u64.to_le_bytes().to_vec());
        let mut injury = vec![0; l.injury_size];
        injury[l.injury_type_ptr..l.injury_type_ptr + 8].copy_from_slice(&0x50000u64.to_le_bytes());
        injury[l.injury_start..l.injury_start + 4]
            .copy_from_slice(&date(2025, 131).raw.to_le_bytes());
        injury[l.injury_full_days..l.injury_full_days + 2].copy_from_slice(&498u16.to_le_bytes());
        injury[l.injury_light_days..l.injury_light_days + 2].copy_from_slice(&468u16.to_le_bytes());
        memory.insert(0x40000, injury.clone());
        fixture_name(
            &mut memory,
            0x50000,
            l.injury_type_name,
            0x60000,
            "a patella dislocation",
        );
        let items = read_injuries(&|a, n| fixture_read(&memory, a, n), Some(medical), l).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "a patella dislocation");
        assert_eq!(date(2026, 194).elapsed_since(items[0].start), 428);
        assert_eq!(items[0].days_addr, 0x40000 + l.injury_full_days as u64);
        memory.remove(&0x60000);
        assert!(read_injuries(&|a, n| fixture_read(&memory, a, n), Some(medical), l).is_none());
        fixture_name(
            &mut memory,
            0x50000,
            l.injury_type_name,
            0x60000,
            "a patella dislocation",
        );
        memory.insert(
            medical + l.injuries_vec as u64,
            vector(0x30000, 0x30010, 0x30010),
        );
        memory.insert(
            0x30000,
            [0x40000u64.to_le_bytes(), 0x41000u64.to_le_bytes()].concat(),
        );
        injury[l.injury_light_days..l.injury_light_days + 2].copy_from_slice(&499u16.to_le_bytes());
        memory.insert(0x41000, injury);
        assert!(read_injuries(&|a, n| fixture_read(&memory, a, n), Some(medical), l).is_none());
    }

    #[test]
    fn relation_fixture_resolves_identity_and_filters_other_kinds() {
        let l = layout();
        let mut memory = std::collections::BTreeMap::new();
        let object = 0x20000;
        let base = 0x30000;
        let witnesses = [
            (l.kind_club, l.type_favoured_club, 0x4F, 0x40000u64, "Boca"),
            (l.kind_club, l.type_favoured_club, 0x4E, 0x41000, "Peñarol"),
            (l.kind_nation, l.type_other_nationality, 0, 0x42000, "Italy"),
            (
                l.kind_nation,
                l.type_other_nationality,
                0,
                0x43000,
                "Scotland",
            ),
            (
                l.kind_club,
                l.type_favoured_club,
                0x4F,
                0x44000,
                "NAC Breda",
            ),
            (l.kind_club, l.type_favoured_club, 0x4E, 0x45000, "Leeds"),
        ];
        let mut entries = Vec::new();
        for (i, (kind, relation_type, reason, target, name)) in witnesses.iter().enumerate() {
            let mut entry = vec![0; l.relation_size];
            entry[l.relation_target..l.relation_target + 8].copy_from_slice(&target.to_le_bytes());
            entry[l.relation_kind] = *kind;
            entry[l.relation_type] = *relation_type;
            entry[l.relation_strength] = 75;
            entry[l.relation_reason] = *reason;
            let name_offset = if *kind == l.kind_club {
                l.club_name
            } else {
                l.nation_name
            };
            fixture_name(
                &mut memory,
                *target,
                name_offset,
                0x60000 + i as u64 * 0x1000,
                name,
            );
            let item = parse_relation(
                &entry,
                base + i as u64 * l.relation_size as u64,
                l,
                (*name).into(),
            )
            .unwrap();
            assert_eq!(
                (
                    item.target_addr,
                    item.kind,
                    item.relation_type,
                    item.strength,
                    item.reason
                ),
                (*target, *kind, *relation_type, 75, *reason)
            );
            entries.extend(entry);
        }
        // Unsupported records are ignored without dereferencing their target.
        for (kind, relation_type) in [(7, 70), (l.kind_club, 72), (l.kind_nation, 70)] {
            let mut entry = vec![0; l.relation_size];
            entry[l.relation_kind] = kind;
            entry[l.relation_type] = relation_type;
            entries.extend(entry);
        }
        memory.insert(
            object + l.relations_vec as u64,
            vector(
                base,
                base + entries.len() as u64,
                base + entries.len() as u64,
            ),
        );
        memory.insert(base, entries);
        let items = read_relations(&|a, n| fixture_read(&memory, a, n), Some(object), l).unwrap();
        assert_eq!(items.len(), witnesses.len());
        for (i, (item, (_, _, _, target, name))) in items.iter().zip(witnesses).enumerate() {
            assert_eq!(
                (item.addr, item.target_addr, item.name.as_str()),
                (base + (i * l.relation_size) as u64, target, name)
            );
        }
        // A shown relation with an unreadable target must not become an empty list.
        memory.remove(&0x60000);
        assert!(read_relations(&|a, n| fixture_read(&memory, a, n), Some(object), l).is_none());
    }

    #[test]
    fn game_dates_preserve_flags_and_validate() {
        let current = GameDate::from_raw(0x07EA1AC2).unwrap();
        assert_eq!(
            (current.raw, current.year, current.doy),
            (0x07EA1AC2, 2026, 194)
        );
        assert_eq!(current.display(), "13 Jul 2026");
        for raw in [
            0,
            (1899 << 16) | 1,
            (2201 << 16) | 1,
            2026 << 16,
            (2026 << 16) | 367,
        ] {
            assert!(GameDate::from_raw(raw).is_none());
        }
        assert!(GameDate::from_raw((1900 << 16) | 1).is_some());
        assert!(GameDate::from_raw((2200 << 16) | 366).is_some());
    }

    #[test]
    fn elapsed_and_return_dates_include_428_day_witness() {
        let start = date(2025, 131); // Kulusevski: 11 May 2025.
        let now = date(2026, 194);
        assert_eq!(start.display(), "11 May 2025");
        assert_eq!(now.elapsed_since(start), 428);
        assert_eq!(start.add_days(428).display(), now.display());
        assert_eq!(now.elapsed_since(now), 0);
        assert_eq!(start.elapsed_since(now), 0);
        assert_eq!(date(2026, 1).elapsed_since(date(2025, 365)), 1);
        assert_eq!(date(2025, 1).elapsed_since(date(2024, 365)), 2);
        assert_eq!(date(2024, 59).add_days(1).display(), "29 Feb 2024");
        assert_eq!(date(2024, 59).add_days(2).display(), "01 Mar 2024");
        assert_eq!(date(2200, 1).elapsed_since(date(1900, 1)), u16::MAX);
        let flagged = GameDate::from_raw(0x07EA1AC2).unwrap();
        assert_eq!(flagged.add_days(0).raw, flagged.raw);
        // Healing/lifting uses elapsed days, not remaining duration.
        assert_eq!(now.elapsed_since(date(2026, 194)), 0); // van de Ven
        assert_eq!(now.elapsed_since(date(2024, 347)), 578); // Mudryk, 12 Dec 2024
    }

    #[test]
    fn vector_validation_rejects_bad_bounds_and_limits() {
        assert_eq!(parse_vector(&vector(0, 0, 0), 8, 16), Some((0, 0)));
        assert_eq!(
            parse_vector(&vector(0x20000, 0x20000, 0x20000), 8, 16),
            Some((0x20000, 0))
        );
        assert_eq!(
            parse_vector(&vector(0x20000, 0x20080, 0x20100), 8, 16),
            Some((0x20000, 16))
        );
        for (begin, end, cap) in [
            (0x10000, 0x10008, 0x10008),
            (0, 8, 8),
            (0x20000, 0, 0),
            (0x20008, 0x20000, 0x20010),
            (0x20000, 0x20010, 0x20008),
            (0x20000, 0x20009, 0x20010),
            (0x20000, 0x20088, 0x20088),
            (0x20000, 0x20008, 0x7FFF_FFFF_FFFF),
            (0, 0, 1),
        ] {
            assert!(parse_vector(&vector(begin, end, cap), 8, 16).is_none());
        }
        assert!(parse_vector(&[0; 23], 8, 16).is_none());
        assert!(parse_vector(&vector(0, 0, 0), 0, 16).is_none());
        assert!(parse_vector(&vector(0x20000, 0x20410, 0x20410), 16, 64).is_none());
        assert!(parse_vector(&vector(0x20000, 0x20108, 0x20108), 8, 32).is_none());
    }

    #[test]
    fn checked_reads_reject_pointers_overflow_and_short_buffers() {
        assert!(!is_valid_ptr(0x10000));
        assert!(is_valid_ptr(0x10001));
        assert!(!is_valid_ptr(0x7FFF_FFFF_FFFF));
        assert!(range_addr(0x20000, usize::MAX, 8).is_none());
        assert!(range_addr(0x7FFF_FFFF_FFFE, 0, 2).is_none());
        assert!(checked_read(&|_, _| Some(vec![0; 3]), 0x20000, 4).is_none());
        assert!(
            checked_read(&|_, _| panic!("invalid range must not be read"), 0x10000, 4).is_none()
        );
        assert!(checked_read(&|_, _| None, 0x20000, 4).is_none());
    }

    #[test]
    fn injury_parser_decodes_verified_witnesses_and_rejects_invalid_records() {
        let l = layout();
        for (name, year, doy, full, light) in [
            ("a ruptured quad", 2026, 4, 236u16, 206u16),
            ("a moderate knee ligament sprain", 2026, 194, 33, 19),
            ("a patella dislocation", 2025, 131, 498, 468),
            ("a torn ACL", 2026, 115, 260, 230),
            ("a torn ACL", 2026, 41, 270, 240),
        ] {
            let mut bytes = vec![0; l.injury_size];
            bytes[l.injury_start..l.injury_start + 4]
                .copy_from_slice(&date(year, doy).raw.to_le_bytes());
            bytes[l.injury_full_days..l.injury_full_days + 2].copy_from_slice(&full.to_le_bytes());
            bytes[l.injury_light_days..l.injury_light_days + 2]
                .copy_from_slice(&light.to_le_bytes());
            let item = parse_injury(&bytes, 0x30000, l, name.into()).unwrap();
            assert_eq!(
                (
                    item.name.as_str(),
                    item.start.year,
                    item.start.doy,
                    item.full_days,
                    item.light_days
                ),
                (name, year, doy, full, light)
            );
            assert_eq!(
                (item.addr, item.days_addr),
                (0x30000, 0x30000 + l.injury_full_days as u64)
            );
            assert!(parse_injury(&bytes, 0x30000, l, String::new()).is_none());
            assert!(parse_injury(&bytes[..l.injury_start + 3], 0x30000, l, name.into()).is_none());
            bytes[l.injury_light_days..l.injury_light_days + 2]
                .copy_from_slice(&(full + 1).to_le_bytes());
            assert!(parse_injury(&bytes, 0x30000, l, name.into()).is_none());
            bytes[l.injury_full_days..l.injury_full_days + 2]
                .copy_from_slice(&3001u16.to_le_bytes());
            assert!(parse_injury(&bytes, 0x30000, l, name.into()).is_none());
            bytes[l.injury_start..l.injury_start + 4].fill(0);
            assert!(parse_injury(&bytes, 0x30000, l, name.into()).is_none());
        }
    }

    #[test]
    fn absence_parser_preserves_bans_holidays_and_non_date_sentinel() {
        let l = layout();
        let mut bytes = vec![0; l.absence_size];
        bytes[l.absence_kind] = l.absence_ban;
        bytes[l.absence_start..l.absence_start + 4]
            .copy_from_slice(&date(2024, 347).raw.to_le_bytes());
        bytes[l.absence_days..l.absence_days + 2].copy_from_slice(&595u16.to_le_bytes());
        let ban = parse_absence(&bytes, 0x30000, l).unwrap();
        assert_eq!(
            (ban.kind, ban.days, ban.start.display()),
            (0x19, 595, "12 Dec 2024".into())
        );
        assert_eq!(ban.days_addr, ban.addr + l.absence_days as u64);
        bytes[l.absence_kind] = l.absence_holiday;
        bytes[l.absence_days..l.absence_days + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        let holiday = parse_absence(&bytes, 0x30000, l).unwrap();
        assert_eq!((holiday.kind, holiday.days), (0x1F, u16::MAX));
        bytes[l.absence_kind] = 0xAA;
        assert_eq!(parse_absence(&bytes, 0x30000, l).unwrap().kind, 0xAA);
        bytes[l.absence_start..l.absence_start + 4].fill(0);
        assert!(parse_absence(&bytes, 0x30000, l).is_none());
    }

    #[test]
    fn clause_parser_decodes_verified_bonuses_percent_years_and_unknowns() {
        let l = layout();
        for (kind, amount, value) in [
            (0x20, 35000i32, -1i16),
            (0x21, 50000, -1),
            (0x27, 15000, -1),
            (0x26, 5000, -1),
            (0x20, 40000, -1),
            (0x21, 55000, -1),
            (0x22, 30000, -1),
            (0x26, 6000, -1),
            (0x05, -1, 50),
            (0x16, -1, 1),
            (0xFE, -1, -1),
        ] {
            let mut bytes = vec![0; l.clause_size];
            bytes[l.clause_amount..l.clause_amount + 4].copy_from_slice(&amount.to_le_bytes());
            bytes[l.clause_value..l.clause_value + 2].copy_from_slice(&value.to_le_bytes());
            bytes[l.clause_type] = kind;
            let item = parse_clause(&bytes, 0x30000, l).unwrap();
            assert_eq!(
                (item.addr, item.amount_addr),
                (0x30000, 0x30000 + l.clause_amount as u64)
            );
            assert_eq!(
                (item.clause_type, item.amount, item.value),
                (kind, amount, value)
            );
            assert!(parse_clause(&bytes[..l.clause_type], 0x30000, l).is_none());
        }
    }

    #[test]
    fn list_reads_distinguish_missing_empty_and_failed_data() {
        let l = layout();
        let missing = |_, _| None;
        assert!(matches!(
            list_state(read_injuries(&missing, None, l)),
            ListState::Unreadable
        ));
        assert!(matches!(
            list_state(read_absences(&missing, Some(0x20000), l)),
            ListState::Unreadable
        ));
        assert!(matches!(
            list_state(read_relations(&missing, None, l)),
            ListState::Unreadable
        ));
        assert!(matches!(
            list_state(read_clauses(&missing, None, l)),
            ListState::Unreadable
        ));
        let empty = |_, size| Some(vec![0; size]);
        assert!(
            matches!(list_state(read_injuries(&empty, Some(0x20000), l)), ListState::Items(v) if v.is_empty())
        );
        assert!(
            matches!(list_state(read_absences(&empty, Some(0x20000), l)), ListState::Items(v) if v.is_empty())
        );
        assert!(
            matches!(list_state(read_relations(&empty, Some(0x20000), l)), ListState::Items(v) if v.is_empty())
        );
        assert!(
            matches!(list_state(read_clauses(&empty, Some(0x20000), l)), ListState::Items(v) if v.is_empty())
        );
        let bad_item = |addr, size| match (addr, size) {
            (0x20000, 24) => Some(vector(0x30000, 0x30008, 0x30008)),
            (0x30000, 8) => Some(0x10000u64.to_le_bytes().to_vec()),
            _ => None,
        };
        assert!(matches!(
            list_state(read_injuries(&bad_item, Some(0x20000), l)),
            ListState::Unreadable
        ));
        for build in [GameBuild::Fm24Steam, GameBuild::Fm26Steam] {
            assert!(build.schemas().player_lists.is_none());
            assert_eq!(build.schemas().rva_current_date, 0);
        }
    }

    #[test]
    fn records_decode_with_their_own_build() {
        let versions = [GameBuild::Fm24Steam, GameBuild::Fm27Steam];
        let mut raw = vec![
            0;
            versions
                .iter()
                .map(|v| v.schemas().player_struct_size)
                .max()
                .unwrap()
        ];
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
            game_date: None,
            injuries: ListState::Unmapped,
            absences: ListState::Unmapped,
            relations: ListState::Unmapped,
            clauses: ListState::Unmapped,
        };
        let fm27 = PlayerRecord {
            build: GameBuild::Fm27Steam,
            ..fm24.clone()
        };

        assert_eq!(fm24.field_u16("p_ca"), Some(123));
        assert_eq!(fm27.field_u16("p_ca"), Some(187));
        assert_eq!(player_age(&fm24, None), Some(25));
        assert_eq!(player_age(&fm27, None), Some(25));
        assert_eq!(fm24.field_u16("p_ca"), Some(123));
        let mut fm27 = fm27;
        let dob_offset = fm27.build.schemas().offset_of("p_dob").unwrap();
        // 8 July 1991, including the flag bits carried by live DOB words.
        fm27.raw[dob_offset..dob_offset + 2].copy_from_slice(&(189u16 | 0x8000).to_le_bytes());
        fm27.raw[dob_offset + 2..dob_offset + 4].copy_from_slice(&1991u16.to_le_bytes());
        assert_eq!(player_age(&fm27, None), Some(36));
        for (day, expected) in [(188, 34), (189, 35), (195, 35)] {
            assert_eq!(
                player_age(&fm27, Some(crate::game_date::GameDate { day, year: 2026 })),
                Some(expected)
            );
        }
        fm27.raw[dob_offset..dob_offset + 2].fill(0);
        assert_eq!(
            player_age(
                &fm27,
                Some(crate::game_date::GameDate {
                    day: 195,
                    year: 2026
                })
            ),
            None
        );
        assert_eq!(player_age(&fm27, None), Some(36));
    }
}
