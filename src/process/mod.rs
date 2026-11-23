//! Attach to fm.exe and read/write its memory.
//!
//! Platform backends implement the raw primitives — pid lookup, module
//! enumeration, cross-process byte transfer. Everything above raw bytes
//! (version sniffing, fingerprint pinning, typed reads, FM name decoding)
//! is shared and platform-independent.
//!
//! On Linux the target is the same Windows fm.exe running under
//! Proton/Wine, so every schema offset is identical to a native Windows
//! install; only the process-access primitives differ.

mod detection_log;
pub mod layout_detect;
pub mod layout_validate;
pub mod rtti;
mod version_resource;

#[cfg(windows)]
mod win32;
#[cfg(windows)]
use self::win32 as backend;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use self::linux as backend;

use self::layout_validate::GateReport;
use crate::fields::{ModuleAddresses, PeIdentity, ReleaseLayout};
use crate::game_date::GameDate;
use crate::game_version::{GameBuild, GameVersion, Storefront};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionStatus {
    Pinned,
    AwaitingSave,
    Validated,
    Failed(String),
}

/// The latest module scan for an attachment whose recorded addresses failed.
pub struct ScanRecord {
    pub at: std::time::Instant,
    /// Build with the found addresses, or why the scan failed.
    pub result: Result<GameBuild, String>,
    pub missing: Vec<&'static str>,
    pub notes: Vec<String>,
}

impl ScanRecord {
    pub fn summary(&self) -> String {
        let mut lines = vec![match &self.result {
            Ok(build) => {
                let a = build.addresses;
                format!(
                    "scan: humans 0x{:X}/0x{:X}, staff 0x{:X}/0x{:X}, clubs 0x{:X}/0x{:X}, stadium 0x{:X}, date 0x{:X}, finance vtable 0x{:X}, player-coach vtable 0x{:X}, human-manager vtable 0x{:X}; missing: {}",
                    a.rva_humans_data,
                    a.rva_humans_end,
                    a.rva_staff_data,
                    a.rva_staff_end,
                    a.rva_clubs_data,
                    a.rva_clubs_end,
                    a.rva_stadium_base,
                    a.rva_current_date,
                    a.full_finance_vtable_rva,
                    a.player_coach_vtable_rva,
                    a.human_manager_vtable_rva,
                    if self.missing.is_empty() {
                        "none".to_owned()
                    } else {
                        self.missing.join(", ")
                    },
                )
            }
            Err(error) => format!("scan failed: {error}"),
        }];
        lines.extend(self.notes.iter().map(|note| format!("  {note}")));
        lines.join("\n")
    }
}

pub struct DetectionReport {
    pub release: GameVersion,
    pub storefront: Storefront,
    pub module_hash: Option<u64>,
    pub pe_identity: Option<PeIdentity>,
    pub pinned: bool,
    /// Debug-only `FME_FORCE_DETECT=1`: skip recorded address sets and scan.
    pub force_scan: bool,
    pub candidates: Vec<&'static str>,
    pub chosen: Option<&'static str>,
    pub status: DetectionStatus,
    pub gate: Option<GateReport>,
    pub scan: Option<ScanRecord>,
}

impl DetectionReport {
    pub fn summary(&self) -> String {
        let hash = self
            .module_hash
            .map_or_else(|| "unavailable".into(), |hash| format!("0x{hash:016X}"));
        let pe = self.pe_identity.map_or_else(
            || "unavailable".into(),
            |pe| format!("0x{:08X}/0x{:08X}", pe.timestamp, pe.size_of_image),
        );
        let mut summary = format!(
            "{} {}; module hash: {}; PE identity: {}; pinned: {}; candidates: {}; chosen: {}; status: {:?}",
            self.release.display_name(),
            self.storefront.display_name(),
            hash,
            pe,
            self.pinned,
            self.candidates.join(", "),
            self.chosen.unwrap_or("none"),
            self.status,
        );
        if let Some(scan) = &self.scan {
            summary.push('\n');
            summary.push_str(&scan.summary());
        }
        if let Some(gate) = &self.gate {
            summary.push('\n');
            summary.push_str(&gate.summary());
        }
        summary
    }
}

fn attachment_build(
    path_build: GameBuild,
    module_hash: Option<u64>,
    pe_identity: Option<PeIdentity>,
    ignore_fingerprint: bool,
) -> (GameBuild, DetectionReport) {
    let matched = if ignore_fingerprint {
        None
    } else {
        path_build.pinned_by(module_hash, pe_identity)
    };
    let pinned = matched.is_some();
    let storefront = matched.unwrap_or(path_build).storefront();
    let candidates = GameBuild::candidates(path_build.version(), storefront);
    let build = matched.unwrap_or_else(|| GameBuild::detected(candidates[0], storefront));
    let detection = DetectionReport {
        release: build.version(),
        storefront,
        module_hash,
        pe_identity,
        pinned,
        force_scan: false,
        candidates: candidates
            .iter()
            .map(|candidate| candidate.schema_name)
            .collect(),
        chosen: pinned.then_some(build.schema_name()),
        status: if pinned {
            DetectionStatus::Pinned
        } else {
            DetectionStatus::AwaitingSave
        },
        gate: None,
        scan: None,
    };
    (build, detection)
}

