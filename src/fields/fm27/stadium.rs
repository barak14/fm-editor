//! Stadium entity field definitions for FM27. Aggregated into
//! `fields::all_fields` via `fields::fm27::STADIUM_FIELDS`.
//! Stadium entities live in a flat inline array (stride 0xC0 = 192 bytes),
//! NOT a pointer array. Access chain from GP:
//!
//!   GP+rva_stadium_base → container → container+0x80 → type_desc → vec_hdr → data_ptr
//!   entity[i] = data_ptr + i * 0xC0
//!
//! A club reaches its home stadium through its senior team (see the
//! `home_stadium` club sub-record) and its training ground through the
//! pointer at club+0x140.
//!
//! Re-mapped against the 2026-10 FM26 patch. Witnesses: Allianz-Arena,
//! Tottenham Hotspur Stadium, Etihad Stadium, Santiago Bernabéu, Wembley,
//! Melbourne Cricket Ground, Michigan Stadium.
//!
//! UNVERIFIED: copied from the FM26 schema as the FM27 starting point.
//! Every offset below must be re-verified against FM27 witnesses.

use crate::fields::Category::*;
use crate::fields::Entity::Stadium;
use crate::fields::{date, direct, fm_string, pointer, raw, raw_capped, raw_u32, Encoding, Field, FieldLocation};

pub(crate) const FIELDS: &[Field] = &[
    // ── Identity ─────────────────────────────────────────────────────────
    // Wembley=102969, MCG=425196, Michigan=2000069874.
    Field { key: "st_uid",           label: "UID",            entity: Stadium, category: General, offset: raw_u32(0x0C) },
    Field { key: "st_name",          label: "Name",           entity: Stadium, category: General, offset: fm_string(0x40) },
    // Alternative / official name; null for most stadiums.
    // Allianz-Arena="Fußball Arena München", Etihad="City of Manchester Stadium".
    Field { key: "st_name2",         label: "Name 2",         entity: Stadium, category: General, offset: fm_string(0x48) },

    // ── Capacity ─────────────────────────────────────────────────────────
    // Michigan=115109/107601/0/107601/143886, MCG=97474/94984/0/0/101500,
    // Wembley=90000/90000/90000/0/90000, Allianz cap=75000 seated=61682 exp=75024.
    Field { key: "st_capacity",      label: "Capacity",        entity: Stadium, category: General, offset: raw_u32(0x7C) },
    Field { key: "st_seated",        label: "Seated",          entity: Stadium, category: General, offset: raw_u32(0x80) },
    Field { key: "st_used",          label: "Used",            entity: Stadium, category: General, offset: raw_u32(0x84) },
    Field { key: "st_all_seater",    label: "All-Seater",      entity: Stadium, category: General, offset: raw_u32(0x88) },
    Field { key: "st_expansion",     label: "Expansion",       entity: Stadium, category: General, offset: raw_u32(0x8C) },
    Field { key: "st_standing",      label: "Standing",        entity: Stadium, category: General, offset: None },

    // ── Pitch ────────────────────────────────────────────────────────────
    // Values are x10: 1050 = 105m, 680 = 68m. Michigan=1005x594 (in-game 100m x 59m).
    Field { key: "st_pitch_length",  label: "Pitch Length",    entity: Stadium, category: General, offset: raw(0x98) },
    Field { key: "st_pitch_width",   label: "Pitch Width",     entity: Stadium, category: General, offset: raw(0x9A) },

    // ── Pointers ─────────────────────────────────────────────────────────
    // Owning club: Allianz → FC Bayern, Bernabéu → Real Madrid, Tottenham Hotspur
    // Stadium → Tottenham. Null for non-club owners (Etihad, Wembley, MCG).
    Field { key: "st_club_ptr",      label: "Club Owner",      entity: Stadium, category: Internal, offset: pointer(0x90) },

    // Jackhamerp24/Football-Manager-27-Editor offsets/layouts/windows.json (80f2d27, 2026-11-23), not checked here.
    Field { key: "st_environment",       label: "Environment",          entity: Stadium, category: General, offset: direct(0xB0) },
    Field { key: "st_pitch_last_relaid",  label: "Pitch Last Relaid",    entity: Stadium, category: General, offset: date(0x28) },
    Field { key: "st_pitch_relay_required", label: "Pitch Relay Required", entity: Stadium, category: General, offset: date(0x2C) },
    Field { key: "st_build_date",        label: "Build Date",           entity: Stadium, category: General, offset: date(0x30) },
    Field { key: "st_rebuild_date",      label: "Rebuild Date",         entity: Stadium, category: General, offset: date(0x34) },
    Field { key: "st_latitude",    label: "Latitude",    entity: Stadium, category: General, offset: Some(FieldLocation::Direct { offset: 0x70, encoding: Encoding::F32Raw }) },
    Field { key: "st_longitude",   label: "Longitude",   entity: Stadium, category: General, offset: Some(FieldLocation::Direct { offset: 0x74, encoding: Encoding::F32Raw }) },
    Field { key: "st_seat_colour",       label: "Seat Colour",          entity: Stadium, category: General, offset: raw_u32(0x78) },
    Field { key: "st_min_pitch_length",  label: "Min Pitch Length (dm)", entity: Stadium, category: General, offset: raw(0xA0) },
    Field { key: "st_min_pitch_width",   label: "Min Pitch Width (dm)",  entity: Stadium, category: General, offset: raw(0xA2) },
    Field { key: "st_max_pitch_length",  label: "Max Pitch Length (dm)", entity: Stadium, category: General, offset: raw(0xA4) },
    Field { key: "st_max_pitch_width",   label: "Max Pitch Width (dm)",  entity: Stadium, category: General, offset: raw(0xA6) },
    Field { key: "st_pitch_condition",   label: "Pitch Condition",      entity: Stadium, category: General, offset: raw_capped(0xA8, 200) },
    Field { key: "st_decay",             label: "Decay",                entity: Stadium, category: General, offset: direct(0xAA) },
    Field { key: "st_pitch_deterioration", label: "Pitch Deterioration Rate", entity: Stadium, category: General, offset: direct(0xAC) },
    Field { key: "st_pitch_type",        label: "Pitch Type",           entity: Stadium, category: General, offset: direct(0xAD) },
    Field { key: "st_pitch_recovery",    label: "Pitch Recovery Rate",  entity: Stadium, category: General, offset: direct(0xAE) },
    Field { key: "st_national_team_use", label: "National Team Use",     entity: Stadium, category: General, offset: direct(0xB1) },
    Field { key: "st_national_u21_use",  label: "National U21 Use",      entity: Stadium, category: General, offset: direct(0xB2) },
    Field { key: "st_national_u19_use",  label: "National U19 Use",      entity: Stadium, category: General, offset: direct(0xB3) },
    Field { key: "st_owner_type",        label: "Owner Type",           entity: Stadium, category: General, offset: direct(0xB4) },

    // The club stadium panel reads this key as u8, not the u16 pitch condition.
    Field { key: "st_condition",     label: "Condition",       entity: Stadium, category: General, offset: None },
];
