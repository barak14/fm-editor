//! FM27 per-version field tables, same layout as `fields::fm26`.
//!
//! Fields not listed as verified in docs/fm27/memory-layout.md remain
//! inherited leads from FM26, not verified FM27 mappings.

mod club;
#[cfg(debug_assertions)]
pub(crate) mod contract;
mod player;
pub(crate) mod policy;
mod stadium;
pub mod staff;

pub(crate) use club::CLUB_SUB_RECORDS;
pub(crate) use club::FIELDS as CLUB_FIELDS;
pub(crate) use player::FIELDS as PLAYER_FIELDS;
pub(crate) use player::PLAYER_SUB_RECORDS;
pub(crate) use stadium::FIELDS as STADIUM_FIELDS;
pub(crate) use staff::FIELDS as STAFF_FIELDS;
pub(crate) use staff::STAFF_SUB_RECORDS;

use crate::fields::{ModuleAddresses, PeIdentity, RecordedBuild, ReleaseLayout};
use crate::game_version::{GameVersion, Storefront};

pub static FM27: ReleaseLayout = ReleaseLayout {
    player_fields: PLAYER_FIELDS,
    staff_fields: STAFF_FIELDS,
    club_fields: CLUB_FIELDS,
    stadium_fields: STADIUM_FIELDS,

    player_sub_records: PLAYER_SUB_RECORDS,
    staff_sub_records: STAFF_SUB_RECORDS,
    club_sub_records: CLUB_SUB_RECORDS,
    stadium_sub_records: &[],
    player_lists: Some(&player::FM27_PLAYER_LISTS),
    morale_labels: player::MORALE_LABELS,
    person_profile: Some(&staff::PERSON_PROFILE),

    player_struct_size: 992, // 0x3E0 — covers block-2 name sub-records at 0x3D0/0x3D8
    staff_struct_size: 576,
    club_read_size: 0x2000,
    stadium_entity_stride: 0xC0, // 192 bytes, flat inline array (was 0x170 before the 2026-10 patch)

    human_manager_shift: staff::HUMAN_MANAGER_SHIFT,
    manager_shifted_fields: staff::MANAGER_SHIFTED_FIELDS,
    player_person_offset: 0x288,
    player_coach_shift: 0xF8,
    age_reference_year: 2027,
};

pub static FM27_1: ReleaseLayout = ReleaseLayout {
    club_fields: club::FIELDS_27_1,
    ..FM27
};

/// FM27 Steam build. FM27 is a Unity build like FM26: game logic lives in
/// `game_plugin.dll`, and the entity RVAs below are relative to its base.
/// Values not listed as verified in docs/fm27/memory-layout.md are inherited
/// from FM26.
pub static FM27_STEAM: RecordedBuild = RecordedBuild {
    version: GameVersion::Fm27,
    storefront: Storefront::Steam,
    layout: &FM27,
    schema_name: "FM27_STEAM",
    addresses: ModuleAddresses {
        // FM27 keeps FM26's array cluster layout, shifted +0x8BA158 as a block.
        // Found by sampling elements of every heap vector in .sdata and decoding
        // names at FM26's offsets: players (p_name 0x2C8), staff (s_name 0x140),
        // clubs (c_full_name 0xC0 / c_name 0xC8) all decode. Stadium RVA is the
        // same shift applied, not yet checked.
        rva_humans_data: 0x57037D0,
        rva_humans_end: 0x57037D8,
        rva_staff_data: 0x5703818,
        rva_staff_end: 0x5703820,
        rva_clubs_data: 0x5703988,
        rva_clubs_end: 0x5703990,
        rva_stadium_base: 0x5703750,
        // Current game date (DateDayYear, flags above bit 8). Witnesses: raw
        // 0x07EA1AC2 (day 194) on Mon 13 Jul 2026; 195/2026 after one day of
        // holiday, Tue 14 Jul 2026 (2026-10-31). A pointer path reads the same
        // date: +0x56F1A18 -> game object (vtable +0x4C8C858) + 0x7F4.
        rva_current_date: 0x56AD378,
        // Full club finance class: Spurs, Arsenal, Man City, Liverpool and 881
        // other clubs. 32,682 clubs use a smaller class (vtable +0x4CF07F8) where
        // the budget offsets read packed dates and floats.
        full_finance_vtable_rva: 0x4CF0CC8,
        // Player-coaches (391 in this save): vtable +0x4F75A08, person block at
        // +0x288+0xF8. Witnesses: James Milner (born 4 Jan 1986, Leeds, England,
        // 61 caps), David Nugent, Keisuke Honda (FC Jurong), Remko Pasveer (Twente).
        player_coach_vtable_rva: 0x4F75A08,
        // Human-manager staff class: the user's manager (last staff entry) and the
        // entry FM creates for the player's Steam profile (named "(<account
        // name>)", staff id 2002106548, no club, CA/PA 100/200). Both showed as
        // nameless ID-0 rows in the staff list.
        human_manager_vtable_rva: 0x4CC086C,
    },

    // No FMRTE reference counts exist for FM27 yet. These are the counts
    // observed in a fresh FM27 save (day 1 of the first season); they
    // depend on the database size chosen at game creation.
    fmrte_players: 106868,
    fmrte_staff: 51466,
    fmrte_clubs: 33570,
    fmrte_humans: 1,

    // Not calibrated: attach reports the running game_plugin.dll hash.
    fingerprint: 0,
    // FMLibreEditor profiles/fm27/steam-26227640.toml (3db3d77, 2026-11-23), not checked here (27.0.0+2486695).
    pe_identities: &[PeIdentity {
        timestamp: 0x6ADFB11C,
        size_of_image: 0x19E69000,
    }],
    checked_here: true,
};

