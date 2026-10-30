//! Field definitions — the single source of truth for every known field on
//! every entity type.
//!
//! Per-entity field vectors live in `player.rs`, `staff.rs`, and `club.rs`.
//! This module owns the shared types (`Field`, `Entity`, `Category`,
//! `FieldLocation`, `Encoding`) and exposes the lookup helpers the rest of
//! the codebase uses.
//!
//! When adding or reverse-engineering a field, touch only the sub-module
//! for that entity — there should be no reason to edit more than one of
//! `player.rs` / `staff.rs` / `club.rs` in a given session.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

pub mod fm26;
pub mod fm24;
pub mod fm27;

/// A field definition: key, display label, entity type, category, and optional offset.
#[derive(Debug, Clone)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub entity: Entity,
    pub category: Category,
    pub offset: Option<FieldLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Entity {
    Player,
    Staff,
    Club,
    /// Stadium entity — flat inline array (stride 0x170), NOT a pointer array.
    /// Access chain from GP: GP+RVA → base → base+0x80 → type → vec → data → data+0x40 + i*stride.
    /// Not yet wired to UI or entity loading.
    Stadium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Ability,
    BoardRelations,
    Coaching,
    Contract,
    Condition,
    Facilities,
    Finance,
    Footedness,
    General,
    Goalkeeping,
    Hidden,
    /// Schema-only helper fields (name sub-record pointers, etc.) — never rendered in any UI panel.
    Internal,
    Knowledge,
    Medical,
    Mental,
    Personality,
    Physical,
    Positions,
    Reputation,
    SetPieces,
    /// Role ratings stored in the staff-role sub-record (15 × u8 at role_ptr+0).
    /// Rendered via `roles_column` — not editable through the standard widget path.
    StaffRole,
    TacticalStyle,
    Technical,
    Traits,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoardRelations => write!(f, "Board Relations"),
            Self::SetPieces => write!(f, "Set Pieces"),
            Self::StaffRole => write!(f, "Staff Roles"),
            Self::TacticalStyle => write!(f, "Tactical Style"),
            other => write!(f, "{other:?}"),
        }
    }
}

/// Stable string identifier for a sub-record within an entity.
/// Examples for club: "info", "finance", "budget", "stadium".
pub type SubRecordId = &'static str;

/// Where a field lives: either flat in the entity's top-level bytes,
/// or inside one of the entity's named sub-records.
#[derive(Debug, Clone, Copy)]
pub enum FieldLocation {
    /// Field lives at `offset` inside the entity's top-level raw buffer.
    Direct {
        offset: u64,
        encoding: Encoding,
    },
    /// Field lives at `offset` inside a named sub-record buffer.
    SubRecord {
        container: SubRecordId,
        offset: u64,
        encoding: Encoding,
    },
}

impl FieldLocation {
    /// Byte offset within whichever buffer holds this field.
    pub fn offset(&self) -> u64 {
        match self {
            Self::Direct { offset, .. } | Self::SubRecord { offset, .. } => *offset,
        }
    }

    /// Encoding of this field's raw value.
    pub fn encoding(&self) -> Encoding {
        match self {
            Self::Direct { encoding, .. } | Self::SubRecord { encoding, .. } => *encoding,
        }
    }

    /// Decode a raw byte to displayed value.
    pub fn decode_u8(&self, raw: u8) -> u8 {
        match self.encoding() {
            Encoding::Direct | Encoding::Raw => raw,
            Encoding::Encoded => (raw as u16 + 2) as u8 / 5,
            _ => raw,
        }
    }

