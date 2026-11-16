//! Release layouts and recorded executable identities, selected per attachment.

use crate::fields::{ModuleAddresses, RecordedBuild, ReleaseLayout, fm24, fm26, fm27};

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

/// How the attached executable was recognised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recognition {
    Recorded,
}

/// An attached build's recorded identity and resolved module addresses.
#[derive(Clone, Copy)]
pub struct GameBuild {
    pub recorded: &'static RecordedBuild,
    pub addresses: ModuleAddresses,
    pub recognition: Recognition,
}

impl PartialEq for GameBuild {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.recorded, other.recorded)
            && self.addresses == other.addresses
            && self.recognition == other.recognition
    }
}

impl Eq for GameBuild {}

impl std::fmt::Debug for GameBuild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameBuild")
            .field("display_name", &self.display_name())
            .field("recognition", &self.recognition)
            .field("addresses", &self.addresses)
            .finish()
    }
}

impl GameBuild {
    pub const FM24_STEAM: Self = Self {
        recorded: &fm24::FM24_STEAM,
        addresses: fm24::FM24_STEAM.addresses,
        recognition: Recognition::Recorded,
    };

    pub const FM24_EPIC: Self = Self {
        recorded: &fm24::FM24_EPIC,
        addresses: fm24::FM24_EPIC.addresses,
        recognition: Recognition::Recorded,
    };

    pub const FM26_STEAM: Self = Self {
        recorded: &fm26::FM26_STEAM,
        addresses: fm26::FM26_STEAM.addresses,
        recognition: Recognition::Recorded,
    };

    pub const FM27_STEAM: Self = Self {
        recorded: &fm27::FM27_STEAM,
        addresses: fm27::FM27_STEAM.addresses,
        recognition: Recognition::Recorded,
    };

    pub const FM27_GAMEPASS: Self = Self {
        recorded: &fm27::FM27_GAMEPASS,
        addresses: fm27::FM27_GAMEPASS.addresses,
        recognition: Recognition::Recorded,
    };

    pub const ALL: [Self; 5] = [
        Self::FM24_STEAM,
        Self::FM24_EPIC,
        Self::FM26_STEAM,
        Self::FM27_STEAM,
        Self::FM27_GAMEPASS,
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
        if self.fingerprint() == hash {
            return self;
        }
        Self::ALL
            .into_iter()
            .find(|b| {
                let known = b.fingerprint();
                b.version() == self.version() && known != 0 && known == hash
            })
            .unwrap_or(self)
    }

    pub fn version(self) -> GameVersion {
        self.recorded.version
    }

    pub fn storefront(self) -> Storefront {
        self.recorded.storefront
    }

    pub fn schemas(self) -> &'static ReleaseLayout {
        self.recorded.layout
    }

    pub fn addresses(self) -> ModuleAddresses {
        self.addresses
    }

    pub fn fingerprint(self) -> u64 {
        self.recorded.fingerprint
    }

    pub fn schema_name(self) -> &'static str {
        self.recorded.schema_name
    }

    pub fn display_name(self) -> &'static str {
        match (self.version(), self.storefront()) {
            (GameVersion::Fm24, Storefront::Epic) => "FM24 Epic",
            (GameVersion::Fm27, Storefront::GamePass) => "FM27 Game Pass",
            (version, _) => version.display_name(),
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
            Ok(GameBuild::FM24_EPIC)
        );
    }

    #[test]
    fn fingerprint_selects_the_recorded_build_of_the_same_release() {
        let epic = GameBuild::FM24_EPIC.fingerprint();
        let steam = GameBuild::FM24_STEAM.fingerprint();
        assert_eq!(
            GameBuild::FM24_STEAM.with_fingerprint(epic),
            GameBuild::FM24_EPIC
        );
        assert_eq!(
            GameBuild::FM24_EPIC.with_fingerprint(steam),
            GameBuild::FM24_STEAM
        );
        assert_eq!(
            GameBuild::FM24_EPIC.with_fingerprint(epic),
            GameBuild::FM24_EPIC
        );
        // An unknown fingerprint keeps the path's build for the gate to refuse.
        assert_eq!(
            GameBuild::FM24_STEAM.with_fingerprint(0x1234),
            GameBuild::FM24_STEAM
        );
        // Another release's fingerprint never switches releases.
        assert_eq!(
            GameBuild::FM26_STEAM.with_fingerprint(epic),
            GameBuild::FM26_STEAM
        );
        let gamepass = GameBuild::FM27_GAMEPASS.fingerprint();
        assert_eq!(
            GameBuild::FM27_STEAM.with_fingerprint(gamepass),
            GameBuild::FM27_GAMEPASS
        );
        // FM27 Steam's fingerprint is uncalibrated (0): it is never matched.
        assert_eq!(
            GameBuild::FM27_GAMEPASS.with_fingerprint(0),
            GameBuild::FM27_GAMEPASS
        );
    }

    #[test]
    fn storefronts_share_release_layouts_but_not_recorded_metadata() {
        for (i, a) in GameBuild::ALL.iter().enumerate() {
            for b in &GameBuild::ALL[i + 1..] {
                assert_eq!(
                    std::ptr::eq(a.schemas(), b.schemas()),
                    a.version() == b.version(),
                    "{a:?} / {b:?}",
                );
                assert!(!std::ptr::eq(a.recorded, b.recorded));
                assert_ne!(
                    (a.addresses(), a.fingerprint()),
                    (b.addresses(), b.fingerprint()),
                    "{a:?} / {b:?}",
                );
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn altered_addresses_change_build_identity() {
        let original = GameBuild::FM27_STEAM;
        let mut altered = original;
        altered.addresses = GameBuild::FM27_GAMEPASS.addresses();
        assert_ne!(original, altered);
        assert!(std::ptr::eq(original.recorded, altered.recorded));
        assert_eq!(original, original);
    }
}
