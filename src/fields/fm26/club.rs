//! Club field and sub-record definitions for FM26. Aggregated into
//! `fields::all_fields` via `fields::fm26::CLUB_FIELDS`, and into
//! `FM26::club_sub_records` via `CLUB_SUB_RECORDS`.

use crate::fields::{fm_string, pointer, raw, raw_u32, sub_direct, sub_f32, sub_fm_string, sub_raw, Field, SubRecord, SubRecordParent};
use crate::fields::Category::*;
use crate::fields::Entity::Club;

/// Sub-records reachable from the club base pointer.
///
/// Order matters for the data-driven loader: parents must be listed before
/// children so that each sub-record's parent buffer is already resolved
/// when we get to it.
pub(crate) const CLUB_SUB_RECORDS: &[SubRecord] = &[
    SubRecord { id: "info",    parent: SubRecordParent::Direct,         ptr_offset: 0x100, size: 0x130 },
    SubRecord { id: "finance", parent: SubRecordParent::Direct,         ptr_offset: 0x150, size: 0x50  },
    SubRecord { id: "budget",  parent: SubRecordParent::In("finance"),  ptr_offset: 0x48,  size: 0x20  },
    SubRecord { id: "stadium", parent: SubRecordParent::Direct,         ptr_offset: 0xB0,  size: 0x98  },
    // Home stadium: club+0x18 is the team vector's data pointer; entry 0 is the
    // senior team (team UID == club UID for Bayern, Spurs, Man City, Madrid),
    // and team+0x78 points at the stadium entity. Resolves for 1779 of a
    // 2000-club sample (the rest are null: clubs without a ground).
    SubRecord { id: "teams",        parent: SubRecordParent::Direct,             ptr_offset: 0x18, size: 0x08 },
    SubRecord { id: "senior_team",  parent: SubRecordParent::In("teams"),        ptr_offset: 0x00, size: 0x80 },
    SubRecord { id: "home_stadium", parent: SubRecordParent::In("senior_team"),  ptr_offset: 0x78, size: 0xC0 },
];

