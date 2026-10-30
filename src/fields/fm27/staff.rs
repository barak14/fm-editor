//! Staff field definitions for FM27. Aggregated into `fields::all_fields`
//! via `fields::fm27::STAFF_FIELDS`.
//!
//! UNVERIFIED: copied from the FM26 schema as the FM27 starting point.
//! Every offset below must be re-verified against FM27 witnesses.

use crate::fields::{date, direct, encoded, fm_string, pointer, raw, raw_capped, role_index, sub_date, sub_direct, sub_fm_string, sub_raw_u32, Field, SubRecord, SubRecordParent};
use crate::fields::Category::*;
use crate::fields::Entity::Staff;

/// Staff sub-records, resolved from the staff base (the human manager's from
/// base + `HUMAN_MANAGER_SHIFT`, where his identity block lives).
pub(crate) const STAFF_SUB_RECORDS: &[SubRecord] = &[
    // Contract: pointer at staff+0x1A8. contract+0x08 points back at the
    // staff's person object (staff+0x100) for every witness.
    SubRecord { id: "contract",   parent: SubRecordParent::Direct,         ptr_offset: 0x1A8, size: 0x60 },
    // Employer: contract+0x10 → team, team+0x30 → club.
    SubRecord { id: "team",       parent: SubRecordParent::In("contract"), ptr_offset: 0x10,  size: 0xB0 },
    SubRecord { id: "club",       parent: SubRecordParent::In("team"),     ptr_offset: 0x30,  size: 0xD0 },
    // Nation (staff+0x168) and place of birth (staff+0x180); name FmString
    // at +0x18 in both objects.
    SubRecord { id: "nation",     parent: SubRecordParent::Direct,         ptr_offset: 0x168, size: 0x20 },
    SubRecord { id: "birth_city", parent: SubRecordParent::Direct,         ptr_offset: 0x180, size: 0x20 },
];

