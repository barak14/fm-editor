//! Game version selector + runtime dispatch.
//!
//! Every schema lookup routes through a `&'static VersionedSchemas` that
//! belongs to the game being read: `FmProcess::schemas()` on the worker side,
//! the attached game's version on the UI side. There is no process-global
//! "active version", so several games open at once cannot cross schemas.
//!
//! Version detection is automatic: FM24, FM26 and FM27 all run as `fm.exe`,
//! so `process::running_games()` lists them and reads each one's on-disk
//! module path — the install directory always contains the release year —
//! to determine which schema it uses.
//!
//! Adding a future version (e.g. FM25) is a 3-line change: new enum
//! variant, new match arm in `schemas()`, new `exe_name()` arm. The
//! actual field/RVA data for that version lives in its own sub-module
//! under `crate::fields::`.

use crate::fields::{fm24, fm26, fm27, VersionedSchemas};

/// Which game the editor is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameVersion {
    Fm24,
    Fm26,
    Fm27,
}

impl GameVersion {
    /// The per-version schema bundle (field lists, RVAs, sizes).
    pub fn schemas(self) -> &'static VersionedSchemas {
        match self {
            GameVersion::Fm24 => &fm24::FM24,
            GameVersion::Fm26 => &fm26::FM26,
            GameVersion::Fm27 => &fm27::FM27,
        }
    }

    /// Module that hosts the game logic and the entity-array RVAs.
    /// FM26 and FM27 (Unity) split it into `game_plugin.dll`; FM24 links it
    /// into `fm.exe` itself.
    pub fn module_name(self) -> &'static str {
        match self {
            GameVersion::Fm24 => "fm.exe",
            GameVersion::Fm26 | GameVersion::Fm27 => "game_plugin.dll",
        }
    }

    /// Human-readable name for status text and pickers (e.g. "FM26").
    pub fn display_name(self) -> &'static str {
        match self {
            GameVersion::Fm24 => "FM24",
            GameVersion::Fm26 => "FM26",
            GameVersion::Fm27 => "FM27",
        }
    }

    /// Short lowercase identifier used in source paths (e.g. "fm26").
    pub fn short_name(self) -> &'static str {
        match self {
            GameVersion::Fm24 => "fm24",
            GameVersion::Fm26 => "fm26",
            GameVersion::Fm27 => "fm27",
        }
    }
}