/// FM27 schema, PC Game Pass build (Microsoft GDK, Xbox app install), Store
/// package 0.9.2435.0 (27.0.1).
///
/// The Steam struct layouts with this binary's own module hash and
/// module-relative RVAs (entity arrays, current date, class vtables).
/// Evidence and remaining gaps in docs/fm27/memory-layout.md ("Game Pass build").
pub static FM27_GAMEPASS: RecordedBuild = RecordedBuild {
    storefront: Storefront::GamePass,
    schema_name: "FM27_GAMEPASS",
    // game_plugin.dll of Store package 0.9.2435.0 (2026-11-12).
    fingerprint: 0xAAD1_D2D3_AD51_8F23,
    // Jackhamerp24/Football-Manager-27-Editor offsets/27.0.1+2494687-gamepass.json (80f2d27, 2026-11-23), not checked here.
    pe_identities: &[PeIdentity {
        timestamp: 0x6AE8CF77,
        size_of_image: 0x17969000,
    }],
    addresses: ModuleAddresses {
        // Current game date, same encoding as Steam: raw 0x07EA1AC2 (day 194,
        // Mon 13 Jul 2026) on a new save. Only date-shaped static with that value.
        rva_current_date: 0x53597C8,
        // Same array cluster as Steam, shifted -0x34B9F0 as a block. Found by
        // decoding names at the Steam struct offsets in every heap vector.
        rva_humans_data: 0x53B7DE0,
        rva_humans_end: 0x53B7DE8,
        rva_staff_data: 0x53B7E28,
        rva_staff_end: 0x53B7E30,
        rva_clubs_data: 0x53B7F98,
        rva_clubs_end: 0x53B7FA0,
        // Same shift applied; not yet checked.
        rva_stadium_base: 0x53B7D60,
        // Class RVAs, matched to the Steam classes by a vtable census of each
        // array: 158 full-finance clubs (Arsenal, Aston Villa; small class
        // +0x4A2F518, 0x4D0 below it as on Steam), 77 player-coaches (staff-side
        // class +0x4C5FE6C), and the last two staff entries for the human manager.
        full_finance_vtable_rva: 0x4A2F9E8,
        player_coach_vtable_rva: 0x4C5FE88,
        human_manager_vtable_rva: 0x4A072BC,
    },
    ..FM27_STEAM
};

/// FM27 Game Pass, Store package 0.9.2524.0 (27.1.0+2505117). Same layouts as
/// the earlier package; evidence in docs/fm27/memory-layout.md ("Store package
/// 0.9.2524.0").
pub static FM27_GAMEPASS_27_1: RecordedBuild = RecordedBuild {
    schema_name: "FM27_GAMEPASS_27_1",
    layout: &FM27_1,
    fingerprint: 0x136C_8A4D_64A3_35FD,
    // Jackhamerp24/Football-Manager-27-Editor offsets/27.1.0+2505117-gamepass.json (80f2d27, 2026-11-23), not checked here.
    pe_identities: &[PeIdentity {
        timestamp: 0x6AF5C8C6,
        size_of_image: 0x179AD000,
    }],
    addresses: ModuleAddresses {
        // Raw 0x07EA1AC2 (day 194, 13 Jul 2026), the only 2026 date-shaped
        // static near the arrays; the module scan found 7 date-shaped words there.
        rva_current_date: 0x539DD10,
        // The 0.9.2435.0 cluster shifted +0x430A8 as a block, found by the
        // module scan (layout_detect) and decoded at the FM27 struct offsets.
        rva_humans_data: 0x53FAE88,
        rva_humans_end: 0x53FAE90,
        rva_staff_data: 0x53FAED0,
        rva_staff_end: 0x53FAED8,
        rva_clubs_data: 0x53FB040,
        rva_clubs_end: 0x53FB048,
        // Same shift applied; not yet checked.
        rva_stadium_base: 0x53FAE08,
        // Class RVAs from the scan's vtable census: 38 full-finance clubs (small
        // class +0x4A702E8, 0x4D0 below it), 22 player-coaches, and the last two
        // staff entries for the human manager.
        full_finance_vtable_rva: 0x4A707B8,
        player_coach_vtable_rva: 0x4C9ECB8,
        human_manager_vtable_rva: 0x4A471CC,
    },
    ..FM27_GAMEPASS
};