    /// Encode a displayed value to raw byte for writing.
    pub fn encode_u8(&self, displayed: u8) -> u8 {
        match self.encoding() {
            Encoding::Direct | Encoding::Raw => displayed,
            Encoding::Encoded => displayed * 5 - 2,
            _ => displayed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Encoding {
    Direct,      // stored = displayed (u8 1-20, or raw value)
    Encoded,     // displayed = (raw + 2) / 5
    Raw,            // u16 little-endian, no transformation (reputation, CA, PA)
    RawCapped(u16), // u16 little-endian with an explicit max (e.g. CA/PA → 200, rep → 10000)
    RawU32,         // u32 little-endian, no transformation (wages, budgets)
    DateDayYear, // [u16 day_of_year][u16 year]
    Pointer,     // u64 pointer to sub-object
    FmString,    // u64 pointer to [u32 len][UTF-8]
    F32Raw,      // f32 little-endian
    RoleIndex,   // index into [u8; 15] staff-role sub-record (offset field = byte index)
}

/// A sub-record — a block of data reached via pointer chase from the
/// entity base, optionally nested inside another sub-record.
pub struct SubRecord {
    pub id: SubRecordId,
    pub parent: SubRecordParent,
    /// Byte offset inside the parent where the 8-byte pointer lives.
    pub ptr_offset: u64,
    /// Number of bytes to read from the sub-record buffer.
    pub size: usize,
}

/// Where a sub-record's pointer lives.
pub enum SubRecordParent {
    /// Pointer lives at `entity_base + ptr_offset`.
    Direct,
    /// Pointer lives at another named sub-record's buffer + ptr_offset.
    In(SubRecordId),
}

/// Dispatch seam for records that may expose named sub-record buffers.
/// Player and staff records implement this returning `None` for every id.
/// `ClubView` implements it returning the appropriate `Option<&[u8]>` per id.
pub trait HasSubRecords {
    fn sub_record(&self, id: SubRecordId) -> Option<&[u8]>;
}

/// No-sub-records sentinel — useful for calling `read_field_*` when
/// you only have a raw buffer and no entity view (e.g., mid-construction).
impl HasSubRecords for () {
    fn sub_record(&self, _id: SubRecordId) -> Option<&[u8]> {
        None
    }
}

// ============================================================
// Shorthand constructors — used inside const field tables.
// All produce FieldLocation::Direct variants; sub-record
// variants use the sub_* constructors below.
// ============================================================

pub(crate) const fn direct(offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::Direct { offset, encoding: Encoding::Direct })
}
pub(crate) const fn encoded(offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::Direct { offset, encoding: Encoding::Encoded })
}
pub(crate) const fn raw(offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::Direct { offset, encoding: Encoding::Raw })
}
pub(crate) const fn raw_capped(offset: u64, max: u16) -> Option<FieldLocation> {
    Some(FieldLocation::Direct { offset, encoding: Encoding::RawCapped(max) })
}
pub(crate) const fn raw_u32(offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::Direct { offset, encoding: Encoding::RawU32 })
}
pub(crate) const fn date(offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::Direct { offset, encoding: Encoding::DateDayYear })
}
pub(crate) const fn pointer(offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::Direct { offset, encoding: Encoding::Pointer })
}
pub(crate) const fn fm_string(offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::Direct { offset, encoding: Encoding::FmString })
}

/// Index into the 15-byte staff-role sub-record buffer.
/// The `offset` field on the produced `FieldLocation` carries the byte index (0–14),
/// not a memory offset into `record.raw`. Rendered by `roles_column` only.
pub(crate) const fn role_index(idx: u8) -> Option<FieldLocation> {
    Some(FieldLocation::Direct { offset: idx as u64, encoding: Encoding::RoleIndex })
}

// Sub-record shorthand constructors:
pub(crate) const fn sub_direct(container: SubRecordId, offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::SubRecord { container, offset, encoding: Encoding::Direct })
}
pub(crate) const fn sub_raw(container: SubRecordId, offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::SubRecord { container, offset, encoding: Encoding::Raw })
}
pub(crate) const fn sub_raw_u32(container: SubRecordId, offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::SubRecord { container, offset, encoding: Encoding::RawU32 })
}
pub(crate) const fn sub_date(container: SubRecordId, offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::SubRecord { container, offset, encoding: Encoding::DateDayYear })
}
pub(crate) const fn sub_f32(container: SubRecordId, offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::SubRecord { container, offset, encoding: Encoding::F32Raw })
}
pub(crate) const fn sub_fm_string(container: SubRecordId, offset: u64) -> Option<FieldLocation> {
    Some(FieldLocation::SubRecord { container, offset, encoding: Encoding::FmString })
}