/// A running `fm.exe` whose game build could be identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunningGame {
    pub pid: u32,
    pub build: GameBuild,
}

impl std::fmt::Display for RunningGame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (PID {})", self.build.display_name(), self.pid)
    }
}

/// Every running `fm.exe` whose build can be determined, ordered by PID.
///
/// FM24, FM26 and FM27 all run as `fm.exe`, so several can be open at once.
/// A process whose build cannot be sniffed (e.g. a launcher sharing the
/// name, or a storefront without a schema) is left out; when nothing is
/// identifiable the error says why.
pub fn running_games() -> Result<Vec<RunningGame>, String> {
    let pids = backend::find_pids("fm.exe")?;
    if pids.is_empty() {
        return Err("fm.exe is not running".to_string());
    }
    let mut games = Vec::with_capacity(pids.len());
    let mut errors = Vec::new();
    for pid in pids {
        match detect_build(pid) {
            Ok(build) => games.push(RunningGame { pid, build }),
            Err(e) => {
                errors.push(format!("pid {pid}: {e}"));
            }
        }
    }
    if games.is_empty() {
        Err(errors.join("\n\n"))
    } else {
        Ok(games)
    }
}

/// Identify the build of one `fm.exe`.
///
/// Read the game module's PE product metadata first: FM24 embeds it in
/// `fm.exe`; FM26/FM27 use `game_plugin.dll` (their exe describes Unity).
/// The install directory is only a fallback when no FM identity is available.
/// Storefront detection is separate and does not establish layout compatibility.
fn detect_build(pid: u32) -> Result<GameBuild, String> {
    let (result, report) = detect_build_from_modules(
        pid,
        backend::find_module_info(pid, "fm.exe"),
        backend::find_module_info(pid, "game_plugin.dll"),
        version_resource::read,
    );
    detection_log::record(pid, &report);
    result.map_err(|e| format!("{e}\n{report}"))
}

struct InspectedModule {
    name: &'static str,
    path: String,
    metadata: Result<version_resource::VersionMetadata, String>,
}

fn inspect_module(
    name: &'static str,
    info: Result<(u64, u64, String), String>,
    read_metadata: &mut impl FnMut(&str) -> Result<version_resource::VersionMetadata, String>,
    report: &mut String,
) -> Option<InspectedModule> {
    use std::fmt::Write;
    match info {
        Ok((base, size, path)) => {
            let _ = writeln!(
                report,
                "module {name}: path={path:?}, base=0x{base:X}, size=0x{size:X}"
            );
            let metadata = read_metadata(&path);
            match &metadata {
                Ok(info) => {
                    let _ = writeln!(report, "  ProductName: {:?}", info.product_names);
                    let _ = writeln!(report, "  ProductVersion: {:?}", info.product_versions);
                    let _ = writeln!(report, "  FileVersion: {:?}", info.file_versions);
                }
                Err(e) => {
                    let _ = writeln!(report, "  PE version resource unavailable: {e}");
                }
            }
            Some(InspectedModule {
                name,
                path,
                metadata,
            })
        }
        Err(e) => {
            let _ = writeln!(report, "module {name}: discovery failed for this PID: {e}");
            None
        }
    }
}

/// Keep discovery, file metadata and selection in one process-local report.
/// Dependency injection lets tests prove that a missing plugin never borrows
/// another process's path or metadata, without starting or attaching to FM.
fn detect_build_from_modules(
    pid: u32,
    exe_info: Result<(u64, u64, String), String>,
    plugin_info: Result<(u64, u64, String), String>,
    mut read_metadata: impl FnMut(&str) -> Result<version_resource::VersionMetadata, String>,
) -> (Result<GameBuild, String>, String) {
    use std::fmt::Write;
    let mut report = format!("FM build detection: PID {pid}\n");
    let exe = inspect_module("fm.exe", exe_info, &mut read_metadata, &mut report);
    let plugin = inspect_module(
        "game_plugin.dll (optional)",
        plugin_info,
        &mut read_metadata,
        &mut report,
    );
    let result = (|| {
        let selected = plugin.as_ref().or(exe.as_ref()).ok_or_else(|| {
            "cannot identify game: neither game module nor executable could be located".to_string()
        })?;
        let _ = writeln!(
            report,
            "selected module: {} ({:?})",
            selected.name, selected.path
        );
        let storefront_path = exe.as_ref().map_or(&selected.path, |exe| &exe.path);
        let gdk = has_gdk_config(storefront_path);
        let storefront = storefront_of(storefront_path, gdk);
        let _ = writeln!(
            report,
            "storefront hint: {}; path={storefront_path:?}; MicrosoftGame.config present={gdk}",
            storefront.display_name()
        );
        let identity = selected.metadata.as_ref().ok().map(version_from_metadata);
        let source = if matches!(identity, Some(Ok(Some(_)))) {
            "PE ProductName"
        } else {
            let _ = writeln!(
                report,
                "PE identity unavailable or unsupported: {identity:?}; resource error: {:?}",
                selected.metadata.as_ref().err()
            );
            "install-directory fallback"
        };
        let version = identify_version(&selected.path, selected.metadata.clone())?;
        let _ = writeln!(
            report,
            "release: {}; source: {source}",
            version.display_name()
        );
        GameBuild::resolve(version, storefront)
    })();
    match &result {
        Ok(build) => {
            let _ = writeln!(
                report,
                "result: selected {} schema {}; layout compatibility still requires attachment validation",
                build.display_name(),
                build.schema_name()
            );
        }
        Err(e) => {
            let _ = writeln!(report, "result: {e}");
        }
    }
    (result, report)
}