pub(crate) const FIELDS: &[Field] = &[
        // =====================================================================
        // Coaching
        // =====================================================================
        Field { key: "s_goalkeeping",  label: "Goalkeeping",      entity: Staff, category: Coaching,      offset: encoded(0x02B) },
        Field { key: "s_attacking",    label: "Attacking",        entity: Staff, category: Coaching,      offset: encoded(0x032) },
        Field { key: "s_defending",    label: "Defending",        entity: Staff, category: Coaching,      offset: encoded(0x033) },
        Field { key: "s_fitness",      label: "Fitness",          entity: Staff, category: Coaching,      offset: encoded(0x034) },
        Field { key: "s_possession",   label: "Possession",       entity: Staff, category: Coaching,      offset: encoded(0x035) },
        Field { key: "s_tactical",     label: "Tactical",         entity: Staff, category: Coaching,      offset: encoded(0x037) },
        Field { key: "s_technical",    label: "Technical",        entity: Staff, category: Coaching,      offset: encoded(0x036) },
        Field { key: "s_set_pieces",   label: "Set Pieces",       entity: Staff, category: Coaching,      offset: encoded(0x043) },
        Field { key: "s_youngsters",   label: "Youngsters",       entity: Staff, category: Coaching,      offset: direct(0x01C) },

        // Mental
        Field { key: "s_authority",       label: "Authority",         entity: Staff, category: Mental,      offset: direct(0x014) },
        Field { key: "s_determination",   label: "Determination",     entity: Staff, category: Mental,      offset: encoded(0x01D) },
        Field { key: "s_motivating",      label: "Motivation",        entity: Staff, category: Mental,      offset: encoded(0x02F) },
        Field { key: "s_people_mgmt",     label: "People Mgmt",       entity: Staff, category: Mental,      offset: encoded(0x02E) },

        // Knowledge
        Field { key: "s_judge_ability",   label: "Judge Ability",     entity: Staff, category: Knowledge,   offset: encoded(0x02C) },
        Field { key: "s_judge_potential",  label: "Judge Potential",   entity: Staff, category: Knowledge,   offset: encoded(0x02D) },
        Field { key: "s_judge_staff",     label: "Judge Staff",       entity: Staff, category: Knowledge,   offset: encoded(0x042) },
        Field { key: "s_negotiating",     label: "Negotiating",       entity: Staff, category: Knowledge,   offset: encoded(0x041) },
        Field { key: "s_tactical_knowl",  label: "Tactical Knowledge",entity: Staff, category: Knowledge,   offset: encoded(0x031) },
        Field { key: "s_data_analysis",   label: "Data Analysis",     entity: Staff, category: Knowledge,   offset: encoded(0x03C) },

        // Medical
        Field { key: "s_physiotherapy",   label: "Physiotherapy",     entity: Staff, category: Medical,     offset: encoded(0x030) },
        Field { key: "s_sports_science",  label: "Sports Science",    entity: Staff, category: Medical,     offset: encoded(0x03F) },

        // Tactical Style
        Field { key: "s_ts_attacking",  label: "Attacking",         entity: Staff, category: TacticalStyle, offset: direct(0x010) },
        Field { key: "s_def_line",      label: "Defensive Line",    entity: Staff, category: TacticalStyle, offset: direct(0x023) },
        Field { key: "s_directness",    label: "Directness",        entity: Staff, category: TacticalStyle, offset: direct(0x013) },
        Field { key: "s_flexibility",   label: "Flexibility",       entity: Staff, category: TacticalStyle, offset: direct(0x025) },
        Field { key: "s_tempo",         label: "Tempo",             entity: Staff, category: TacticalStyle, offset: direct(0x028) },
        Field { key: "s_width",         label: "Width",             entity: Staff, category: TacticalStyle, offset: direct(0x029) },
        Field { key: "s_trigger_press", label: "Trigger Press",     entity: Staff, category: TacticalStyle, offset: direct(0x01A) },
        Field { key: "s_rotation",      label: "Squad Rotation",    entity: Staff, category: TacticalStyle, offset: direct(0x027) },
        Field { key: "s_hardness",      label: "Train Hardness",    entity: Staff, category: TacticalStyle, offset: direct(0x026) },
        Field { key: "s_dirtiness",     label: "Dirtiness",         entity: Staff, category: TacticalStyle, offset: encoded(0x038) },
        Field { key: "s_versatility",   label: "Versatility",       entity: Staff, category: TacticalStyle, offset: encoded(0x03B) },

        // Board Relations
        Field { key: "s_buying_players", label: "Buying Players",   entity: Staff, category: BoardRelations, offset: direct(0x01E) },
        Field { key: "s_business",       label: "Business",         entity: Staff, category: BoardRelations, offset: direct(0x011) },
        Field { key: "s_interference",   label: "Interference",     entity: Staff, category: BoardRelations, offset: direct(0x016) },
        Field { key: "s_mind_games",     label: "Mind Games",       entity: Staff, category: BoardRelations, offset: direct(0x01F) },
        Field { key: "s_patience",       label: "Patience",         entity: Staff, category: BoardRelations, offset: direct(0x019) },
        Field { key: "s_resources",      label: "Resources",        entity: Staff, category: BoardRelations, offset: direct(0x01B) },

        // Personality (at +0x170)
        Field { key: "s_adaptability",    label: "Adaptability",     entity: Staff, category: Personality, offset: direct(0x170) },
        Field { key: "s_ambition",        label: "Ambition",         entity: Staff, category: Personality, offset: direct(0x171) },
        Field { key: "s_loyalty",         label: "Loyalty",          entity: Staff, category: Personality, offset: direct(0x172) },
        Field { key: "s_pressure",        label: "Pressure",         entity: Staff, category: Personality, offset: direct(0x173) },
        Field { key: "s_professionalism", label: "Professionalism",  entity: Staff, category: Personality, offset: direct(0x174) },
        Field { key: "s_sportsmanship",   label: "Sportsmanship",    entity: Staff, category: Personality, offset: direct(0x175) },
        Field { key: "s_temperament",     label: "Temperament",      entity: Staff, category: Personality, offset: direct(0x176) },
        Field { key: "s_controversy",     label: "Controversy",      entity: Staff, category: Personality, offset: direct(0x177) },

        // Contract (sub-record at staff+0x1A8). Witnesses (job / club / wage
        // £/week / expiry): Arteta 16 Head Coach / Arsenal / 375000 / 30 Jun
        // 2030; Flick Head Coach / Barcelona / 99218 / 2028; Xabi Alonso Head
        // Coach / Chelsea / 210000 / 2030; Maresca Head Coach / Man City /
        // 150000 / 2029; the test save's human manager Head Coach /
        // Tottenham Hotspur / 120000 / 2028. Guardiola, Slot and Ancelotti
        // have no contract in this save (pointer 0).
        Field { key: "s_weekly_wage",     label: "Wage (£ p/w)",     entity: Staff, category: Contract, offset: sub_raw_u32("contract", 0x20) },
        Field { key: "s_contract_expiry", label: "Contract Expiry", entity: Staff, category: Contract, offset: sub_date("contract", 0x48) },
        // Job code: 16 = Head Coach, 20 = Assistant Coach, 2 = Coach, ...
        // (see `staff::job_name`).
        Field { key: "s_job",             label: "Job",             entity: Staff, category: Contract, offset: sub_direct("contract", 0x26) },

        // General
        // Witnesses: Arteta Spain / San Sebastián, Flick Germany / Heidelberg,
        // Xabi Alonso Spain / Tolosa, Guardiola Spain / Santpedor, the user's
        // manager England.
        Field { key: "s_nationality",     label: "Nationality",     entity: Staff, category: General,  offset: sub_fm_string("nation", 0x18) },
        Field { key: "s_birthplace",      label: "Place of Birth",  entity: Staff, category: General,  offset: sub_fm_string("birth_city", 0x18) },
        Field { key: "s_club",            label: "Club",            entity: Staff, category: General,  offset: sub_fm_string("club", 0xC0) },
        Field { key: "s_name",            label: "Name",            entity: Staff, category: General,  offset: fm_string(0x140) },
        // Name sub-record pointers — ~72% of staff have null s_name FmString;
        // their names live in these two 5×24-byte candidate blocks.
        // Witnesses: Frank Dauwen (+0x150→Frank, +0x158→Dauwen), Zvonimir Boban, René Henriksen, etc.
        Field { key: "s_name_first_sub",  label: "Name First Sub",  entity: Staff, category: Internal, offset: pointer(0x150) },
        Field { key: "s_name_last_sub",   label: "Name Last Sub",   entity: Staff, category: Internal, offset: pointer(0x158) },
        // Common name sub-record, same layout. Witnesses: Luis Enrique Martínez
        // García (common "Luis Enrique"), Joaquim Silva ("Quim"), Jon Pérez
        // Alonso ("Bolo").
        Field { key: "s_name_common_sub", label: "Name Common Sub", entity: Staff, category: Internal, offset: pointer(0x160) },
        Field { key: "s_ca",           label: "Current Ability",  entity: Staff, category: Ability,      offset: raw_capped(0x0DA, 200) },
        Field { key: "s_pa",           label: "Potential Ability", entity: Staff, category: Ability,     offset: raw_capped(0x0DC, 200) },
        Field { key: "s_id",           label: "Staff ID",         entity: Staff, category: General,      offset: raw(0x10C) },
        Field { key: "s_dob",          label: "Date of Birth",    entity: Staff, category: General,      offset: date(0x188) },
        // FM26's role-ratings pointer (+0x1D0) holds an unaligned non-pointer
        // value on FM27 (0x263009D03 for the manager); role ratings unmapped.
        Field { key: "s_role_ptr",     label: "Role Ratings",     entity: Staff, category: General,      offset: None },

        // =====================================================================
        // Staff Roles (15-byte sub-record reached via s_role_ptr)
        // Each entry's offset field is the byte index (0–14), not a memory offset.
        // Rendered by `roles_column`; declaration order = FMSE display order.
        // =====================================================================
        Field { key: "s_role_manager",    label: "Manager",            entity: Staff, category: StaffRole, offset: role_index(0) },
        Field { key: "s_role_asst_mgr",   label: "Asst Manager",       entity: Staff, category: StaffRole, offset: role_index(1) },
        Field { key: "s_role_coach",      label: "Coach",              entity: Staff, category: StaffRole, offset: role_index(2) },
        Field { key: "s_role_gk_coach",   label: "GK Coach",           entity: Staff, category: StaffRole, offset: role_index(5) },
        Field { key: "s_role_fit_coach",  label: "Fitness Coach",      entity: Staff, category: StaffRole, offset: role_index(6) },
        Field { key: "s_role_sp_coach",   label: "Set Piece Coach",    entity: Staff, category: StaffRole, offset: role_index(7) },
        Field { key: "s_role_scout",      label: "Scout",              entity: Staff, category: StaffRole, offset: role_index(4) },
        Field { key: "s_role_youth_dev",  label: "Head of Youth Dev",  entity: Staff, category: StaffRole, offset: role_index(10) },
        Field { key: "s_role_data",       label: "Data Analyst",       entity: Staff, category: StaffRole, offset: role_index(12) },
        Field { key: "s_role_loan",       label: "Loan Manager",       entity: Staff, category: StaffRole, offset: role_index(11) },
        Field { key: "s_role_dof",        label: "Dir of Football",    entity: Staff, category: StaffRole, offset: role_index(9) },
        Field { key: "s_role_physio",     label: "Physio",             entity: Staff, category: StaffRole, offset: role_index(3) },
        Field { key: "s_role_sports_sci", label: "Sports Scientist",   entity: Staff, category: StaffRole, offset: role_index(13) },
        Field { key: "s_role_tech_dir",   label: "Technical Director", entity: Staff, category: StaffRole, offset: role_index(14) },

        // Reputation (person-base embedded block, matching player layout)
        Field { key: "s_home_rep",     label: "Home Rep",         entity: Staff, category: Reputation,   offset: raw_capped(0x0D4, 10000) },
        Field { key: "s_current_rep",  label: "Current Rep",      entity: Staff, category: Reputation,   offset: raw_capped(0x0D6, 10000) },
        Field { key: "s_world_rep",    label: "World Rep",        entity: Staff, category: Reputation,   offset: raw_capped(0x0D8, 10000) },
];

