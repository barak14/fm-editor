//! Player field definitions for FM26. Aggregated into `fields::all_fields`
//! via `fields::fm26::PLAYER_FIELDS`.

use crate::fields::{direct, date, encoded, fm_string, pointer, raw, raw_u32, sub_date, sub_raw, sub_raw_u32, Field, SubRecord, SubRecordParent};
use crate::fields::Category::*;
use crate::fields::Entity::Player;

/// Player sub-records. Not yet loaded by PlayerRecord (player loading doesn't
/// support sub-records yet), but the definitions capture the discovered offsets.
pub(crate) const PLAYER_SUB_RECORDS: &[SubRecord] = &[
    // Contract sub-record: 196 bytes reached via pointer at player+0x330.
    // Contains wage, squad status, contract type, start/expiry dates.
    // Discovered 2026-04-17 from a memory snapshot of Kane's player entity.
    SubRecord { id: "contract", parent: SubRecordParent::Direct, ptr_offset: 0x330, size: 0xC4 },
];

pub(crate) const FIELDS: &[Field] = &[
        // =====================================================================
        // Technical
        // =====================================================================
        Field { key: "crossing",       label: "Crossing",        entity: Player, category: Technical,   offset: encoded(0x015F) },
        Field { key: "dribbling",      label: "Dribbling",       entity: Player, category: Technical,   offset: encoded(0x0160) },
        Field { key: "finishing",      label: "Finishing",        entity: Player, category: Technical,   offset: encoded(0x0161) },
        Field { key: "heading",        label: "Heading",          entity: Player, category: Technical,   offset: encoded(0x0162) },
        Field { key: "long_shots",     label: "Long Shots",       entity: Player, category: Technical,   offset: encoded(0x0163) },
        Field { key: "marking",        label: "Marking",          entity: Player, category: Technical,   offset: encoded(0x0164) },
        Field { key: "passing",        label: "Passing",          entity: Player, category: Technical,   offset: encoded(0x0166) },
        Field { key: "tackling",       label: "Tackling",         entity: Player, category: Technical,   offset: encoded(0x0168) },
        Field { key: "technique",      label: "Technique",        entity: Player, category: Technical,   offset: encoded(0x0176) },
        Field { key: "first_touch",    label: "First Touch",      entity: Player, category: Technical,   offset: encoded(0x0175) },

        // Set Pieces
        Field { key: "corners",        label: "Corners",          entity: Player, category: SetPieces,   offset: encoded(0x017A) },
        Field { key: "free_kicks",     label: "Free Kicks",       entity: Player, category: SetPieces,   offset: encoded(0x0182) },
        Field { key: "long_throws",    label: "Long Throws",      entity: Player, category: SetPieces,   offset: encoded(0x017D) },
        Field { key: "penalty_taking", label: "Penalty Taking",   entity: Player, category: SetPieces,   offset: encoded(0x0167) },

        // Mental
        Field { key: "aggression",     label: "Aggression",       entity: Player, category: Mental,      offset: encoded(0x018C) },
        Field { key: "anticipation",   label: "Anticipation",     entity: Player, category: Mental,      offset: encoded(0x0170) },
        Field { key: "bravery",        label: "Bravery",          entity: Player, category: Mental,      offset: encoded(0x018A) },
        Field { key: "composure",      label: "Composure",        entity: Player, category: Mental,      offset: encoded(0x0193) },
        Field { key: "concentration",  label: "Concentration",    entity: Player, category: Mental,      offset: encoded(0x0194) },
        Field { key: "decisions",      label: "Decisions",        entity: Player, category: Mental,      offset: encoded(0x0171) },
        Field { key: "flair",          label: "Flair",            entity: Player, category: Mental,      offset: encoded(0x0179) },
        Field { key: "leadership",     label: "Leadership",       entity: Player, category: Mental,      offset: encoded(0x0187) },
        Field { key: "off_the_ball",   label: "Off The Ball",     entity: Player, category: Mental,      offset: encoded(0x0165) },
        Field { key: "positioning",    label: "Positioning",      entity: Player, category: Mental,      offset: encoded(0x0173) },
        Field { key: "teamwork",       label: "Teamwork",         entity: Player, category: Mental,      offset: encoded(0x017B) },
        Field { key: "vision",         label: "Vision",           entity: Player, category: Mental,      offset: encoded(0x0169) },
        Field { key: "work_rate",      label: "Work Rate",        entity: Player, category: Mental,      offset: encoded(0x017C) },
        Field { key: "determination",  label: "Determination",    entity: Player, category: Mental,      offset: encoded(0x0192) },

        // Physical
        Field { key: "acceleration",   label: "Acceleration",     entity: Player, category: Physical,    offset: encoded(0x0181) },
        Field { key: "agility",        label: "Agility",          entity: Player, category: Physical,    offset: encoded(0x018D) },
        Field { key: "balance",        label: "Balance",          entity: Player, category: Physical,    offset: encoded(0x0189) },
        Field { key: "jumping_reach",  label: "Jumping Reach",    entity: Player, category: Physical,    offset: encoded(0x0186) },
        Field { key: "natural_fitness",label: "Natural Fitness",   entity: Player, category: Physical,    offset: encoded(0x0191) },
        Field { key: "pace",           label: "Pace",             entity: Player, category: Physical,    offset: encoded(0x0185) },
        Field { key: "stamina",        label: "Stamina",          entity: Player, category: Physical,    offset: encoded(0x0184) },
        Field { key: "strength",       label: "Strength",         entity: Player, category: Physical,    offset: encoded(0x0183) },

        // Goalkeeping
        Field { key: "handling",         label: "Handling",         entity: Player, category: Goalkeeping, offset: encoded(0x016A) },
        Field { key: "aerial_reach",     label: "Aerial Reach",     entity: Player, category: Goalkeeping, offset: encoded(0x016B) },
        Field { key: "command_of_area",  label: "Command of Area",  entity: Player, category: Goalkeeping, offset: encoded(0x016C) },
        Field { key: "communication",    label: "Communication",    entity: Player, category: Goalkeeping, offset: encoded(0x016D) },
        Field { key: "kicking",          label: "Kicking",          entity: Player, category: Goalkeeping, offset: encoded(0x016E) },
        Field { key: "throwing",         label: "Throwing",         entity: Player, category: Goalkeeping, offset: encoded(0x016F) },
        Field { key: "one_on_ones",      label: "One on Ones",      entity: Player, category: Goalkeeping, offset: encoded(0x0172) },
        Field { key: "reflexes",         label: "Reflexes",         entity: Player, category: Goalkeeping, offset: encoded(0x0174) },
        Field { key: "eccentricity",     label: "Eccentricity",     entity: Player, category: Goalkeeping, offset: encoded(0x017E) },
        Field { key: "rushing_out",      label: "Rushing Out",      entity: Player, category: Goalkeeping, offset: encoded(0x017F) },
        Field { key: "punching",         label: "Punching",         entity: Player, category: Goalkeeping, offset: encoded(0x0180) },

        // Footedness
        Field { key: "left_foot",      label: "Left Foot",        entity: Player, category: Footedness,  offset: encoded(0x0177) },
        Field { key: "right_foot",     label: "Right Foot",       entity: Player, category: Footedness,  offset: encoded(0x0178) },

        // Hidden
        Field { key: "consistency",       label: "Consistency",       entity: Player, category: Hidden,   offset: encoded(0x018B) },
        Field { key: "injury_proneness",  label: "Injury Proneness",  entity: Player, category: Hidden,   offset: encoded(0x018F) },
        Field { key: "important_matches", label: "Important Matches", entity: Player, category: Hidden,   offset: encoded(0x018E) },
        Field { key: "p_dirtiness",       label: "Dirtiness",         entity: Player, category: Hidden,   offset: encoded(0x0188) },
        Field { key: "p_versatility",     label: "Versatility",       entity: Player, category: Hidden,   offset: encoded(0x0190) },

        // Condition (offsets unknown)
        Field { key: "p_sharpness",    label: "Sharpness",        entity: Player, category: Condition,   offset: None },
        Field { key: "p_fitness",      label: "Fitness",           entity: Player, category: Condition,   offset: None },
        Field { key: "p_fatigue",      label: "Fatigue",           entity: Player, category: Condition,   offset: None },

        // Personality (at +0x2F8)
        Field { key: "p_adaptability",    label: "Adaptability",     entity: Player, category: Personality, offset: direct(0x02F8) },
        Field { key: "p_ambition",        label: "Ambition",         entity: Player, category: Personality, offset: direct(0x02F9) },
        Field { key: "p_loyalty",         label: "Loyalty",          entity: Player, category: Personality, offset: direct(0x02FA) },
        Field { key: "p_pressure",        label: "Pressure",         entity: Player, category: Personality, offset: direct(0x02FB) },
        Field { key: "p_professionalism", label: "Professionalism",  entity: Player, category: Personality, offset: direct(0x02FC) },
        Field { key: "p_sportsmanship",   label: "Sportsmanship",    entity: Player, category: Personality, offset: direct(0x02FD) },
        Field { key: "p_temperament",     label: "Temperament",      entity: Player, category: Personality, offset: direct(0x02FE) },
        Field { key: "p_controversy",     label: "Controversy",      entity: Player, category: Personality, offset: direct(0x02FF) },

        // Identity / General
        Field { key: "p_name",             label: "Name",             entity: Player, category: General,  offset: fm_string(0x02C8) },
        // Name-component sub-record pointers (block 1, +0x2C8 region): first entry gives the chosen name part.
        // Layout: 5×24-byte entries, each [u64 ptr_to_name_len][u32 id][u16 nation][u16 type][u32 flags][u32 pad].
        // ptr_to_name_len points directly at [u32 len][UTF-8 name_bytes].
        Field { key: "p_name_first_sub",   label: "Name First Sub",   entity: Player, category: Internal, offset: pointer(0x02D8) },
        Field { key: "p_name_last_sub",    label: "Name Last Sub",     entity: Player, category: Internal, offset: pointer(0x02E0) },
        // Secondary name block (+0x3C0 region): same layout as block 1.
        // ~209 players in a 50k-human save have null block-1 ptrs; block-2 supplies their names.
        // Witnesses (all block-1-null players rescued in diagnose_missing_names): Gary Hamilton,
        // James Henry, Tommi Björn Paavola, Ioan Viorel Ganea, Keith Foy, Dan Mihai Iurişniţi, …
        Field { key: "p_name2",            label: "Name (secondary)",  entity: Player, category: Internal, offset: fm_string(0x03C0) },
        Field { key: "p_name2_first_sub",  label: "Name2 First Sub",   entity: Player, category: Internal, offset: pointer(0x03D0) },
        Field { key: "p_name2_last_sub",   label: "Name2 Last Sub",    entity: Player, category: Internal, offset: pointer(0x03D8) },
        Field { key: "p_height",       label: "Height",           entity: Player, category: General,     offset: raw(0x022E) },
        Field { key: "p_value",        label: "Market Value",     entity: Player, category: General,     offset: raw_u32(0x0234) },
        Field { key: "p_weekly_wage",  label: "Weekly Wage",      entity: Player, category: General,     offset: raw(0x0248) },
        Field { key: "p_dob",          label: "Date of Birth",    entity: Player, category: General,     offset: date(0x0310) },
        Field { key: "p_senior_apps",  label: "Snr Apps",         entity: Player, category: General,     offset: raw(0x03BC) },
        Field { key: "p_senior_goals", label: "Snr Goals",        entity: Player, category: General,     offset: raw(0x03BE) },
        Field { key: "p_prev_club",    label: "Previous Club",    entity: Player, category: General,     offset: pointer(0x0390) },

        // Dates (discovered 2026-04-17 from a memory snapshot — Kane witnesses)
        // Transfer date: Kane moved to Bayern ~2022-08-20 (day 232 of 2022).
        Field { key: "p_transfer_date",  label: "Transfer Date",    entity: Player, category: General,     offset: date(0x0204) },
        // Contract start / join date: Kane 2023-08-12 (day 224 of 2023).
        Field { key: "p_contract_start", label: "Contract Start",   entity: Player, category: Contract,    offset: date(0x0250) },
        Field { key: "p_join_date",      label: "Join Club Date",   entity: Player, category: Contract,    offset: date(0x03A4) },
        // Senior debut: Kane's first Spurs appearance 2013-05-31 (day 151 of 2013).
        Field { key: "p_debut_date",     label: "Senior Debut",     entity: Player, category: General,     offset: date(0x03A8) },
        // Listing/event date.
        Field { key: "p_listing_date",   label: "Listing Date",     entity: Player, category: Contract,    offset: date(0x03B4) },

        // Pointers (discovered 2026-04-17 from a memory snapshot)
        // Nationality/nation array.
        Field { key: "p_nation_ptr",     label: "Nation",            entity: Player, category: Internal,    offset: pointer(0x02F0) },
        // Contract sub-record pointer.
        Field { key: "p_contract_ptr",   label: "Contract",          entity: Player, category: Internal,    offset: pointer(0x0330) },
        // Squad/match record pointer.
        Field { key: "p_squad_ptr",      label: "Squad Record",      entity: Player, category: Internal,    offset: pointer(0x0378) },
        // Club entity pointer at +0x128 (points to club entity data).
        Field { key: "p_club_ptr",       label: "Club Entity",       entity: Player, category: Internal,    offset: pointer(0x0128) },

        // ── Contract sub-record fields (at pointer from +0x330, 196 bytes) ────
        // Not yet readable (PlayerRecord doesn't load sub-records), but offsets captured.
        // Kane witnesses: wage=381198, squad_status=11, contract_type=1,
        //   signed=2023-08-12, expiry=2027-06-30.
        Field { key: "p_contract_wage",    label: "Contract Wage",    entity: Player, category: Contract,   offset: sub_raw_u32("contract", 0x20) },
        Field { key: "p_squad_status",     label: "Squad Status",     entity: Player, category: Contract,   offset: sub_raw("contract", 0x24)     },
        Field { key: "p_contract_type",    label: "Contract Type",    entity: Player, category: Contract,   offset: sub_raw("contract", 0x26)     },
        Field { key: "p_contract_signed",  label: "Contract Signed",  entity: Player, category: Contract,   offset: sub_date("contract", 0x44)    },
        Field { key: "p_contract_expiry",  label: "Contract Expiry",  entity: Player, category: Contract,   offset: sub_date("contract", 0x48)    },

        // Person base struct (embedded at +0x258)
        Field { key: "p_ca",           label: "Current Ability",  entity: Player, category: Ability,     offset: raw(0x0264) },
        Field { key: "p_pa",           label: "Potential Ability", entity: Player, category: Ability,     offset: raw(0x0266) },
        Field { key: "p_morale",       label: "Morale",           entity: Player, category: Condition,   offset: direct(0x026C) },
        Field { key: "p_home_rep",     label: "Home Rep",         entity: Player, category: Reputation,  offset: raw(0x025E) },
        Field { key: "p_current_rep",  label: "Current Rep",      entity: Player, category: Reputation,  offset: raw(0x0260) },
        Field { key: "p_world_rep",    label: "World Rep",        entity: Player, category: Reputation,  offset: raw(0x0262) },

        // Positions (15 x u8 at +0x150, direct 1-20)
        // All 15 confirmed via Kane, Kimmich, Litmanen, Gvardiol, Moore.
        Field { key: "pos_gk",  label: "GK",  entity: Player, category: Positions, offset: direct(0x0150) },
        Field { key: "pos_sw",  label: "SW",  entity: Player, category: Positions, offset: direct(0x0151) },
        Field { key: "pos_dl",  label: "DL",  entity: Player, category: Positions, offset: direct(0x0152) },
        Field { key: "pos_dc",  label: "DC",  entity: Player, category: Positions, offset: direct(0x0153) },
        Field { key: "pos_dr",  label: "DR",  entity: Player, category: Positions, offset: direct(0x0154) },
        Field { key: "pos_dm",  label: "DM",  entity: Player, category: Positions, offset: direct(0x0155) },
        Field { key: "pos_ml",  label: "ML",  entity: Player, category: Positions, offset: direct(0x0156) },
        Field { key: "pos_mc",  label: "MC",  entity: Player, category: Positions, offset: direct(0x0157) },
        Field { key: "pos_mr",  label: "MR",  entity: Player, category: Positions, offset: direct(0x0158) },
        Field { key: "pos_aml", label: "AML", entity: Player, category: Positions, offset: direct(0x0159) },
        Field { key: "pos_amc", label: "AMC", entity: Player, category: Positions, offset: direct(0x015A) },
        Field { key: "pos_amr", label: "AMR", entity: Player, category: Positions, offset: direct(0x015B) },
        Field { key: "pos_st",  label: "ST",  entity: Player, category: Positions, offset: direct(0x015C) },
        Field { key: "pos_wbl", label: "WBL", entity: Player, category: Positions, offset: direct(0x015D) },
        Field { key: "pos_wbr", label: "WBR", entity: Player, category: Positions, offset: direct(0x015E) },

        // Other
        Field { key: "p_uid",          label: "UID",              entity: Player, category: General,     offset: raw(0x0294) },
        Field { key: "p_current_club", label: "Current Club",     entity: Player, category: General,     offset: None },
        Field { key: "p_traits",       label: "Traits (PPMs)",    entity: Player, category: Traits,      offset: raw(0x0348) }, // 64-bit bitmask, bit = PPM_ID - 1
];