/// True when a `MicrosoftGame.config` (the Microsoft GDK package manifest
/// every Game Pass PC title ships) sits next to the exe.
fn has_gdk_config(exe_path: &str) -> bool {
    let dir = match exe_path.rfind(['/', '\\']) {
        Some(i) => &exe_path[..i],
        None => return false,
    };
    std::path::Path::new(dir)
        .join("MicrosoftGame.config")
        .is_file()
}

/// Storefront of an install, from its exe (or module) path and whether the
/// GDK manifest sits next to the exe. Game Pass installs live under an
/// `XboxGames` folder (any drive, the Xbox app default) or, for older Store
/// packages, `Program Files\WindowsApps`. Epic installs live under
/// `Epic Games`. Other locations retain the historical Steam default;
/// metadata identifies the release, not the storefront or compatible offsets.
/// The attachment's module identity corrects a wrong hint between recorded builds of one release.
fn storefront_of(path: &str, has_gdk_config: bool) -> Storefront {
    let lower = path.to_lowercase().replace('\\', "/");
    if has_gdk_config || lower.contains("/xboxgames/") || lower.contains("/windowsapps/") {
        Storefront::GamePass
    } else if lower.contains("/epic games/") {
        Storefront::Epic
    } else {
        Storefront::Steam
    }
}

/// Handle to an open game process with its resolved game module and
/// selected game build.
pub struct FmProcess {
    handle: backend::ProcHandle,
    pub pid: u32,
    /// Base of the module that holds the game data RVAs:
    /// `game_plugin.dll` on FM26/FM27, `fm.exe` itself on FM24
    /// (see `GameVersion::module_name`).
    pub gp_base: u64,
    pub gp_end: u64,
    /// On-disk path of that module, as the OS reports it.
    pub module_path: String,
    pub build: GameBuild,
    pub detection: DetectionReport,
    rtti: std::sync::OnceLock<rtti::Index>,
}