/// Game version whose schema backs the global lookups below. Set by
/// `FmProcess::attach` before any record is read; defaults to FM26 so
/// offline tools (no attached process) keep their previous behaviour.
static ACTIVE_VERSION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Select the schema used by `all_fields` / `field` / `fields_for*`.
pub fn set_active_version(version: crate::game_version::GameVersion) {
    let v = match version {
        crate::game_version::GameVersion::Fm26 => 0,
        crate::game_version::GameVersion::Fm24 => 1,
        crate::game_version::GameVersion::Fm27 => 2,
    };
    ACTIVE_VERSION.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// Year ages are computed against for the active game version.
pub fn age_reference_year() -> u16 {
    active_schema().age_reference_year
}

/// Schema for the game version the editor is currently attached to.
pub fn active_schema() -> &'static VersionedSchemas {
    match ACTIVE_VERSION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => &fm24::FM24,
        2 => &fm27::FM27,
        _ => &fm26::FM26,
    }
}

/// All known fields across all entity types for the active game version.
/// Concatenation of that version's per-entity const slices — add new fields
/// to the correct `fm26`/`fm24`/`fm27` sub-module, never here.
pub fn all_fields() -> Vec<Field> {
    let s = active_schema();
    let mut out = Vec::with_capacity(
        s.player_fields.len() + s.staff_fields.len() + s.club_fields.len() + s.stadium_fields.len(),
    );
    out.extend_from_slice(s.player_fields);
    out.extend_from_slice(s.staff_fields);
    out.extend_from_slice(s.club_fields);
    out.extend_from_slice(s.stadium_fields);
    out
}

// =========================================================================
// Lookup helpers — so reader code uses this module as the single source
// =========================================================================

/// Get all mapped fields for an entity type.
pub fn fields_for(entity: Entity) -> Vec<Field> {
    all_fields().into_iter().filter(|f| f.entity == entity).collect()
}

/// Get all mapped fields for an entity + category.
pub fn fields_for_category(entity: Entity, category: Category) -> Vec<Field> {
    all_fields()
        .into_iter()
        .filter(|f| f.entity == entity && f.category == category && f.offset.is_some())
        .collect()
}

fn key_map(s: &'static VersionedSchemas) -> HashMap<&'static str, Field> {
    s.player_fields
        .iter()
        .chain(s.staff_fields)
        .chain(s.club_fields)
        .chain(s.stadium_fields)
        .map(|f| (f.key, f.clone()))
        .collect()
}

static FM26_KEYS: LazyLock<HashMap<&'static str, Field>> = LazyLock::new(|| key_map(&fm26::FM26));
static FM24_KEYS: LazyLock<HashMap<&'static str, Field>> = LazyLock::new(|| key_map(&fm24::FM24));
static FM27_KEYS: LazyLock<HashMap<&'static str, Field>> = LazyLock::new(|| key_map(&fm27::FM27));

/// Look up a single field by key in the active version's schema.
///
/// Backed by a lazily-built `HashMap` per version so repeated lookups (e.g.
/// inside hot loops) are O(1) instead of scanning the full field list.
pub fn field(key: &str) -> Option<Field> {
    let map = match ACTIVE_VERSION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => &*FM24_KEYS,
        2 => &*FM27_KEYS,
        _ => &*FM26_KEYS,
    };
    map.get(key).cloned()
}

/// Byte offset of a field in the active schema, or `None` when the key is
/// unknown or unmapped for this version.
pub fn offset_of(key: &str) -> Option<usize> {
    Some(field(key)?.offset?.offset() as usize)
}

fn decode_u8_enc(byte: u8, encoding: Encoding) -> u8 {
    match encoding {
        Encoding::Direct | Encoding::Raw => byte,
        Encoding::Encoded => (byte as u16 + 2) as u8 / 5,
        _ => byte,
    }
}

