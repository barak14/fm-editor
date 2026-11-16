//! Game version, storefront and build selector + runtime dispatch.
//!
//! Every schema lookup routes through a `&'static VersionedSchemas` that
//! belongs to the game being read: `FmProcess::schemas()` on the worker side,
//! the attached game's build on the UI side. There is no process-global
//! "active version", so several games open at once cannot cross schemas.
//!
//! A schema is selected per [`GameBuild`] — a release ([`GameVersion`]) as
//! shipped by one storefront ([`Storefront`]). The Game Pass build of a
//! release is a different binary from the Steam one (Microsoft GDK instead
//! of Steamworks), so its module hash and module-relative RVAs can differ
//! even when the struct layouts match.
//!
//! Detection is automatic: FM24, FM26 and FM27 all run as `fm.exe`.
//! `process::running_games()` reads the game module's PE product metadata,
//! falling back to the install directory if no FM identity is available.
//! Storefront hints are separate; identifying a release does not verify its
//! offsets. Attachment retains the per-schema module fingerprint gate.
//!
//! Adding a build (e.g. FM26 on Game Pass) is a new `GameBuild` variant,
//! its arms in the `GameBuild` matches, and its schema static under
//! `crate::fields::`.

use crate::fields::{VersionedSchemas, fm24, fm26, fm27};

/// Which Football Manager release a process is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameVersion {
    Fm24,
    Fm26,
    Fm27,
}

impl GameVersion {
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

/// Where a game install came from, from the install location. Epic is a
/// schema only for FM24; locations not recognised as Game Pass or Epic retain
/// the historical Steam default. Either way the attachment's module
/// fingerprint selects among the release's recorded builds
/// ([`GameBuild::with_fingerprint`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storefront {
    Steam,
    /// PC Game Pass / Microsoft Store (Xbox app) install.
    GamePass,
    /// Epic Games Store. Only FM24 has an Epic schema.
    Epic,
}

impl Storefront {
    /// Human-readable name for status text (e.g. "Game Pass").
    pub fn display_name(self) -> &'static str {
        match self {
            Storefront::Steam => "Steam",
            Storefront::GamePass => "Game Pass",
            Storefront::Epic => "Epic",
        }
    }
}

/// A release as shipped by one storefront — the unit a schema is selected
/// for. Only builds with a schema exist as variants, so a `GameBuild` always
/// has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameBuild {
    Fm24Steam,
    /// The Steam layout and addresses with the Epic binary's fingerprint
    /// (contributed evidence in docs/fm24/memory-layout.md).
    Fm24Epic,
    Fm26Steam,
    Fm27Steam,
    /// The Steam struct layouts with the Game Pass binary's own RVAs and
    /// fingerprint (see docs/fm27).
    Fm27GamePass,
}

impl GameBuild {
    /// Every build with a schema.
    pub const ALL: [GameBuild; 5] = [
        GameBuild::Fm24Steam,
        GameBuild::Fm24Epic,
        GameBuild::Fm26Steam,
        GameBuild::Fm27Steam,
        GameBuild::Fm27GamePass,
    ];