impl FmProcess {
    /// Attach to the only running game.
    ///
    /// Fails when no game runs, and when several do (FM24, FM26 and FM27
    /// all run as `fm.exe`): the error lists them so the caller can pick
    /// one with [`FmProcess::attach_pid`].
    pub fn attach() -> Result<Self, String> {
        let games = running_games()?;
        match games.as_slice() {
            [game] => Self::attach_pid(game.pid),
            _ => Err(format!(
                "{} games are running: {}. Choose one by PID.",
                games.len(),
                games
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
        }
    }

    /// Attach to the game with this PID. The schema comes from that
    /// process's own install path, never from any other attachment.
    pub fn attach_pid(pid: u32) -> Result<Self, String> {
        let handle = backend::open(pid)?;
        let path_build = detect_build(pid)?;
        let module = path_build.version().module_name();
        let (gp_base, gp_size, gp_path) = backend::find_module_info(pid, module)?;
        // Probe one read up front so a blocked cross-process access path
        // (e.g. Linux yama ptrace_scope) fails the attach with the OS hint
        // instead of erroring on the first field read later.
        backend::read_bytes(&handle, gp_base, 2)
            .map_err(|e| format!("attached to pid {pid} but cannot read its memory: {e}"))?;
        let module_header = read_module_header(&gp_path);
        let header = module_header.as_deref().ok();
        let debug_flag = |name: &str| {
            cfg!(debug_assertions) && std::env::var(name).is_ok_and(|value| value == "1")
        };
        let force_scan = debug_flag("FME_FORCE_DETECT");
        let ignore_fingerprint = force_scan || debug_flag("FME_IGNORE_FINGERPRINT");
        let (build, mut detection) = attachment_build(
            path_build,
            header.map(hash_module_header),
            header.and_then(parse_pe_identity),
            ignore_fingerprint,
        );
        detection.force_scan = force_scan;
        let mut report = detection.summary();
        if let Err(error) = module_header {
            report.push_str(&format!("\nmodule header unavailable: {error}"));
        }
        detection_log::record(pid, &report);
        Ok(Self {
            handle,
            pid,
            gp_base,
            gp_end: gp_base + gp_size,
            module_path: gp_path,
            build,
            detection,
            rtti: std::sync::OnceLock::new(),
        })
    }

    /// Append the current detection report and `note` to the detection log.
    pub fn log_detection_note(&self, note: &str) {
        detection_log::record(self.pid, &format!("{}\n{note}", self.detection.summary()));
    }

    /// The release's record layout for this process.
    pub fn schemas(&self) -> &'static ReleaseLayout {
        self.build.schemas()
    }

    pub fn addresses(&self) -> ModuleAddresses {
        self.build.addresses
    }

    /// Observed on-disk module header fingerprint, using the attachment gate's
    /// algorithm. Reporting it does not calibrate or override that gate.
    pub fn module_header_hash(&self) -> Result<u64, String> {
        read_module_header(&self.module_path).map(|bytes| hash_module_header(&bytes))
    }

    /// Read the current in-game date through this attachment's verified schema.
    /// Unmapped builds, invalid pointers/dates, and failed reads return None.
    pub fn game_date(&self) -> Option<GameDate> {
        let rva = self.addresses().rva_current_date;
        if rva == 0 {
            return None;
        }
        let valid = |address: u64| address > 0x10000 && address < 0x7FFF_FFFF_FFFF;
        let address = self.gp_base.checked_add(rva)?;
        if !valid(address) || !valid(address.checked_add(3)?) {
            return None;
        }
        let bytes = self.read_bytes(address, 4).ok()?;
        GameDate::from_bytes(&bytes)
    }

    /// Read `n` bytes from the target process at `addr`.
    pub fn read_bytes(&self, addr: u64, n: usize) -> Result<Vec<u8>, String> {
        backend::read_bytes(&self.handle, addr, n)
    }

    /// Read a u64 (little-endian) from the target process.
    pub fn read_u64(&self, addr: u64) -> Result<u64, String> {
        let buf = self.read_bytes(addr, 8)?;
        Ok(u64::from_le_bytes(buf.try_into().unwrap()))
    }

    /// Read a u32 (little-endian) from the target process.
    pub fn read_u32(&self, addr: u64) -> Result<u32, String> {
        let buf = self.read_bytes(addr, 4)?;
        Ok(u32::from_le_bytes(buf.try_into().unwrap()))
    }

    /// Read a `std::vector<T*>`-style `(data, end)` pointer pair stored at
    /// `gp_base + data_rva` / `gp_base + end_rva` and return
    /// `(data, end, count)`. Rejects bounds that are not valid user-space
    /// pointers, run backwards, or are not a whole number of 8-byte slots —
    /// the usual symptom of an RVA that a game patch has moved.
    pub fn read_ptr_array(&self, data_rva: u64, end_rva: u64) -> Result<(u64, u64, usize), String> {
        let data = self.read_u64(self.gp_base + data_rva)?;
        let end = self.read_u64(self.gp_base + end_rva)?;
        let valid = |p: u64| p > 0x10000 && p < 0x7FFF_FFFF_FFFF;
        if !valid(data) || !valid(end) || end < data || (end - data) % 8 != 0 {
            return Err(format!(
                "bad pointer array at gp+0x{data_rva:X}: data=0x{data:X} end=0x{end:X} \
                 (RVA moved by a game patch? run Tools/scan-arrays.sh)"
            ));
        }
        Ok((data, end, ((end - data) / 8) as usize))
    }

    /// Read a u8 from the target process.
    pub fn read_u8(&self, addr: u64) -> Result<u8, String> {
        let buf = self.read_bytes(addr, 1)?;
        Ok(buf[0])
    }

    /// Write bytes to the target process at `addr`.
    pub fn write_bytes(&self, addr: u64, data: &[u8]) -> Result<(), String> {
        backend::write_bytes(&self.handle, addr, data)
    }

    /// Write a single u8 to the target process.
    pub fn write_u8(&self, addr: u64, val: u8) -> Result<(), String> {
        self.write_bytes(addr, &[val])
    }

    /// Read an FM name string: pointer → [u32 len][UTF-8 bytes].
    ///
    /// Reads the length prefix + name bytes in a single cross-process call
    /// (up to 260 bytes) instead of two separate calls, cutting round trips.
    pub fn read_fm_name(&self, name_ptr: u64) -> Result<String, String> {
        if name_ptr < 0x10000 || name_ptr > 0x7FFFFFFFFFFF {
            return Err("invalid name pointer".into());
        }
        // Single read: 4-byte length prefix + up to 256 bytes of name data.
        let buf = self.read_bytes(name_ptr, 4 + 256)?;
        let len = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
        if len == 0 || len > 256 {
            return Err(format!("bad name length: {len}"));
        }
        String::from_utf8(buf[4..4 + len].to_vec()).map_err(|e| format!("name UTF-8 error: {e}"))
    }
}

/// Recognise an FM product name, without interpreting engine version numbers.
/// An explicit unsupported or conflicting FM identity must not fall back to a
/// directory that happens to name a supported release.
fn version_from_metadata(
    metadata: &version_resource::VersionMetadata,
) -> Result<Option<GameVersion>, String> {
    let mut found = None;
    for name in &metadata.product_names {
        let lower = name.trim().to_ascii_lowercase();
        let Some(release) = lower.strip_prefix("football manager ") else {
            continue;
        };
        let version = match release {
            "24" | "2024" => GameVersion::Fm24,
            "26" | "2026" => GameVersion::Fm26,
            "27" | "2027" => GameVersion::Fm27,
            _ => return Err(format!("unsupported Football Manager product: {name}")),
        };
        if found.is_some_and(|previous| previous != version) {
            return Err("conflicting Football Manager product names in version resource".into());
        }
        found = Some(version);
    }
    Ok(found)
}

fn identify_version(
    module_path: &str,
    metadata: Result<version_resource::VersionMetadata, String>,
) -> Result<GameVersion, String> {
    let reason = match metadata {
        Ok(metadata) => match version_from_metadata(&metadata) {
            Ok(Some(version)) => return Ok(version),
            Ok(None) => format!(
                "no Football Manager product identity (product names: {:?}, product versions: {:?}, file versions: {:?})",
                metadata.product_names, metadata.product_versions, metadata.file_versions,
            ),
            Err(e) => return Err(format!("{e} ({module_path})")),
        },
        Err(e) => e,
    };
    sniff_version(module_path).map_err(|e| format!("{e}; version resource: {reason}"))
}

/// Legacy fallback for missing product metadata. Match an installation-directory
/// component, not an arbitrary year elsewhere in the path. Compact Epic-style
/// names and the spaced Steam names are both accepted.
fn sniff_version(module_path: &str) -> Result<GameVersion, String> {
    for component in module_path.split(['/', '\\']) {
        let compact: String = component
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
            .flat_map(char::to_lowercase)
            .collect();
        match compact.as_str() {
            "footballmanager24" | "footballmanager2024" => return Ok(GameVersion::Fm24),
            "footballmanager26" | "footballmanager2026" => return Ok(GameVersion::Fm26),
            "footballmanager27" | "footballmanager2027" => return Ok(GameVersion::Fm27),
            _ => {}
        }
    }
    Err(format!(
        "cannot determine game version from module path: {module_path}"
    ))
}

fn read_module_header(path: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::with_capacity(65536);
    std::fs::File::open(path)
        .and_then(|f| f.take(65536).read_to_end(&mut bytes))
        .map_err(|e| format!("cannot read {path}: {e}"))?;
    Ok(bytes)
}

/// Project fingerprint of the first 64 KiB of a module file.
/// The historical multiplier differs from standard FNV-1a.
fn hash_module_header(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000000001b3);
    }
    h
}

