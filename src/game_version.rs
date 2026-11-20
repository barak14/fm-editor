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

/// Where a game install came from, inferred from its install location.
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
    Detected(DetectionOutcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionOutcome {
    /// A recorded build's address set passed the gate.
    MatchesRecorded,
    /// Scanning found every entity array and class identifier.
    Complete,
    /// Scanning found the entity arrays but not every class identifier.
    Partial,
}

/// An attached build's recorded identity and resolved module addresses.
#[derive(Clone, Copy)]
pub struct GameBuild {
    pub recorded: &'static RecordedBuild,
    pub addresses: ModuleAddresses,
    pub recognition: Recognition,
    pub storefront: Storefront,
}

impl PartialEq for GameBuild {
    fn eq(&self, other: &Self) -> bool {
        self.same_recorded_build(*other) && self.recognition == other.recognition
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
    pub fn same_recorded_build(self, other: Self) -> bool {
        std::ptr::eq(self.recorded, other.recorded)
            && self.addresses == other.addresses
            && self.storefront == other.storefront
    }

    pub const FM24_STEAM: Self = Self {
        recorded: &fm24::FM24_STEAM,
        addresses: fm24::FM24_STEAM.addresses,
        recognition: Recognition::Recorded,
        storefront: Storefront::Steam,
    };

    pub const FM24_EPIC: Self = Self {
        recorded: &fm24::FM24_EPIC,
        addresses: fm24::FM24_EPIC.addresses,
        recognition: Recognition::Recorded,
        storefront: Storefront::Epic,
    };

    pub const FM26_STEAM: Self = Self {
        recorded: &fm26::FM26_STEAM,
        addresses: fm26::FM26_STEAM.addresses,
        recognition: Recognition::Recorded,
        storefront: Storefront::Steam,
    };

    pub const FM27_STEAM: Self = Self {
        recorded: &fm27::FM27_STEAM,
        addresses: fm27::FM27_STEAM.addresses,
        recognition: Recognition::Recorded,
        storefront: Storefront::Steam,
    };

    pub const FM27_GAMEPASS: Self = Self {
        recorded: &fm27::FM27_GAMEPASS,
        addresses: fm27::FM27_GAMEPASS.addresses,
        recognition: Recognition::Recorded,
        storefront: Storefront::GamePass,
    };

    pub const FM27_GAMEPASS_27_1: Self = Self {
        recorded: &fm27::FM27_GAMEPASS_27_1,
        addresses: fm27::FM27_GAMEPASS_27_1.addresses,
        recognition: Recognition::Recorded,
        storefront: Storefront::GamePass,
    };

    /// Within a storefront, newer packages come first: they are tried first while
    /// waiting for a save, and `resolve` picks the first.
    pub const ALL: [Self; 6] = [
        Self::FM24_STEAM,
        Self::FM24_EPIC,
        Self::FM26_STEAM,
        Self::FM27_STEAM,
        Self::FM27_GAMEPASS_27_1,
        Self::FM27_GAMEPASS,
    ];

    /// Attached as Game Pass or using a Game Pass build's addresses, whose
    /// club membership and filters are unverified.
    pub fn uses_gamepass_layout(self) -> bool {
        self.storefront == Storefront::GamePass || self.recorded.storefront == Storefront::GamePass
    }

    pub fn candidates(version: GameVersion, storefront: Storefront) -> Vec<&'static RecordedBuild> {
        let mut candidates: Vec<_> = Self::ALL
            .into_iter()
            .filter(|build| build.version() == version)
            .map(|build| build.recorded)
            .collect();
        candidates.sort_by_key(|build| {
            if build.storefront == storefront {
                0
            } else if build.storefront == Storefront::Steam {
                1
            } else {
                2
            }
        });
        candidates
    }

    pub fn resolve(version: GameVersion, storefront: Storefront) -> Result<Self, String> {
        let recorded = Self::candidates(version, storefront)
            .into_iter()
            .next()
            .ok_or_else(|| format!("no recorded layouts for {}", version.display_name()))?;
        if recorded.storefront == storefront {
            Ok(Self {
                recorded,
                addresses: recorded.addresses,
                recognition: Recognition::Recorded,
                storefront,
            })
        } else {
            Ok(Self::detected(recorded, storefront))
        }
    }

    pub fn detected(recorded: &'static RecordedBuild, storefront: Storefront) -> Self {
        Self {
            recorded,
            addresses: recorded.addresses,
            recognition: Recognition::Detected(DetectionOutcome::MatchesRecorded),
            storefront,
        }
    }

    /// A build whose addresses were found by scanning the module. `recorded`
    /// supplies the release layout; its addresses are not used.
    pub fn scanned(
        recorded: &'static RecordedBuild,
        storefront: Storefront,
        addresses: ModuleAddresses,
        complete: bool,
    ) -> Self {
        Self {
            recorded,
            addresses,
            recognition: Recognition::Detected(if complete {
                DetectionOutcome::Complete
            } else {
                DetectionOutcome::Partial
            }),
            storefront,
        }
    }

    pub fn is_detected(self) -> bool {
        matches!(self.recognition, Recognition::Detected(_))
    }

    /// Addresses came from a scan rather than a recorded build. The worker
    /// refuses writes on such builds.
    pub fn is_scanned(self) -> bool {
        matches!(
            self.recognition,
            Recognition::Detected(DetectionOutcome::Complete | DetectionOutcome::Partial)
        )
    }

    /// The recorded build of the same release whose schema pins this module
    /// fingerprint, or `self` when none does. A moved or renamed install
    /// whose path gave the wrong storefront still gets its own schema; an
    /// unknown fingerprint is left to the attachment's fingerprint gate.
    pub fn with_fingerprint(self, hash: u64) -> Self {
        if self.fingerprint() != 0 && self.fingerprint() == hash {
            return Self {
                recognition: Recognition::Recorded,
                storefront: self.recorded.storefront,
                ..self
            };
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
        self.storefront
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
            (GameVersion::Fm24, Storefront::Steam) => "FM24",
            (GameVersion::Fm24, Storefront::Epic) => "FM24 Epic",
            (GameVersion::Fm24, Storefront::GamePass) => "FM24 Game Pass",
            (GameVersion::Fm26, Storefront::Steam) => "FM26",
            (GameVersion::Fm26, Storefront::Epic) => "FM26 Epic",
            (GameVersion::Fm26, Storefront::GamePass) => "FM26 Game Pass",
            (GameVersion::Fm27, Storefront::Steam) => "FM27",
            (GameVersion::Fm27, Storefront::Epic) => "FM27 Epic",
            (GameVersion::Fm27, Storefront::GamePass) => "FM27 Game Pass",
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
                GameBuild::resolve(build.version(), build.storefront())
                    .map(|resolved| resolved.with_fingerprint(build.fingerprint())),
                Ok(build)
            );
        }
    }

    #[test]
    fn resolve_detects_unrecorded_storefronts() {
        for (version, storefront) in [
            (GameVersion::Fm24, Storefront::GamePass),
            (GameVersion::Fm26, Storefront::GamePass),
            (GameVersion::Fm26, Storefront::Epic),
            (GameVersion::Fm27, Storefront::Epic),
        ] {
            let build = GameBuild::resolve(version, storefront).unwrap();
            assert_eq!(build.version(), version);
            assert_eq!(build.storefront(), storefront);
            assert!(build.is_detected());
            assert_eq!(build.recorded.storefront, Storefront::Steam);
        }
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
        // Unknown fingerprints retain the candidate for runtime validation.
        assert_eq!(
            GameBuild::FM24_STEAM.with_fingerprint(0x1234),
            GameBuild::FM24_STEAM
        );
        // Another release's fingerprint never switches releases.
        assert_eq!(
            GameBuild::FM26_STEAM.with_fingerprint(epic),
            GameBuild::FM26_STEAM
        );
        for gamepass in [GameBuild::FM27_GAMEPASS, GameBuild::FM27_GAMEPASS_27_1] {
            assert_eq!(
                GameBuild::FM27_STEAM.with_fingerprint(gamepass.fingerprint()),
                gamepass
            );
            // FM27 Steam's fingerprint is uncalibrated (0): it is never matched.
            assert_eq!(gamepass.with_fingerprint(0), gamepass);
        }
        assert_eq!(
            GameBuild::FM27_GAMEPASS
                .with_fingerprint(GameBuild::FM27_GAMEPASS_27_1.fingerprint()),
            GameBuild::FM27_GAMEPASS_27_1
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

    #[test]
    fn recorded_build_identity_ignores_only_recognition() {
        let recorded = GameBuild::FM27_STEAM;
        let detected = GameBuild::detected(recorded.recorded, recorded.storefront());
        assert_ne!(recorded, detected);
        assert!(detected.same_recorded_build(recorded));
        assert!(recorded.same_recorded_build(detected));
        for other in GameBuild::ALL {
            assert_eq!(detected.same_recorded_build(other), other == recorded);
        }
        let mut altered = detected;
        altered.addresses = GameBuild::FM27_GAMEPASS.addresses();
        assert!(!altered.same_recorded_build(recorded));
        altered = detected;
        altered.storefront = Storefront::GamePass;
        assert!(!altered.same_recorded_build(recorded));
    }

    #[test]
    fn candidate_order_prefers_observed_then_steam() {
        for version in [GameVersion::Fm24, GameVersion::Fm26, GameVersion::Fm27] {
            for storefront in [Storefront::Steam, Storefront::Epic, Storefront::GamePass] {
                let candidates = GameBuild::candidates(version, storefront);
                let priority = |build: &&RecordedBuild| {
                    if build.storefront == storefront {
                        0
                    } else if build.storefront == Storefront::Steam {
                        1
                    } else {
                        2
                    }
                };
                assert!(
                    candidates
                        .windows(2)
                        .all(|pair| priority(&pair[0]) <= priority(&pair[1]))
                );
                assert_eq!(
                    candidates.len(),
                    GameBuild::ALL
                        .iter()
                        .filter(|b| b.version() == version)
                        .count()
                );
                assert!(candidates.iter().all(|build| build.version == version));
            }
        }
    }

    #[test]
    fn display_names_cover_every_release_and_storefront() {
        for (version, names) in [
            (GameVersion::Fm24, ["FM24", "FM24 Epic", "FM24 Game Pass"]),
            (GameVersion::Fm26, ["FM26", "FM26 Epic", "FM26 Game Pass"]),
            (GameVersion::Fm27, ["FM27", "FM27 Epic", "FM27 Game Pass"]),
        ] {
            for (storefront, name) in [Storefront::Steam, Storefront::Epic, Storefront::GamePass]
                .into_iter()
                .zip(names)
            {
                assert_eq!(
                    GameBuild::resolve(version, storefront)
                        .unwrap()
                        .display_name(),
                    name
                );
            }
        }
    }

    #[test]
    fn detected_builds_never_equal_recorded_builds() {
        for recorded in GameBuild::ALL {
            let detected = GameBuild::detected(recorded.recorded, recorded.storefront());
            assert_ne!(detected, recorded);
            assert!(detected.is_detected());
            assert!(!recorded.is_detected());
            let mut other_storefront = detected;
            other_storefront.storefront = if detected.storefront == Storefront::Epic {
                Storefront::Steam
            } else {
                Storefront::Epic
            };
            assert_ne!(detected, other_storefront);
        }
    }
}