/// Resolve the in-process address of a named sub-record from the entity base.
///
/// Recursively chases `SubRecordParent::In(pid)` chains (finance → budget,
/// team → club) so callers never special-case a hop.
pub fn resolve_sub_ptr(
    fm: &crate::process::FmProcess,
    base_ptr: u64,
    target_id: SubRecordId,
    sub_records: &[SubRecord],
) -> Option<u64> {
    resolve_sub_ptr_mapped(fm, base_ptr, target_id, sub_records, &|o| o)
}

/// `resolve_sub_ptr` for an entity whose physical layout differs from the
/// schema's: `phys` maps a schema offset in the entity itself to its real
/// offset (player-coaches). Offsets inside sub-records are not mapped.
pub fn resolve_sub_ptr_mapped(
    fm: &crate::process::FmProcess,
    base_ptr: u64,
    target_id: SubRecordId,
    sub_records: &[SubRecord],
    phys: &dyn Fn(u64) -> u64,
) -> Option<u64> {
    let sr = sub_records.iter().find(|s| s.id == target_id)?;
    let ptr_addr = match &sr.parent {
        SubRecordParent::Direct => base_ptr + phys(sr.ptr_offset),
        SubRecordParent::In(pid) => {
            resolve_sub_ptr_mapped(fm, base_ptr, pid, sub_records, phys)? + sr.ptr_offset
        }
    };
    let ptr = fm.read_u64(ptr_addr).ok()?;
    (ptr > 0x10000 && ptr < 0x7FFFFFFFFFFF).then_some(ptr)
}

/// Read an FmString field that lives in a sub-record (e.g. a player's club
/// name via team → club). `None` when the active schema leaves the key
/// unmapped, it is not a sub-record FmString, or any hop is invalid.
pub fn read_sub_fm_string(
    fm: &crate::process::FmProcess,
    base_ptr: u64,
    key: &str,
    sub_records: &[SubRecord],
    phys: &dyn Fn(u64) -> u64,
) -> Option<String> {
    let FieldLocation::SubRecord { container, offset, encoding: Encoding::FmString } = field(key)?.offset? else {
        return None;
    };
    let sub_ptr = resolve_sub_ptr_mapped(fm, base_ptr, container, sub_records, phys)?;
    let name_ptr = fm.read_u64(sub_ptr + offset).ok()?;
    fm.read_fm_name(name_ptr).ok()
}

/// Read a u8 field, dispatching on Direct vs SubRecord.
pub fn read_field_u8<V: HasSubRecords>(view: &V, raw: &[u8], f: &Field) -> Option<u8> {
    match f.offset.as_ref()? {
        FieldLocation::Direct { offset, encoding } => {
            Some(decode_u8_enc(*raw.get(*offset as usize)?, *encoding))
        }
        FieldLocation::SubRecord { container, offset, encoding } => {
            let sub = view.sub_record(container)?;
            Some(decode_u8_enc(*sub.get(*offset as usize)?, *encoding))
        }
    }
}

/// Read a u16 field.
pub fn read_field_u16<V: HasSubRecords>(view: &V, raw: &[u8], f: &Field) -> Option<u16> {
    match f.offset.as_ref()? {
        FieldLocation::Direct { offset, .. } => {
            let off = *offset as usize;
            if off + 2 > raw.len() { return None; }
            Some(u16::from_le_bytes(raw[off..off + 2].try_into().unwrap()))
        }
        FieldLocation::SubRecord { container, offset, .. } => {
            let sub = view.sub_record(container)?;
            let off = *offset as usize;
            if off + 2 > sub.len() { return None; }
            Some(u16::from_le_bytes(sub[off..off + 2].try_into().unwrap()))
        }
    }
}

/// Read a u32 field.
pub fn read_field_u32<V: HasSubRecords>(view: &V, raw: &[u8], f: &Field) -> Option<u32> {
    match f.offset.as_ref()? {
        FieldLocation::Direct { offset, .. } => {
            let off = *offset as usize;
            if off + 4 > raw.len() { return None; }
            Some(u32::from_le_bytes(raw[off..off + 4].try_into().unwrap()))
        }
        FieldLocation::SubRecord { container, offset, .. } => {
            let sub = view.sub_record(container)?;
            let off = *offset as usize;
            if off + 4 > sub.len() { return None; }
            Some(u32::from_le_bytes(sub[off..off + 4].try_into().unwrap()))
        }
    }
}

