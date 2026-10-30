//! Background worker thread that owns FmProcess. UI communicates via mpsc channels.
//!
//! This module is split across several files so that reverse-engineering a new
//! entity's search filters or bridge behaviour doesn't force a merge conflict
//! with unrelated work:
//!
//! - `cmd.rs` — the BridgeCmd / BridgeResult protocol enums, EntityCounts,
//!   StaffView / ClubView, and the UI-side ProcessBridge handle.
//! - `worker.rs` — the worker loop, ArrayCache, count-stability logic, and
//!   shared read helpers.
//! - `bulk_load.rs` — bulk entity cache loader; builds the EntityCache and
//!   sends CacheProgress / CacheReady results.
//! - `entity_load.rs` — loaders for the individual detail views
//!   (LoadStaff, LoadClub, LoadHumanManager) and the human-manager
//!   shifted-field layout.

mod bulk_load;
mod cmd;
mod entity_load;
mod worker;

pub use cmd::{
    BridgeCmd, BridgeResult, ClubView, EntityCounts, ProcessBridge, StaffView,
};
pub use entity_load::find_human_manager;