fn parse_pe_identity(bytes: &[u8]) -> Option<PeIdentity> {
    fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(offset..offset + 4)?.try_into().ok()?,
        ))
    }

    if !bytes.starts_with(b"MZ") {
        return None;
    }
    let pe_offset = usize::try_from(read_u32(bytes, 0x3C)?).ok()?;
    if pe_offset < 0x40 {
        return None;
    }
    let coff = bytes.get(pe_offset..)?.strip_prefix(b"PE\0\0")?;
    let timestamp = read_u32(coff, 4)?;
    let optional_size = u16::from_le_bytes(coff.get(16..18)?.try_into().ok()?) as usize;
    let optional = coff.get(20..)?.get(..optional_size)?;
    let magic = u16::from_le_bytes(optional.get(..2)?.try_into().ok()?);
    if !matches!(magic, 0x10B | 0x20B) {
        return None;
    }
    let size_of_image = read_u32(optional, 56)?;
    let pe = PeIdentity {
        timestamp,
        size_of_image,
    };
    pe.is_valid().then_some(pe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_pin_only_calibrated_builds() {
        for recorded in GameBuild::ALL {
            let (build, report) =
                attachment_build(recorded, Some(recorded.fingerprint()), None, false);
            if recorded.fingerprint() == 0 {
                assert!(build.is_detected());
                assert_eq!(report.status, DetectionStatus::AwaitingSave);
            } else {
                assert_eq!(build, recorded);
                assert!(report.pinned);
                assert_eq!(report.status, DetectionStatus::Pinned);
                assert_eq!(report.chosen, Some(recorded.schema_name()));
            }
        }
    }

    #[test]
    fn unpinned_builds_await_validation() {
        for hash in [None, Some(0x1234)] {
            let (build, report) = attachment_build(GameBuild::FM24_EPIC, hash, None, false);
            assert!(build.is_detected());
            assert_eq!(build.storefront(), Storefront::Epic);
            assert_eq!(build.recorded.schema_name, "FM24_EPIC");
            assert!(!report.pinned);
            assert_eq!(report.chosen, None);
            assert_eq!(report.status, DetectionStatus::AwaitingSave);
            assert_eq!(report.module_hash, hash);
        }
    }

    #[test]
    fn fingerprints_correct_storefront_unless_forced_to_detect() {
        let steam = GameBuild::FM24_STEAM;
        let epic = GameBuild::FM24_EPIC;
        let (build, report) = attachment_build(steam, Some(epic.fingerprint()), None, false);
        assert_eq!(build, epic);
        assert_eq!(report.storefront, Storefront::Epic);
        let (build, report) = attachment_build(steam, Some(epic.fingerprint()), None, true);
        assert!(build.is_detected());
        assert_eq!(build.storefront(), Storefront::Steam);
        assert_eq!(report.status, DetectionStatus::AwaitingSave);
        assert!(!report.pinned);
    }

    #[test]
    fn detection_summary_includes_hash_candidates_and_failure() {
        let pe = PeIdentity {
            timestamp: 0x12,
            size_of_image: 0x34,
        };
        let (_, mut report) =
            attachment_build(GameBuild::FM24_STEAM, Some(0x1234), Some(pe), false);
        report.status = DetectionStatus::Failed("players failed validation".into());
        let summary = report.summary();
        for expected in [
            "0x0000000000001234",
            "PE identity: 0x00000012/0x00000034",
            "FM24_STEAM",
            "FM24_EPIC",
            "players failed validation",
            "chosen: none",
        ] {
            assert!(summary.contains(expected), "{summary}");
        }
    }

    #[test]
    fn pe_identity_pins_an_imported_build_unless_forced_to_detect() {
        let path = GameBuild::FM27_GAMEPASS_27_1;
        let imported = GameBuild::FM27_STEAM_27_0_1;
        for &pe in imported.recorded.pe_identities {
            let (build, report) = attachment_build(path, Some(0x1234), Some(pe), false);
            assert_eq!(build, imported);
            assert_eq!(report.storefront, Storefront::Steam);
            assert_eq!(report.status, DetectionStatus::Pinned);
            assert_eq!(report.chosen, Some(imported.schema_name()));
            assert_eq!(report.pe_identity, Some(pe));
            assert!(report.pinned);

            let (build, report) = attachment_build(path, Some(0x1234), Some(pe), true);
            assert!(build.is_detected());
            assert_eq!(report.storefront, Storefront::GamePass);
            assert_eq!(report.status, DetectionStatus::AwaitingSave);
            assert!(!report.pinned);

            let (build, report) = attachment_build(GameBuild::FM26_STEAM, None, Some(pe), false);
            assert_eq!(build.version(), GameVersion::Fm26);
            assert!(!report.pinned);
        }
    }

    #[test]
    fn fm27_epic_is_selected_by_path_and_pinned_by_pe_identity() {
        let path = r"C:\Program Files\Epic Games\FootballManager27";
        let exe = format!(r"{path}\fm.exe");
        let plugin = format!(r"{path}\fm_Data\Plugins\x86_64\game_plugin.dll");
        let epic = GameBuild::FM27_EPIC_27_1;
        let (selected, _) =
            detect_build_from_modules(1, module_info(&exe), module_info(&plugin), |_| {
                Err("resource missing".into())
            });
        assert_eq!(selected, Ok(epic));
        let steam = GameBuild::FM27_STEAM_27_1;
        let pe = PeIdentity {
            timestamp: 0x6AF5C793,
            size_of_image: 0x18095000,
        };
        let (build, report) = attachment_build(steam, None, Some(pe), false);
        assert_eq!(build, epic);
        assert_eq!(report.status, DetectionStatus::Pinned);
    }

    #[test]
    fn zero_and_unknown_pe_identities_do_not_pin() {
        let path = GameBuild::FM27_STEAM_27_1;
        let known = path.recorded.pe_identities[0];
        for pe in [
            PeIdentity {
                timestamp: 0,
                ..known
            },
            PeIdentity {
                size_of_image: 0,
                ..known
            },
            PeIdentity {
                size_of_image: known.size_of_image + 0x1000,
                ..known
            },
        ] {
            let (build, report) = attachment_build(path, Some(0), Some(pe), false);
            assert!(build.is_detected());
            assert!(!report.pinned);
        }
    }

    #[test]
    fn pe_identity_parser_checks_header_signature_size_and_magic() {
        let mut bytes = [0u8; 0x148];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3C..0x40].copy_from_slice(&0x40_u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes[0x48..0x4C].copy_from_slice(&0x6AE565AF_u32.to_le_bytes());
        bytes[0x54..0x56].copy_from_slice(&0xF0_u16.to_le_bytes());
        bytes[0x58..0x5A].copy_from_slice(&0x20B_u16.to_le_bytes());
        bytes[0x90..0x94].copy_from_slice(&0x1821C000_u32.to_le_bytes());
        assert_eq!(
            parse_pe_identity(&bytes),
            Some(PeIdentity {
                timestamp: 0x6AE565AF,
                size_of_image: 0x1821C000,
            })
        );
        for boundary in [0, 0x3F, 0x42, 0x54, 0x90, 0x147] {
            assert_eq!(parse_pe_identity(&bytes[..boundary]), None);
        }
        for offset in [0, 0x3F, 0x40, 0x58] {
            let mut invalid = bytes;
            invalid[offset] = 0xFF;
            assert_eq!(parse_pe_identity(&invalid), None, "offset {offset:#x}");
        }
        bytes[0x54] = 59;
        assert_eq!(parse_pe_identity(&bytes), None);
    }

    #[test]
    fn running_game_label_names_build_and_pid() {
        let game = RunningGame {
            pid: 4242,
            build: GameBuild::FM27_STEAM,
        };
        assert_eq!(game.to_string(), "FM27 (PID 4242)");
        let game = RunningGame {
            pid: 4242,
            build: GameBuild::FM27_GAMEPASS,
        };
        assert_eq!(game.to_string(), "FM27 Game Pass (PID 4242)");
    }

    #[test]
    fn version_sniffed_per_install_path() {
        let fm24 = r"S:\SteamLibrary\steamapps\common\Football Manager 2024\fm.exe";
        let fm27 = r"S:\SteamLibrary\steamapps\common\Football Manager 27\data\game_plugin.dll";
        assert_eq!(sniff_version(fm24), Ok(GameVersion::Fm24));
        assert_eq!(sniff_version(fm27), Ok(GameVersion::Fm27));
        assert!(sniff_version(r"C:\Games\fm.exe").is_err());
        assert_eq!(
            sniff_version(r"C:\Program Files\Epic Games\FootballManager24\fm.exe"),
            Ok(GameVersion::Fm24)
        );
        assert!(sniff_version(r"C:\2024\UnknownGame\fm.exe").is_err());
    }

    fn metadata(name: &str, version: &str) -> version_resource::VersionMetadata {
        version_resource::VersionMetadata {
            product_names: vec![name.into()],
            product_versions: vec![version.into()],
            file_versions: vec![],
        }
    }

    fn module_info(path: &str) -> Result<(u64, u64, String), String> {
        Ok((0x140000000, 0x100000, path.into()))
    }

    #[test]
    fn failed_detection_reports_only_this_process_modules_and_raw_metadata() {
        let path = r"C:\Custom\fm.exe";
        let (result, report) = detect_build_from_modules(
            25592,
            module_info(path),
            Err("module not mapped".into()),
            |requested| {
                assert_eq!(requested, path);
                let mut info = metadata("Unknown game", "99.2+123");
                info.file_versions.push("99.2".into());
                Ok(info)
            },
        );
        assert!(result.is_err());
        for expected in [
            "PID 25592",
            "module fm.exe:",
            "base=0x140000000",
            "size=0x100000",
            "Unknown game",
            "99.2+123",
            "FileVersion: [\"99.2\"]",
            "game_plugin.dll (optional): discovery failed for this PID: module not mapped",
            "selected module: fm.exe",
            "cannot determine game version",
        ] {
            assert!(report.contains(expected), "missing {expected:?}: {report}");
        }
        assert!(!report.contains("FM27"), "{report}");
        assert!(!report.contains("Football Manager 27"), "{report}");
    }

    #[test]
    fn detection_report_retains_metadata_for_unrecorded_storefront() {
        let path = r"C:\Program Files\Epic Games\FootballManager26\fm.exe";
        let (result, report) =
            detect_build_from_modules(1, module_info(path), Err("not mapped".into()), |_| {
                Ok(metadata("Football Manager 2026", "26.3.2"))
            });
        let build = result.unwrap();
        assert_eq!(build.version(), GameVersion::Fm26);
        assert_eq!(build.storefront(), Storefront::Epic);
        assert!(build.is_detected());
        assert!(report.contains("26.3.2"), "{report}");
        assert!(
            report.contains("release: FM26; source: PE ProductName"),
            "{report}"
        );
        assert!(report.contains("storefront hint: Epic"), "{report}");
    }

    #[test]
    fn epic_fm24_install_selects_the_epic_schema() {
        let path = r"C:\Program Files\Epic Games\FootballManager24\fm.exe";
        let (result, report) =
            detect_build_from_modules(1, module_info(path), Err("not mapped".into()), |_| {
                Ok(metadata("Football Manager 2024", "24.4.2+2081827"))
            });
        assert_eq!(result, Ok(GameBuild::FM24_EPIC));
        assert!(report.contains("FM24 Epic schema FM24_EPIC"), "{report}");
    }

    #[test]
    fn detection_report_explains_successful_directory_fallback() {
        let (result, report) = detect_build_from_modules(
            2,
            module_info("/Football Manager 2024/fm.exe"),
            Err("not mapped".into()),
            |_| Err("permission denied reading resource".into()),
        );
        assert_eq!(result, Ok(GameBuild::FM24_STEAM));
        assert!(
            report.contains("permission denied reading resource"),
            "{report}"
        );
        assert!(
            report.contains("source: install-directory fallback"),
            "{report}"
        );
        assert!(
            report.contains("layout compatibility still requires"),
            "{report}"
        );
    }

    #[test]
    fn detection_report_distinguishes_unity_exe_from_fm_plugin() {
        let (result, report) = detect_build_from_modules(
            3,
            module_info("/custom/fm.exe"),
            module_info("/custom/game_plugin.dll"),
            |path| {
                if path.ends_with(".exe") {
                    Ok(metadata("Unity", "6000.3.18f1"))
                } else {
                    Ok(metadata("Football Manager 27", "27.0.0+2493426"))
                }
            },
        );
        assert_eq!(result, Ok(GameBuild::FM27_STEAM_27_1));
        assert!(
            report.contains("Unity") && report.contains("6000.3.18f1"),
            "{report}"
        );
        assert!(
            report.contains("selected module: game_plugin.dll"),
            "{report}"
        );
        assert!(report.contains("27.0.0+2493426"), "{report}");
    }

    #[test]
    fn failed_module_discovery_retains_both_errors_without_reading_files() {
        let (result, report) = detect_build_from_modules(
            4,
            Err("exe enumeration denied".into()),
            Err("plugin not mapped".into()),
            |_| panic!("must not read an undiscovered module"),
        );
        assert!(result.is_err());
        assert!(
            report.contains("exe enumeration denied") && report.contains("plugin not mapped"),
            "{report}"
        );
        assert!(!report.contains("selected module:"), "{report}");
    }

    #[test]
    fn conflicting_identity_report_retains_all_product_versions() {
        let (result, report) = detect_build_from_modules(
            5,
            module_info("/Football Manager 2024/fm.exe"),
            Err("not mapped".into()),
            |_| {
                let mut info = metadata("Football Manager 2024", "24.4.2");
                info.product_names.push("Football Manager 27".into());
                info.product_versions.push("27.0.0".into());
                Ok(info)
            },
        );
        assert!(result.unwrap_err().contains("conflicting"));
        assert!(
            report.contains("24.4.2") && report.contains("27.0.0"),
            "{report}"
        );
        assert!(
            !report.contains("source: install-directory fallback"),
            "{report}"
        );
    }

    #[test]
    #[ignore = "requires FM_VERSION_TEST_MODULE pointing to an installed game module"]
    fn diagnose_installed_module_from_metadata() {
        let path = std::env::var("FM_VERSION_TEST_MODULE").expect("set FM_VERSION_TEST_MODULE");
        let info = version_resource::read(&path);
        let result = identify_version(&path, info.clone());
        let report = format!(
            "FILE-ONLY PE diagnostic (no process/module discovery or memory reads); test-runner PID {}\npath: {path:?}\nmetadata: {info:#?}\nrelease: {result:?}",
            std::process::id()
        );
        println!("{report}");
        detection_log::record(std::process::id(), &report);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn product_metadata_identifies_release_without_install_directory() {
        let fm24 = metadata("Football Manager 2024", "24.4.2+2081827");
        assert_eq!(
            identify_version(r"C:\Custom\fm.exe", Ok(fm24)),
            Ok(GameVersion::Fm24)
        );
        let fm27 = metadata("Football Manager 27", "27.0.0+2493426");
        // Metadata wins over a stale or misleading directory name.
        assert_eq!(
            identify_version("/Football Manager 2024/game_plugin.dll", Ok(fm27)),
            Ok(GameVersion::Fm27)
        );
        assert_eq!(
            version_from_metadata(&metadata("Football Manager 2026", "26.3.2")),
            Ok(Some(GameVersion::Fm26))
        );
    }

    #[test]
    fn missing_or_generic_metadata_uses_path_fallback() {
        let path = r"C:\Epic Games\FootballManager24\fm.exe";
        assert_eq!(
            identify_version(path, Err("resource missing".into())),
            Ok(GameVersion::Fm24)
        );
        // Unity's version number is not an FM release.
        assert_eq!(
            version_from_metadata(&metadata("Unity", "6000.3.18f1")),
            Ok(None)
        );
        assert_eq!(
            identify_version(
                "/Football Manager 27/fm.exe",
                Ok(metadata("Unity", "6000.3.18f1"))
            ),
            Ok(GameVersion::Fm27)
        );
        let err = identify_version("/custom/fm.exe", Err("cannot read file".into())).unwrap_err();
        assert!(
            err.contains("module path") && err.contains("cannot read file"),
            "{err}"
        );
        assert!(!err.contains("game_plugin.dll"), "{err}");
    }

    #[test]
    fn explicit_unsupported_or_conflicting_metadata_never_uses_path_fallback() {
        let path = "/Football Manager 2024/fm.exe";
        assert!(
            identify_version(path, Ok(metadata("Football Manager 2025", "25.0")))
                .unwrap_err()
                .contains("unsupported")
        );
        let mut conflict = metadata("Football Manager 2024", "24.4.2");
        conflict.product_names.push("Football Manager 27".into());
        assert!(
            identify_version(path, Ok(conflict))
                .unwrap_err()
                .contains("conflicting")
        );
    }

    #[test]
    #[ignore = "requires FM_VERSION_TEST_MODULE pointing to an installed game module"]
    fn identify_installed_module_from_metadata() {
        let path = std::env::var("FM_VERSION_TEST_MODULE").expect("set FM_VERSION_TEST_MODULE");
        let info = version_resource::read(&path).expect("read version resource");
        let version = version_from_metadata(&info)
            .expect("unambiguous FM identity")
            .expect("FM product name");
        println!("{path}: {info:?}; identified {}", version.display_name());
        // A neutral path proves the result is independent of the install folder.
        assert_eq!(identify_version("/neutral/game.bin", Ok(info)), Ok(version));
    }

    #[test]
    fn storefront_from_install_location() {
        let steam = r"S:\SteamLibrary\steamapps\common\Football Manager 27\fm.exe";
        let proton = "/home/u/.steam/steam/steamapps/common/Football Manager 27/fm.exe";
        let xbox = r"D:\XboxGames\Football Manager 27\Content\fm.exe";
        let store = r"C:\Program Files\WindowsApps\SportsInteractive.FM27_1.0.0.0_x64__abc\fm.exe";
        assert_eq!(storefront_of(steam, false), Storefront::Steam);
        assert_eq!(storefront_of(proton, false), Storefront::Steam);
        assert_eq!(storefront_of(xbox, false), Storefront::GamePass);
        assert_eq!(storefront_of(store, false), Storefront::GamePass);
        let epic = r"C:\Program Files\Epic Games\FootballManager24\fm.exe";
        assert_eq!(storefront_of(epic, false), Storefront::Epic);
        assert_eq!(
            GameBuild::resolve(GameVersion::Fm24, storefront_of(epic, false)),
            Ok(GameBuild::FM24_EPIC)
        );
        // A moved Game Pass install is still recognised by its GDK manifest.
        assert_eq!(
            storefront_of(r"E:\Games\FM27\fm.exe", true),
            Storefront::GamePass
        );
    }

    #[test]
    fn module_hash_reads_only_the_header() {
        let dir = std::env::temp_dir().join(format!("fm-editor-hash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let short = dir.join("short.bin");
        let long = dir.join("long.bin");
        std::fs::write(&short, vec![7u8; 65536]).unwrap();
        std::fs::write(&long, [vec![7u8; 65536], vec![9u8; 100]].concat()).unwrap();
        let h = |p: &std::path::Path| {
            read_module_header(p.to_str().unwrap()).map(|bytes| hash_module_header(&bytes))
        };
        assert_eq!(h(&short).unwrap(), h(&long).unwrap());
        let missing = h(&dir.join("missing.bin")).unwrap_err();
        assert!(missing.contains("missing.bin"), "{missing}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