// ---------------------------------------------------------------------------
// Human manager layout (FM27 — same scheme as FM26, shift 0x340)
// ---------------------------------------------------------------------------
//
// The human manager struct is a LARGER composite than a normal AI staff.
// The first ~576 bytes mostly match the standard AI staff layout for
// attributes, CA/PA, and reputations — but the "identity" fields (staff ID,
// name ptr, personality, DOB, role sub-record ptr) are zeroed at their
// standard offsets and instead duplicated at
// `standard_offset + HUMAN_MANAGER_SHIFT`.
//
// `load_human_manager` patches the shifted bytes into their standard
// offsets so the existing reader/UI code sees a normal staff layout, and
// `staff_panel::show_manager` redirects writes for those ranges by adding
// the shift to the target address before they go to the bridge.

/// Byte offset added to a standard staff field offset to reach the overlaid
/// "identity" block inside the FM27 human-manager struct. FM26 used 0x350.
/// Witness: the test save's human manager (staff id 2002106549) has
/// id at +0x44C/+0x450, name FmString at +0x480, personality at +0x4B0 and
/// DOB at +0x4C8; at 0x350 the same slots hold zeros and garbage. The role
/// sub-record pointer (+0x1D0 + shift) is not verified.
pub const HUMAN_MANAGER_SHIFT: usize = 0x340;

/// Staff field offset ranges that live at `offset + HUMAN_MANAGER_SHIFT` on
/// the FM26 human manager. Format: (offset, len).
pub const MANAGER_SHIFTED_FIELDS: &[(usize, usize)] = &[
    (0x10C, 4), // staff id
    (0x110, 4), // staff id duplicate
    (0x140, 8), // name pointer
    (0x170, 8), // personality (8 x u8)
    (0x188, 4), // DOB day (u16) + year (u16)
    (0x1D0, 8), // role sub-record pointer
];
