//! Player field definitions for FM27. Aggregated into `fields::all_fields`
//! via `fields::fm27::PLAYER_FIELDS`.
//!
//! UNVERIFIED: copied from the FM26 schema as the FM27 starting point.
//! Every offset below must be re-verified against FM27 witnesses.

use crate::fields::{direct, date, encoded, fm_string, pointer, raw, raw_u32, sub_date, sub_direct, sub_fm_string, sub_raw_u32, Field, SubRecord, SubRecordParent};
use crate::fields::Category::*;
use crate::fields::Entity::Player;

/// Player sub-records, resolved by `player::read_player_at`.
pub(crate) const PLAYER_SUB_RECORDS: &[SubRecord] = &[
    // Contract: pointer at player+0x330. contract+0x08 points back at the
    // player's person object (player+0x288) for every witness, which is how
    // the chain was validated (Haaland, Yamal, Mbappé, Saka, Ronaldo).
    SubRecord { id: "contract", parent: SubRecordParent::Direct, ptr_offset: 0x330, size: 0xC4 },
    // Team the player plays for (player+0x130); the team's club is at
    // team+0x30. Haaland → Manchester City, Yamal → Barcelona, Mbappé → R. Madrid.
    SubRecord { id: "team",       parent: SubRecordParent::Direct,     ptr_offset: 0x130, size: 0xB0 },
    SubRecord { id: "club",       parent: SubRecordParent::In("team"), ptr_offset: 0x30,  size: 0xD0 },
    // Nation object (player+0x2F0); its name FmString is at nation+0x18.
    // Norway, Spain, France, England, Portugal for the five witnesses.
    SubRecord { id: "nation",     parent: SubRecordParent::Direct,     ptr_offset: 0x2F0, size: 0x20 },
    // Place-of-birth city object (player+0x308); name FmString at city+0x18.
    // Leeds, Esplugues de Llobregat, Bondy, London, Funchal.
    SubRecord { id: "birth_city", parent: SubRecordParent::Direct,     ptr_offset: 0x308, size: 0x20 },
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
        // Common name (the name FM displays when set): same sub-record layout.
        // Witnesses: Lamine Yamal (first "Lamine Yamal", last "Nasraoui", common
        // "Lamine Yamal"), Cristiano Ronaldo (last "Aveiro", common "Cristiano
        // Ronaldo"), Vinícius Ciganha Marques Gouvêa (common "Vinícius").
        Field { key: "p_name_common_sub",  label: "Name Common Sub",  entity: Player, category: Internal, offset: pointer(0x02E8) },
        // FM26's secondary name block (+0x3C0/+0x3D0/+0x3D8) does not exist on
        // FM27: +0x3C0 holds the U21 caps/goals bytes below. Unmapped.
        Field { key: "p_name2",            label: "Name (secondary)",  entity: Player, category: Internal, offset: None },
        Field { key: "p_name2_first_sub",  label: "Name2 First Sub",   entity: Player, category: Internal, offset: None },
        Field { key: "p_name2_last_sub",   label: "Name2 Last Sub",    entity: Player, category: Internal, offset: None },
        Field { key: "p_height",       label: "Height",           entity: Player, category: General,     offset: raw(0x022E) },
        Field { key: "p_value",        label: "Market Value",     entity: Player, category: General,     offset: raw_u32(0x0234) },
        // FM26's weekly wage (+0x248) reads 13748 for Haaland on FM27; the real
        // wage is the contract's (`p_contract_wage`). Unmapped.
        Field { key: "p_weekly_wage",  label: "Weekly Wage",      entity: Player, category: General,     offset: None },
        Field { key: "p_dob",          label: "Date of Birth",    entity: Player, category: General,     offset: date(0x0310) },
        // FM26's "senior apps/goals" (+0x3BC/+0x3BE) are international caps and
        // goals on FM27, followed by U21 caps and goals as single bytes.
        // Witnesses: Haaland 55 caps / 62 goals (confirmed in game), Yamal 33/7,
        // Mbappé 106/66; Haaland U21 caps 3.
        Field { key: "p_senior_apps",  label: "Snr Apps",         entity: Player, category: General,     offset: None },
        Field { key: "p_senior_goals", label: "Snr Goals",        entity: Player, category: General,     offset: None },
        Field { key: "p_intl_caps",    label: "International Caps",  entity: Player, category: General,  offset: raw(0x03BC) },
        Field { key: "p_intl_goals",   label: "International Goals", entity: Player, category: General,  offset: raw(0x03BE) },
        Field { key: "p_u21_caps",     label: "U21 Caps",            entity: Player, category: General,  offset: direct(0x03C0) },
        Field { key: "p_u21_goals",    label: "U21 Goals",           entity: Player, category: General,  offset: direct(0x03C1) },
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
        // FM26's +0x128 "club entity" does not lead to a club on FM27. The
        // player's team is at +0x130 and the team's club at team+0x30.
        // Witnesses: Haaland → Manchester City, Yamal → Barcelona, Mbappé → R. Madrid.
        Field { key: "p_club_ptr",       label: "Club Entity",       entity: Player, category: Internal,    offset: None },
        Field { key: "p_team_ptr",       label: "Team",              entity: Player, category: Internal,    offset: pointer(0x0130) },

        // ── Contract sub-record fields (pointer at +0x330) ────────────────
        // FM27 witnesses (Haaland / Yamal / Mbappé / Saka / Ronaldo):
        //   wage £/week 472500 / 343612 / 576610 / 300000 / 3413698
        //   expiry 30 Jun 2034 / 2031 / 2029 / 2031 / 2027
        //   squad status 1 Star / 2 Important / 1 Star / 2 Important / 5 Impact Sub
        //   squad number 9 / 10 / 10 / 7 / 255 (none)
        //   signed Haaland 17 Jan 2025, Mbappé 1 Jul 2024
        // FM26's squad status (+0x24) reads 11 for every player; +0x26 is the
        // job code (1 = player), not a contract type.
        Field { key: "p_contract_wage",    label: "Contract Wage",    entity: Player, category: Contract,   offset: sub_raw_u32("contract", 0x20) },
        Field { key: "p_squad_status",     label: "Squad Status",     entity: Player, category: Contract,   offset: sub_direct("contract", 0x54)  },
        Field { key: "p_contract_type",    label: "Contract Type",    entity: Player, category: Contract,   offset: None },
        Field { key: "p_contract_signed",  label: "Contract Signed",  entity: Player, category: Contract,   offset: sub_date("contract", 0x44)    },
        Field { key: "p_contract_expiry",  label: "Contract Expiry",  entity: Player, category: Contract,   offset: sub_date("contract", 0x48)    },
        Field { key: "p_squad_number",     label: "Squad Number",     entity: Player, category: Contract,   offset: sub_direct("contract", 0x5D)  },
        // Transfer-list flags: bit 0 transfer listed, bit 1 loan listed (other
        // bits are FM's own — Yamal has 0x40). Lead from FM27€0; not yet
        // checked against a listed player.
        Field { key: "p_list_flags",       label: "Listing Flags",    entity: Player, category: Contract,   offset: sub_direct("contract", 0x57)  },
        // Asking price, i32 pounds; -1 = not set (all five witnesses).
        Field { key: "p_asking_price",     label: "Asking Price",     entity: Player, category: Contract,   offset: raw_u32(0x0238) },

        // Names reached through sub-record chains (see PLAYER_SUB_RECORDS).
        Field { key: "p_current_club",     label: "Club",             entity: Player, category: General,    offset: sub_fm_string("club", 0xC0) },
        Field { key: "p_nationality",      label: "Nationality",      entity: Player, category: General,    offset: sub_fm_string("nation", 0x18) },
        Field { key: "p_birthplace",       label: "Place of Birth",   entity: Player, category: General,    offset: sub_fm_string("birth_city", 0x18) },

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
        // UID is a u32 at person+0x0C (Haaland 29179241, Mbappé 85139014,
        // Yamal 2000256231). FM26's raw(u16) read gave 15721 for Haaland.
        Field { key: "p_uid",          label: "UID",              entity: Player, category: General,     offset: raw_u32(0x0294) },
        Field { key: "p_traits",       label: "Traits (PPMs)",    entity: Player, category: Traits,      offset: raw(0x0348) }, // 64-bit bitmask, bit = PPM_ID - 1
];
