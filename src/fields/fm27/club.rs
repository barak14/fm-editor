//! Club field and sub-record definitions for FM27. Aggregated into
//! `fields::all_fields` via `fields::fm27::CLUB_FIELDS`, and into
//! `FM27::club_sub_records` via `CLUB_SUB_RECORDS`.
//!
//! UNVERIFIED: copied from the FM26 schema as the FM27 starting point.
//! Every offset below must be re-verified against FM27 witnesses.

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
    // FM27: the finance object holds the budgets (+0x7C4..) and the facility
    // copies (+0x8A6..) directly, so it is read up to +0x8B0.
    SubRecord { id: "finance", parent: SubRecordParent::Direct,         ptr_offset: 0x150, size: 0x8B0 },
    // FM26's finance+0x48 budget pointer does not resolve on FM27 (null for
    // Arsenal, Spurs, Man City, Liverpool); the budgets live in finance itself.
    SubRecord { id: "stadium", parent: SubRecordParent::Direct,         ptr_offset: 0xB0,  size: 0x98  },
    // Home stadium: club+0x18 is the team vector's data pointer; entry 0 is the
    // senior team (team UID == club UID for Bayern, Spurs, Man City, Madrid),
    // and team+0x78 points at the stadium entity. Resolves for 1779 of a
    // 2000-club sample (the rest are null: clubs without a ground).
    SubRecord { id: "teams",        parent: SubRecordParent::Direct,             ptr_offset: 0x18, size: 0x08 },
    // Read to +0xB0 to cover the club reputation at senior_team+0xA8.
    SubRecord { id: "senior_team",  parent: SubRecordParent::In("teams"),        ptr_offset: 0x00, size: 0xB0 },
    SubRecord { id: "home_stadium", parent: SubRecordParent::In("senior_team"),  ptr_offset: 0x78, size: 0xC0 },
];

