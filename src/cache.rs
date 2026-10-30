//! In-memory entity cache — built once on connect, never mutated post-load.
//!
//! `EntityCache` holds flat structs for every player, staff member, and club
//! in the current save.  The worker thread builds it via `bulk_load` and
//! ships it to the app thread wrapped in an `Arc`; the app stores a clone of
//! the Arc so no data is copied across the channel.
//!
//! `raw` byte slices carry the full struct bytes so detail panels can read
//! any field without a bridge round-trip.  The per-entity Option fields
//! are scaffolded for columns that aren't mapped yet; they slot in once the
//! relevant offsets are added to `src/fields/fm26/`.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Per-entity cached structs
// ---------------------------------------------------------------------------

pub struct CachedPlayer {
    pub addr:      u64,
    pub index:     usize,
    pub name:      String,
    pub uid:       u32,
    pub ca:        u8,
    pub pa:        u8,
    pub dob_year:  u16,
    /// Full 968-byte player struct — used by the detail panel for instant
    /// field reads without a bridge round-trip.
    pub raw:       Vec<u8>,
    /// Person-block shift in memory (player-coaches); see `player::read_player_raw`.
    pub person_shift: usize,
    // Unmapped list-column fields (None until offsets added to fields/fm26/player.rs):
    pub nation_id: Option<u32>,
    pub club_id:   Option<u32>,
    pub value:     Option<u64>,
}

pub struct CachedStaff {
    pub addr:      u64,
    pub index:     usize,
    pub name:      String,
    pub uid:       u32,
    pub ca:        u8,
    pub pa:        u8,
    pub dob_year:  u16,
    pub roles:     [u8; 15],
    pub role_ptr:  u64,
    /// Full 576-byte staff struct — used by the detail panel.
    pub raw:       Vec<u8>,
    // Unmapped list-column fields:
    pub nation_id: Option<u32>,
    pub club_id:   Option<u32>,
}

pub struct CachedClub {
    pub addr:            u64,
    pub index:           usize,
    pub uid:             u32,
    pub name:            String,
    pub gender:          u8,
    pub transfer_budget: Option<u32>,
    // Unmapped list-column fields:
    pub nation_id:       Option<u32>,
    pub status:          Option<u8>,
    pub reputation:      Option<u16>,
}

// ---------------------------------------------------------------------------
// Top-level cache
// ---------------------------------------------------------------------------

pub struct EntityCache {
    pub players:    Vec<CachedPlayer>,
    pub staff:      Vec<CachedStaff>,
    pub clubs:      Vec<CachedClub>,
    /// uid → short name; built from `clubs` on cache completion.
    /// Used to display the Club column in player/staff list rows once
    /// club_id fields are mapped.
    pub club_names: HashMap<u32, String>,
}