/// Read a u64 field.
pub fn read_field_u64<V: HasSubRecords>(view: &V, raw: &[u8], f: &Field) -> Option<u64> {
    match f.offset.as_ref()? {
        FieldLocation::Direct { offset, .. } => {
            let off = *offset as usize;
            if off + 8 > raw.len() { return None; }
            Some(u64::from_le_bytes(raw[off..off + 8].try_into().unwrap()))
        }
        FieldLocation::SubRecord { container, offset, .. } => {
            let sub = view.sub_record(container)?;
            let off = *offset as usize;
            if off + 8 > sub.len() { return None; }
            Some(u64::from_le_bytes(sub[off..off + 8].try_into().unwrap()))
        }
    }
}

/// Read a f32 field (F32Raw encoding).
pub fn read_field_f32<V: HasSubRecords>(view: &V, raw: &[u8], f: &Field) -> Option<f32> {
    match f.offset.as_ref()? {
        FieldLocation::Direct { offset, .. } => {
            let off = *offset as usize;
            if off + 4 > raw.len() { return None; }
            Some(f32::from_le_bytes(raw[off..off + 4].try_into().unwrap()))
        }
        FieldLocation::SubRecord { container, offset, .. } => {
            let sub = view.sub_record(container)?;
            let off = *offset as usize;
            if off + 4 > sub.len() { return None; }
            Some(f32::from_le_bytes(sub[off..off + 4].try_into().unwrap()))
        }
    }
}