pub(crate) const FIELDS: &[Field] = &[
    // u64 pointer-vector bounds, verified Huelva -> Huelva B and Granada -> Granada B.
    Field { key: "c_reserve_teams_begin", label: "Reserve teams begin", entity: Club, category: Internal, offset: pointer(0x30) },
    Field { key: "c_reserve_teams_end", label: "Reserve teams end", entity: Club, category: Internal, offset: pointer(0x38) },
    // ── Top-level direct offsets ──────────────────────────────────────────
    Field { key: "c_uid",                label: "UID",                entity: Club, category: General,    offset: raw(0x0C)      },
    Field { key: "c_name",               label: "Name",               entity: Club, category: General,    offset: fm_string(0xC8)},
    Field { key: "c_full_name",          label: "Full Name",          entity: Club, category: General,    offset: fm_string(0xC0)},
    // FM27: 0 = men, 1 = women. Splits the club array 30,771 / 2,799, matching
    // the two club sub-arrays at gp+0x57039E8 / gp+0x5703A90. Witnesses: Spurs,
    // Arsenal, Man City, Liverpool (men 0, women 1). +0x161 (FM26) reads 10.
    Field { key: "c_gender",             label: "Gender",             entity: Club, category: General,    offset: raw(0xB9)      },
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
    // Club reputation: u16 0-10000 on the senior team object (club+0x18 →
    // teams[0] + 0xA8). Arsenal 8950, Spurs 7650, Man City 9150, Liverpool 8850.
    Field { key: "c_reputation",         label: "Reputation",         entity: Club, category: General,    offset: sub_raw("senior_team", 0xA8) },
    Field { key: "c_nation",             label: "Nation",             entity: Club, category: General,    offset: None           },

    // ── Info sub-record (+0x100 from club base) ───────────────────────────
    // FM27: every info field after the abbreviations sits +8 from FM26.
    // Witnesses: Spurs 1882 / Spurs Women 1985, Arsenal 1886, Man City 1894,
    // Liverpool 1892 (u16 — the following u16 is non-zero for Liverpool);
    // season tickets Spurs 50000, Arsenal 42000; social Spurs 6.8M, City 59M.
    Field { key: "c_year_founded",       label: "Year Founded",       entity: Club, category: General,    offset: sub_raw("info",    0xD8) },
    // FM26's +0xD6 + 8 reads 10000 (Spurs, City), 9000 (Arsenal, Liverpool),
    // 0 (every women's team) — not an owner status. Meaning unknown; unmapped.
    Field { key: "c_owner_status",       label: "Owner Status",       entity: Club, category: General,    offset: None },
    Field { key: "c_season_tickets",     label: "Season Tickets",     entity: Club, category: General,    offset: sub_raw("info",    0xC0) },
    Field { key: "c_social_media",       label: "Social Media",       entity: Club, category: General,    offset: sub_raw("info",    0xF8) },
    // Facilities (1-20). FM27 witness: Arsenal's Club Info > Facilities screen
    // shows Training "Superb" (+0x120 = 19), Youth "Excellent" (+0x12B = 16),
    // Academy Coaching "Excellent" (+0x12C = 17), Youth Recruitment "Good"
    // (+0x12D = 15). The FM26-inherited labels had Training and Youth swapped.
    // Academy coaching and recruitment also have a copy in the finance object
    // (+0x8A6 / +0x8A7) that FM's screen reads; only the info copy is mapped.
    Field { key: "c_training",           label: "Training",           entity: Club, category: Facilities, offset: sub_direct("info", 0x120)},
    Field { key: "c_youth_fac",          label: "Youth Fac",          entity: Club, category: Facilities, offset: sub_direct("info", 0x12B)},
    // FM26's "youth improvement" (+0x126) and "youth academy" (+0x121) match
    // nothing on FM27's Facilities screen (Arsenal 15 / 16); unmapped.
    Field { key: "c_youth_improve",      label: "Youth Improvement",  entity: Club, category: Facilities, offset: None },
    Field { key: "c_youth_coach",        label: "Youth Coaching",     entity: Club, category: Facilities, offset: sub_direct("info", 0x12C)},
    Field { key: "c_youth_recruit",      label: "Youth Recruitment",  entity: Club, category: Facilities, offset: sub_direct("info", 0x12D)},
    // Finance-object copies FM's Facilities screen reads. Equal to the info
    // copies for all four witnesses; edits write both (see club_panel).
    // Corporate exists only here: Arsenal 19 "Excellent", Spurs 20, Man City
    // 14, Liverpool 16.
    Field { key: "c_youth_coach_fin",    label: "Youth Coaching (finance copy)",    entity: Club, category: Internal,   offset: sub_direct("finance", 0x8A6)},
    Field { key: "c_youth_recruit_fin",  label: "Youth Recruitment (finance copy)", entity: Club, category: Internal,   offset: sub_direct("finance", 0x8A7)},
    Field { key: "c_corporate",          label: "Corporate",          entity: Club, category: Facilities, offset: sub_direct("finance", 0x8A9)},
    Field { key: "c_youth_academy",      label: "Youth Academy",      entity: Club, category: Facilities, offset: None },

    // ── Info sub-record: supporter profile percentages (+0x106..+0x10B) ─────
    // 6 u8 values that sum to exactly 100. FM27 is FM26 +8.
    // Order matches FM's supporter profile (hardcore→core→family→fair_weather→casual→other).
    // FM27: S=[10,37,15,19,5,14], MC=[6,32,18,22,3,19], Arsenal=[7,26,24,17,4,22],
    // Liverpool=[7,30,26,15,4,18] (all sum to 100).
    Field { key: "c_sup_hardcore",    label: "Hardcore %",    entity: Club, category: General, offset: sub_direct("info", 0x106) },
    Field { key: "c_sup_core",        label: "Core %",        entity: Club, category: General, offset: sub_direct("info", 0x107) },
    Field { key: "c_sup_family",      label: "Family %",      entity: Club, category: General, offset: sub_direct("info", 0x108) },
    Field { key: "c_sup_fair_weather",label: "Fair Weather %",entity: Club, category: General, offset: sub_direct("info", 0x109) },
    Field { key: "c_sup_casual",      label: "Casual %",      entity: Club, category: General, offset: sub_direct("info", 0x10A) },
    Field { key: "c_sup_other",       label: "Other %",       entity: Club, category: General, offset: sub_direct("info", 0x10B) },

    // ── Info sub-record: supporter attributes (+0x114..+0x119) ───────────────
    // 6 u8 values on a 1-20 scale; FM27 is FM26 +8. Order: loyalty, passion,
    // patience, affluence, temperament, expectations.
    // FM27: S=[18,14,7,14,7,14] and MC=[18,10,10,9,11,8], identical to FM26.
    Field { key: "c_sup_loyalty",     label: "Loyalty",       entity: Club, category: General, offset: sub_direct("info", 0x114) },
    Field { key: "c_sup_passion",     label: "Passion",       entity: Club, category: General, offset: sub_direct("info", 0x115) },
    Field { key: "c_sup_patience",    label: "Patience",      entity: Club, category: General, offset: sub_direct("info", 0x116) },
    Field { key: "c_sup_affluence",   label: "Affluence",     entity: Club, category: General, offset: sub_direct("info", 0x117) },
    Field { key: "c_sup_temperament", label: "Temperament",   entity: Club, category: General, offset: sub_direct("info", 0x118) },
    Field { key: "c_sup_expectations",label: "Expectations",  entity: Club, category: General, offset: sub_direct("info", 0x119) },

    // ── Finance sub-record (+0x150 from club base) ────────────────────────
    // FM27: balance is at finance+0x0C (FM26 +0x14 reads ~1.14-1.15bn for every
    // club). Witnesses: Spurs £173.9M (confirmed on FM27's finance screen),
    // Arsenal £333.1M, Man City £478.5M, Liverpool £185.6M.
    Field { key: "c_balance",            label: "Balance",            entity: Club, category: Finance,    offset: sub_raw("finance", 0x0C) },
    // FM26's ticket-price floats (+0x18/+0x1C) read 0.6-1.4 on FM27; unmapped.
    // The average match ticket price is likely the f32 at finance+0x864
    // (Spurs 65, Arsenal 49), outside the 0x50-byte finance read; not mapped yet.
    // Average match ticket price, f32 pounds: Spurs 65, Arsenal 49, Man City
    // 50, Liverpool 49. FM derives the season-ticket price from it.
    Field { key: "c_avg_ticket",         label: "Avg Ticket Price",   entity: Club, category: Finance,    offset: sub_f32("finance", 0x864) },
    Field { key: "c_avg_season_ticket",  label: "Avg Season Ticket",  entity: Club, category: Finance,    offset: None },

    // ── Budgets (i32 pounds, directly in the full finance object) ───────
    // Confirmed on FM27's Spurs finance screen: Spending Wage Budget
    // £4,355,776 p/w = +0x814; Total Wage Budget £4.5M (incl. youth grant
    // £900 at +0x7DC) and Next Season's Budget £4,505,776 = +0x808 and +0x834,
    // equal for all four witnesses, so which is which is not settled.
    // Transfer budget £25.9M (+0x7C4) confirmed on the same screen.
    // Arsenal / Man City / Liverpool: spending 4,440,326 / 5,352,976 /
    // 5,404,502; total 4,940,326 / 5,852,976 / 5,754,502.
    Field { key: "c_transfer_budget",    label: "Transfer Budget",    entity: Club, category: Finance,    offset: sub_raw("finance", 0x7C4) },
    Field { key: "c_transfer_remaining", label: "Transfer Remaining", entity: Club, category: Finance,    offset: None },
    Field { key: "c_wage_budget",        label: "Spending Wage Budget", entity: Club, category: Finance,  offset: sub_raw("finance", 0x814) },
    Field { key: "c_wage_total",         label: "Total Wage Budget",  entity: Club, category: Finance,    offset: sub_raw("finance", 0x808) },
    Field { key: "c_youth_grant",        label: "Youth Grant Income", entity: Club, category: Finance,    offset: sub_raw("finance", 0x7DC) },


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