// Jackhamerp24/Football-Manager-27-Editor offsets/27.0.0+2493426-steam.json (80f2d27, 2026-11-23), not checked here.
pub static FM27_STEAM_27_0_0_2493426: RecordedBuild = RecordedBuild {
    schema_name: "FM27_STEAM_27_0_0_2493426",
    // Fingerprint and HUMAN_NON_PLAYER vbtable from the installed DLL (2026-11-23), not checked in game.
    fingerprint: 0xCB99_521F_57D4_2B51,
    pe_identities: &[PeIdentity {
        timestamp: 0x6AE565AF,
        size_of_image: 0x1821C000,
    }],
    checked_here: false,
    addresses: ModuleAddresses {
        rva_humans_data: 0x57057D0,
        rva_humans_end: 0x57057D8,
        rva_staff_data: 0x5705818,
        rva_staff_end: 0x5705820,
        rva_clubs_data: 0x5705988,
        rva_clubs_end: 0x5705990,
        rva_stadium_base: 0x5705750,
        // FMLibreEditor profiles/fm27/steam-26340721.toml (3db3d77, 2026-11-23), not checked here (27.0.0+2493426).
        rva_current_date: 0x56AF378,
        full_finance_vtable_rva: 0x4CF2CC8,
        player_coach_vtable_rva: 0x4F77A08,
        human_manager_vtable_rva: 0x4CC286C,
    },
    ..FM27_STEAM
};

// Jackhamerp24/Football-Manager-27-Editor offsets/27.0.1+2494687-steam.json and offsets/27.0.1+2503222-steam.json (80f2d27, 2026-11-23), not checked here.
pub static FM27_STEAM_27_0_1: RecordedBuild = RecordedBuild {
    schema_name: "FM27_STEAM_27_0_1",
    fingerprint: 0,
    pe_identities: &[
        PeIdentity {
            timestamp: 0x6AE8B5A4,
            size_of_image: 0x17CB9000,
        },
        PeIdentity {
            timestamp: 0x6AF38E5C,
            size_of_image: 0x185FA000,
        },
    ],
    checked_here: false,
    addresses: ModuleAddresses {
        rva_humans_data: 0x5820810,
        rva_humans_end: 0x5820818,
        rva_staff_data: 0x5820858,
        rva_staff_end: 0x5820860,
        rva_clubs_data: 0x58209C8,
        rva_clubs_end: 0x58209D0,
        rva_stadium_base: 0x5820790,
        // FMLibreEditor profiles/fm27/steam-26385295.toml (3db3d77, 2026-11-23), not checked here (27.0.1+2494687).
        // FMLibreEditor profiles/fm27/steam-26577676.toml (3db3d77, 2026-11-23), not checked here (27.0.1+2503222).
        rva_current_date: 0x57CA3B8,
        full_finance_vtable_rva: 0x4DF4E38,
        player_coach_vtable_rva: 0x508FCB8,
        human_manager_vtable_rva: 0,
    },
    ..FM27_STEAM
};

// Jackhamerp24/Football-Manager-27-Editor offsets/27.1.0+2505117-steam.json (80f2d27, 2026-11-23), not checked here.
pub static FM27_STEAM_27_1: RecordedBuild = RecordedBuild {
    schema_name: "FM27_STEAM_27_1",
    layout: &FM27_1,
    fingerprint: 0,
    pe_identities: &[PeIdentity {
        timestamp: 0x6AF5C68F,
        size_of_image: 0x18878000,
    }],
    checked_here: false,
    addresses: ModuleAddresses {
        rva_humans_data: 0x587B8B8,
        rva_humans_end: 0x587B8C0,
        rva_staff_data: 0x587B900,
        rva_staff_end: 0x587B908,
        rva_clubs_data: 0x587BA70,
        rva_clubs_end: 0x587BA78,
        rva_stadium_base: 0x587B838,
        // FMLibreEditor profiles/fm27/steam-26620374.toml (3db3d77, 2026-11-23), not checked here (27.1.0+2505117).
        rva_current_date: 0x58268E0,
        full_finance_vtable_rva: 0x4E52C38,
        player_coach_vtable_rva: 0x50EAC28,
        human_manager_vtable_rva: 0,
    },
    ..FM27_STEAM
};