/// Print a coverage report.
pub fn print_coverage() {
    let fields = all_fields();

    for entity in [Entity::Player, Entity::Staff, Entity::Club, Entity::Stadium] {
        let entity_fields: Vec<_> = fields.iter().filter(|f| f.entity == entity).collect();
        let mapped = entity_fields.iter().filter(|f| f.offset.is_some()).count();
        let total = entity_fields.len();

        println!("\n{:?}: {mapped}/{total} fields mapped", entity);
        println!("{:-<60}", "");

        let mut categories: Vec<Category> = entity_fields.iter().map(|f| f.category).collect();
        categories.sort();
        categories.dedup();

        for cat in categories {
            let cat_fields: Vec<_> = entity_fields.iter().filter(|f| f.category == cat).collect();
            let cat_mapped = cat_fields.iter().filter(|f| f.offset.is_some()).count();
            print!("  {cat:<20} {cat_mapped}/{:<3}", cat_fields.len());

            let missing: Vec<_> = cat_fields.iter().filter(|f| f.offset.is_none()).collect();
            if !missing.is_empty() {
                print!(
                    "  missing: {}",
                    missing
                        .iter()
                        .map(|f| f.label)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            println!();
        }
    }
}

// =========================================================================
// VersionedSchemas — one const per supported game version
// =========================================================================

/// Complete field / layout / RVA schema for one game version.
///
/// There is exactly one `const` of this type per supported version
/// (`fm26::FM26`, `fm24::FM24`). `FmProcess::schemas()` returns the
/// schema that matches the game currently attached to.
pub struct VersionedSchemas {
    // Field lists per entity.
    pub player_fields: &'static [Field],
    pub staff_fields: &'static [Field],
    pub club_fields: &'static [Field],
    pub stadium_fields: &'static [Field],

    // Sub-record tables per entity. Empty slices for player/staff (no
    // sub-records today), populated for club.
    pub player_sub_records: &'static [SubRecord],
    pub staff_sub_records: &'static [SubRecord],
    pub club_sub_records: &'static [SubRecord],
    pub stadium_sub_records: &'static [SubRecord],

    // Struct sizes / read sizes.
    pub player_struct_size: usize,
    pub staff_struct_size: usize,
    pub club_read_size: usize,
    /// Stadium entity stride (inline array, not pointer array). FM26: 0xC0 = 192 bytes.
    pub stadium_entity_stride: usize,

    // Human-manager layout (per-version because the shifted-field set may
    // differ between games).
    pub human_manager_shift: usize,
    pub manager_shifted_fields: &'static [(usize, usize)],

    // RVAs from game_plugin.dll base.
    pub rva_humans_data: u64,
    pub rva_humans_end: u64,
    pub rva_staff_data: u64,
    pub rva_staff_end: u64,
    pub rva_clubs_data: u64,
    pub rva_clubs_end: u64,

    /// Stadium array — accessed via a 5-hop pointer chain, NOT a simple data/end pair:
    ///   GP+rva_stadium_base → container → container+0x80 → type_desc → vec_hdr → data_ptr
    ///   entity[i] = data_ptr + i * stadium_entity_stride
    /// This RVA points to the container object (first hop), not directly to the array.
    pub rva_stadium_base: u64,

    // FMRTE reference counts — used by `src/bin/scan_arrays.rs` to
    // validate candidate RVA pairs. Per-version because FMRTE's own
    // numbers shift between game versions.
    pub fmrte_players: u64,
    pub fmrte_staff: u64,
    pub fmrte_clubs: u64,
    pub fmrte_humans: u64,

    /// FNV-1a hash of the first 64 KB of `game_plugin.dll` for the patch
    /// this schema was validated against.
    ///
    /// Convention:
    ///   0  — not yet calibrated; attach is allowed but a notice is shown.
    ///   other — hard gate: attach fails if the running DLL hashes differently.
    ///
    /// To calibrate: launch the editor once with the target FM build, read
    /// the hash from the status bar notice, paste it here, commit.
    pub known_dll_hash: u64,

    /// RVA (module-relative) of the vtable of the club finance class whose
    /// layout the club `finance` sub-record fields describe. 0 = no check.
    /// FM27 has a full finance class (budgets, facility copies) for its ~900
    /// detailed clubs and a smaller class for the rest, where those offsets
    /// fall outside the object; the loader drops the finance sub-record for
    /// any club whose finance object has a different vtable.
    pub full_finance_vtable_rva: u64,

    /// Player-coach layout. A player-coach object is a player object followed
    /// by `player_coach_shift` bytes of staff data and then the person block,
    /// so every player field at or past `player_person_offset` (names, UID,
    /// DOB, nation, caps, contract pointer, ...) lives `player_coach_shift`
    /// bytes later than in a plain player. Identified by the object's vtable
    /// at `player_coach_vtable_rva` (module-relative); 0 = no player-coach
    /// handling.
    pub player_coach_vtable_rva: u64,
    pub player_person_offset: usize,
    pub player_coach_shift: usize,

    /// Year ages are computed against (game start season). FM26 keeps the
    /// editor's historical 2026; FM24 saves start in July 2023.
    pub age_reference_year: u16,
}

impl VersionedSchemas {
    /// Look up a single field by key across all entities of this version.
    pub fn field(&self, key: &str) -> Option<&Field> {
        self.player_fields
            .iter()
            .chain(self.staff_fields.iter())
            .chain(self.club_fields.iter())
            .chain(self.stadium_fields.iter())
            .find(|f| f.key == key)
    }

    /// Return the field slice for a given entity type on this version.
    pub fn fields_for(&self, entity: Entity) -> &'static [Field] {
        match entity {
            Entity::Player => self.player_fields,
            Entity::Staff => self.staff_fields,
            Entity::Club => self.club_fields,
            Entity::Stadium => self.stadium_fields,
        }
    }

    /// True if `rel_offset` (relative to the human-manager base) falls inside
    /// one of this version's shifted identity ranges, i.e. a write to it must
    /// go to `rel_offset + human_manager_shift`.
    pub fn is_manager_shifted_offset(&self, rel_offset: u64) -> bool {
        self.manager_shifted_fields
            .iter()
            .any(|&(off, len)| rel_offset >= off as u64 && rel_offset < (off + len) as u64)
    }
}