    /// The build for this release and storefront, or an error when no
    /// schema exists for that combination (e.g. FM26 on Game Pass).
    pub fn resolve(version: GameVersion, storefront: Storefront) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|b| b.version() == version && b.storefront() == storefront)
            .ok_or_else(|| {
                format!(
                    "no schema for the {} build of {} — only the {} build is mapped",
                    storefront.display_name(),
                    version.display_name(),
                    Storefront::Steam.display_name(),
                )
            })
    }

    /// The recorded build of the same release whose schema pins this module
    /// fingerprint, or `self` when none does. A moved or renamed install
    /// whose path gave the wrong storefront still gets its own schema; an
    /// unknown fingerprint is left to the attachment's fingerprint gate.
    pub fn with_fingerprint(self, hash: u64) -> Self {
        if self.schemas().known_dll_hash == hash {
            return self;
        }
        Self::ALL
            .into_iter()
            .find(|b| {
                let known = b.schemas().known_dll_hash;
                b.version() == self.version() && known != 0 && known == hash
            })
            .unwrap_or(self)
    }

    pub fn version(self) -> GameVersion {
        match self {
            GameBuild::Fm24Steam | GameBuild::Fm24Epic => GameVersion::Fm24,
            GameBuild::Fm26Steam => GameVersion::Fm26,
            GameBuild::Fm27Steam | GameBuild::Fm27GamePass => GameVersion::Fm27,
        }
    }

    pub fn storefront(self) -> Storefront {
        match self {
            GameBuild::Fm24Steam | GameBuild::Fm26Steam | GameBuild::Fm27Steam => Storefront::Steam,
            GameBuild::Fm24Epic => Storefront::Epic,
            GameBuild::Fm27GamePass => Storefront::GamePass,
        }
    }

    /// The per-build schema bundle (field lists, RVAs, sizes).
    pub fn schemas(self) -> &'static VersionedSchemas {
        match self {
            GameBuild::Fm24Steam => &fm24::FM24,
            GameBuild::Fm24Epic => &fm24::FM24_EPIC,
            GameBuild::Fm26Steam => &fm26::FM26,
            GameBuild::Fm27Steam => &fm27::FM27,
            GameBuild::Fm27GamePass => &fm27::FM27_GAMEPASS,
        }
    }

    /// Name of the schema static in `src/fields/<version>/mod.rs`, for
    /// messages that tell the developer what to edit.
    pub fn schema_name(self) -> &'static str {
        match self {
            GameBuild::Fm24Steam => "FM24",
            GameBuild::Fm24Epic => "FM24_EPIC",
            GameBuild::Fm26Steam => "FM26",
            GameBuild::Fm27Steam => "FM27",
            GameBuild::Fm27GamePass => "FM27_GAMEPASS",
        }
    }

    /// Human-readable name for status text and pickers. The Steam build is
    /// the default and keeps the bare version name ("FM27"); other
    /// storefronts are named ("FM27 Game Pass").
    pub fn display_name(self) -> &'static str {
        match self {
            GameBuild::Fm24Epic => "FM24 Epic",
            GameBuild::Fm27GamePass => "FM27 Game Pass",
            other => other.version().display_name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_round_trips_every_build() {
        for build in GameBuild::ALL {
            assert_eq!(
                GameBuild::resolve(build.version(), build.storefront()),
                Ok(build)
            );
        }
    }

    #[test]
    fn resolve_rejects_unmapped_storefront() {
        let err = GameBuild::resolve(GameVersion::Fm26, Storefront::GamePass).unwrap_err();
        assert!(err.contains("Game Pass") && err.contains("FM26"), "{err}");
        for version in [GameVersion::Fm26, GameVersion::Fm27] {
            let err = GameBuild::resolve(version, Storefront::Epic).unwrap_err();
            assert!(
                err.contains("Epic") && err.contains(version.display_name()),
                "{err}"
            );
        }
        assert_eq!(
            GameBuild::resolve(GameVersion::Fm24, Storefront::Epic),
            Ok(GameBuild::Fm24Epic)
        );
    }

    #[test]
    fn fingerprint_selects_the_recorded_build_of_the_same_release() {
        let epic = GameBuild::Fm24Epic.schemas().known_dll_hash;
        let steam = GameBuild::Fm24Steam.schemas().known_dll_hash;
        assert_eq!(
            GameBuild::Fm24Steam.with_fingerprint(epic),
            GameBuild::Fm24Epic
        );
        assert_eq!(
            GameBuild::Fm24Epic.with_fingerprint(steam),
            GameBuild::Fm24Steam
        );
        assert_eq!(
            GameBuild::Fm24Epic.with_fingerprint(epic),
            GameBuild::Fm24Epic
        );
        // An unknown fingerprint keeps the path's build for the gate to refuse.
        assert_eq!(
            GameBuild::Fm24Steam.with_fingerprint(0x1234),
            GameBuild::Fm24Steam
        );
        // Another release's fingerprint never switches releases.
        assert_eq!(
            GameBuild::Fm26Steam.with_fingerprint(epic),
            GameBuild::Fm26Steam
        );
        let gamepass = GameBuild::Fm27GamePass.schemas().known_dll_hash;
        assert_eq!(
            GameBuild::Fm27Steam.with_fingerprint(gamepass),
            GameBuild::Fm27GamePass
        );
        // FM27 Steam's fingerprint is uncalibrated (0): it is never matched.
        assert_eq!(
            GameBuild::Fm27GamePass.with_fingerprint(0),
            GameBuild::Fm27GamePass
        );
    }

    #[test]
    fn every_build_has_its_own_schema() {
        for (i, a) in GameBuild::ALL.iter().enumerate() {
            for b in &GameBuild::ALL[i + 1..] {
                assert!(!std::ptr::eq(a.schemas(), b.schemas()), "{a:?} / {b:?}");
            }
        }
    }
}