/// FM27 Steam patch 27.1.1+2511875 (Steam build ID 26797085, 2026-11-25).
///
/// Jackhamerp24/Football-Manager-27-Editor offsets/27.1.1+2511875-steam.json and
/// FMLibreEditor profiles/fm27/steam-26797085.toml (fetched 2026-11-26). The
/// arrays, the date and the full-finance class stay where 27.1.0 keeps them;
/// the player-coach class moved. Fingerprint from the installed DLL.
/// Evidence in docs/fm27/memory-layout.md ("Steam 27.1.1 build").
pub static FM27_STEAM_27_1_1: RecordedBuild = RecordedBuild {
    schema_name: "FM27_STEAM_27_1_1",
    fingerprint: 0x1300_4084_36D8_2A8F,
    pe_identities: &[PeIdentity {
        timestamp: 0x6AFF57A2,
        size_of_image: 0x1849A000,
    }],
    checked_here: false,
    addresses: ModuleAddresses {
        player_coach_vtable_rva: 0x50EAAF8,
        // From the editor's own RTTI census on the live game (2026-11-26);
        // the same table as the 27.1.0 value checked on 2026-11-18.
        human_manager_vtable_rva: 0x4E2170C,
        ..FM27_STEAM_27_1.addresses
    },
    ..FM27_STEAM_27_1
};

// FMLibreEditor profiles/fm27/epic-27.1.0.toml (3db3d77, 2026-11-23), not checked here.
// The probe found the same RVAs as Steam 26620374 (27.1.0+2505117), linked about four minutes earlier.
pub static FM27_EPIC_27_1: RecordedBuild = RecordedBuild {
    storefront: Storefront::Epic,
    schema_name: "FM27_EPIC_27_1",
    pe_identities: &[PeIdentity {
        timestamp: 0x6AF5C793,
        size_of_image: 0x18095000,
    }],
    ..FM27_STEAM_27_1
};

// Jackhamerp24/Football-Manager-27-Editor offsets/27.0.0+2493426-gamepass.json (80f2d27, 2026-11-23), not checked here.
pub static FM27_GAMEPASS_27_0_0: RecordedBuild = RecordedBuild {
    schema_name: "FM27_GAMEPASS_27_0_0",
    fingerprint: 0,
    pe_identities: &[PeIdentity {
        timestamp: 0x6AE563A1,
        size_of_image: 0x175DA000,
    }],
    checked_here: false,
    addresses: ModuleAddresses {
        rva_humans_data: 0x5683DC0,
        rva_humans_end: 0x5683DC8,
        rva_staff_data: 0x5683E08,
        rva_staff_end: 0x5683E10,
        rva_clubs_data: 0x5683F78,
        rva_clubs_end: 0x5683F80,
        rva_stadium_base: 0x5683D40,
        rva_current_date: 0x56257C8,
        full_finance_vtable_rva: 0x4C9FD78,
        player_coach_vtable_rva: 0x4F20D28,
        human_manager_vtable_rva: 0,
    },
    ..FM27_GAMEPASS
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::{Entity, sub_direct};

    #[test]
    fn fm27_1_changes_only_the_corporate_facilities_offset() {
        for entity in [Entity::Player, Entity::Staff, Entity::Club, Entity::Stadium] {
            let old = FM27.fields_for(entity);
            let new = FM27_1.fields_for(entity);
            assert_eq!(old.len(), new.len());
            for (old, new) in old.iter().zip(new) {
                assert_eq!(
                    (old.key, old.label, old.entity, old.category),
                    (new.key, new.label, new.entity, new.category)
                );
                if old.key != "c_corporate" {
                    assert_eq!(old.offset, new.offset, "{}", old.key);
                }
            }
        }
        assert_eq!(
            FM27.field("c_corporate").unwrap().offset,
            sub_direct("finance", 0x8A9)
        );
        assert_eq!(
            FM27_1.field("c_corporate").unwrap().offset,
            sub_direct("finance", 0x8AD)
        );
    }

    #[test]
    fn fm27_supporter_mix_identifies_corporate_and_casual_bytes() {
        for layout in [&FM27, &FM27_1] {
            let corporate = layout.field("c_sup_other").unwrap();
            assert_eq!(corporate.label, "Corporate %");
            assert_eq!(corporate.offset, sub_direct("info", 0x10A));
            let casual = layout.field("c_sup_casual").unwrap();
            assert_eq!(casual.offset, sub_direct("info", 0x10B));
        }
    }
}