pub(crate) const FIELDS: &[Field] = &[
    // ── Top-level direct offsets ──────────────────────────────────────────
    Field { key: "c_uid",                label: "UID",                entity: Club, category: General,    offset: raw(0x0C)      },
    Field { key: "c_name",               label: "Name",               entity: Club, category: General,    offset: fm_string(0xC8)},
    Field { key: "c_full_name",          label: "Full Name",          entity: Club, category: General,    offset: fm_string(0xC0)},
    Field { key: "c_gender",             label: "Gender",             entity: Club, category: General,    offset: raw(0x161)     },
    Field { key: "c_stadium",            label: "Stadium",            entity: Club, category: General,    offset: pointer(0xB0)  },
    Field { key: "c_index",              label: "Array Index",        entity: Club, category: General,    offset: raw(0x08)      },
    // Unknown top-level
    Field { key: "c_nickname",           label: "Nickname",           entity: Club, category: General,    offset: sub_fm_string("stadium", 0x90) },
    Field { key: "c_abbrev1",            label: "Abbreviation 1",     entity: Club, category: General,    offset: sub_fm_string("info", 0x00) },
    Field { key: "c_abbrev2",            label: "Abbreviation 2",     entity: Club, category: General,    offset: sub_fm_string("info", 0x08) },
    Field { key: "c_abbrev3",            label: "Abbreviation 3",     entity: Club, category: General,    offset: sub_fm_string("info", 0x10) },
    // Discovered 2026-04-17 from a memory snapshot: team_uid = club UID + 1 for all 4 witnesses.
    // Bayern=916, Spurs=729, ManCity=680, Madrid=1737.
    Field { key: "c_team_uid",           label: "Team UID",           entity: Club, category: General,    offset: raw_u32(0x017C) },
    // Pointer to the club's training-ground entity in the stadium array
    // (Bayern → Säbener Straße, Spurs → Hotspur Way). The home stadium is the
    // `home_stadium` sub-record.
    Field { key: "c_stadium_entity_ptr", label: "Training Ground",    entity: Club, category: Internal,   offset: pointer(0x0140) },
    Field { key: "c_reputation",         label: "Reputation",         entity: Club, category: General,    offset: None           },
    Field { key: "c_nation",             label: "Nation",             entity: Club, category: General,    offset: None           },

    // ── Info sub-record (+0x100 from club base) ───────────────────────────
    Field { key: "c_year_founded",       label: "Year Founded",       entity: Club, category: General,    offset: sub_raw("info",    0xD0) },
    Field { key: "c_owner_status",       label: "Owner Status",       entity: Club, category: General,    offset: sub_raw("info",    0xD6) },
    Field { key: "c_season_tickets",     label: "Season Tickets",     entity: Club, category: General,    offset: sub_raw("info",    0xB8) },
    Field { key: "c_social_media",       label: "Social Media",       entity: Club, category: General,    offset: sub_raw("info",    0xF0) },
    Field { key: "c_training",           label: "Training",           entity: Club, category: Facilities, offset: sub_direct("info", 0x123)},
    Field { key: "c_youth_fac",          label: "Youth Fac",          entity: Club, category: Facilities, offset: sub_direct("info", 0x118)},
    Field { key: "c_youth_improve",      label: "Youth Improvement",  entity: Club, category: Facilities, offset: sub_direct("info", 0x11E)},
    Field { key: "c_youth_coach",        label: "Youth Coaching",     entity: Club, category: Facilities, offset: sub_direct("info", 0x124)},
    Field { key: "c_youth_recruit",      label: "Youth Recruitment",  entity: Club, category: Facilities, offset: sub_direct("info", 0x125)},
    // Discovered 2026-04-17 from a memory snapshot: adjacent to youth_fac, likely youth_academy.
    Field { key: "c_youth_academy",      label: "Youth Academy",      entity: Club, category: Facilities, offset: sub_direct("info", 0x119)},

    // ── Info sub-record: supporter profile percentages (+0xFE..+0x103) ──────
    // 6 u8 values that sum to exactly 100 for all 4 witness clubs (Bayern,
    // Spurs, ManCity, Madrid) — confirmed percentage breakdown of supporter base.
    // Order matches FM's supporter profile (hardcore→core→family→fair_weather→casual→other).
    // B=Bayern, S=Spurs, MC=ManCity, RM=Madrid:
    //   B=[5,25,15,27,3,25], S=[9,37,14,21,5,14], MC=[5,32,18,23,3,19], RM=[5,22,18,25,5,25]
    Field { key: "c_sup_hardcore",    label: "Hardcore %",    entity: Club, category: General, offset: sub_direct("info", 0xFE)  },
    Field { key: "c_sup_core",        label: "Core %",        entity: Club, category: General, offset: sub_direct("info", 0xFF)  },
    Field { key: "c_sup_family",      label: "Family %",      entity: Club, category: General, offset: sub_direct("info", 0x100) },
    Field { key: "c_sup_fair_weather",label: "Fair Weather %",entity: Club, category: General, offset: sub_direct("info", 0x101) },
    Field { key: "c_sup_casual",      label: "Casual %",      entity: Club, category: General, offset: sub_direct("info", 0x102) },
    Field { key: "c_sup_other",       label: "Other %",       entity: Club, category: General, offset: sub_direct("info", 0x103) },

    // ── Info sub-record: supporter attributes (+0x10C..+0x111) ───────────────
    // 6 u8 values in 3-18 range (0-20 scale). Order follows FM's supporter
    // attributes: loyalty, passion, patience, affluence, temperament, expectations.
    // B=[11,9,3,18,14,18], S=[18,14,7,14,7,14], MC=[18,10,10,9,11,8], RM=[15,8,7,12,15,12]
    Field { key: "c_sup_loyalty",     label: "Loyalty",       entity: Club, category: General, offset: sub_direct("info", 0x10C) },
    Field { key: "c_sup_passion",     label: "Passion",       entity: Club, category: General, offset: sub_direct("info", 0x10D) },
    Field { key: "c_sup_patience",    label: "Patience",      entity: Club, category: General, offset: sub_direct("info", 0x10E) },
    Field { key: "c_sup_affluence",   label: "Affluence",     entity: Club, category: General, offset: sub_direct("info", 0x10F) },
    Field { key: "c_sup_temperament", label: "Temperament",   entity: Club, category: General, offset: sub_direct("info", 0x110) },
    Field { key: "c_sup_expectations",label: "Expectations",  entity: Club, category: General, offset: sub_direct("info", 0x111) },

    // ── Finance sub-record (+0x150 from club base) ────────────────────────
    Field { key: "c_balance",            label: "Balance",            entity: Club, category: Finance,    offset: sub_raw("finance", 0x14) },
    Field { key: "c_avg_ticket",         label: "Avg Ticket Price",   entity: Club, category: Finance,    offset: sub_f32("finance", 0x18) },
    Field { key: "c_avg_season_ticket",  label: "Avg Season Ticket",  entity: Club, category: Finance,    offset: sub_f32("finance", 0x1C) },

    // ── Budget sub-sub-record (finance+0x48) ──────────────────────────────
    Field { key: "c_transfer_budget",    label: "Transfer Budget",    entity: Club, category: Finance,    offset: sub_raw("budget",  0x04) },
    Field { key: "c_transfer_remaining", label: "Transfer Remaining", entity: Club, category: Finance,    offset: sub_raw("budget",  0x08) },
    Field { key: "c_wage_budget",        label: "Wage Budget",        entity: Club, category: Finance,    offset: sub_raw("budget",  0x0C) },

    // ── Stadium sub-record (+0xB0 from club base) ─────────────────────────
    // Note: +0x90 in the stadium sub-record is the club NICKNAME, not the stadium name.
    // Confirmed 2026-04-16: Bayern="Die Bayern", Spurs="The Lilywhites", Madrid="Los Merengues".
    // The c_nickname field above uses this offset via sub_fm_string.
    Field { key: "c_avg_attendance",     label: "Avg Attendance",     entity: Club, category: General,    offset: sub_raw("stadium", 0x68) },
    Field { key: "c_min_attendance",     label: "Min Attendance",     entity: Club, category: General,    offset: sub_raw("stadium", 0x6C) },
    Field { key: "c_max_attendance",     label: "Max Attendance",     entity: Club, category: General,    offset: sub_raw("stadium", 0x70) },

    // Stand type bytes (discovered 2026-04-17 from a memory snapshot).
    // B=Bayern, S=Spurs, MC=ManCity, RM=Madrid:
    //   +0x74: B=4, S=29, MC=4,  RM=29
    //   +0x75: B=3, S=4,  MC=29, RM=3
    //   +0x76: B=4, S=4,  MC=4,  RM=0
    //   +0x77: B=4, S=29, MC=29, RM=0
    Field { key: "c_stand_type_1",       label: "Stand Type 1",       entity: Club, category: General,    offset: sub_direct("stadium", 0x74) },
    Field { key: "c_stand_type_2",       label: "Stand Type 2",       entity: Club, category: General,    offset: sub_direct("stadium", 0x75) },
    Field { key: "c_stand_type_3",       label: "Stand Type 3",       entity: Club, category: General,    offset: sub_direct("stadium", 0x76) },
    Field { key: "c_stand_type_4",       label: "Stand Type 4",       entity: Club, category: General,    offset: sub_direct("stadium", 0x77) },
];
