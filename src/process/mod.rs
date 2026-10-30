//! Attach to fm.exe and read/write its memory.
//!
//! Platform backends implement the raw primitives — pid lookup, module
//! enumeration, cross-process byte transfer. Everything above raw bytes
//! (version sniffing, the DLL-hash gate, typed reads, FM name decoding)
//! is shared and platform-independent.
//!
//! On Linux the target is the same Windows fm.exe running under
//! Proton/Wine, so every schema offset is identical to a native Windows
//! install; only the process-access primitives differ.

#[cfg(windows)]
mod win32;
#[cfg(windows)]
use self::win32 as backend;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use self::linux as backend;

use crate::fields::VersionedSchemas;
use crate::game_version::GameVersion;

/// Handle to an open game process with its resolved game module and
/// selected game version.
pub struct FmProcess {
    handle: backend::ProcHandle,
    pub pid: u32,
    /// Base of the module that holds the game data RVAs:
    /// `game_plugin.dll` on FM26, `fm.exe` itself on FM24
    /// (see `GameVersion::module_name`).
    pub gp_base: u64,
    pub gp_end: u64,
    pub version: GameVersion,
    /// Set when `known_dll_hash` is 0 in the schema (not yet calibrated).
    /// The worker forwards this to the UI as a status notice so the developer
    /// knows the current hash to paste into `fields/<version>/mod.rs`.
    pub dll_hash_notice: Option<String>,
}

impl FmProcess {
    /// Find a running game process and attach to it.
    ///
    /// Both FM24 and FM26 run as `fm.exe`. The version is sniffed from the
    /// install path (it always contains the release year): FM26 keeps its
    /// game logic in `game_plugin.dll`, FM24 in `fm.exe` itself, so the
    /// plugin path is tried first and the exe path is the fallback.
    pub fn attach() -> Result<Self, String> {
        let pid = backend::find_pid("fm.exe").map_err(|_| "fm.exe is not running".to_string())?;
        let handle = backend::open(pid)?;
        let sniff_path = backend::find_module_info(pid, "game_plugin.dll")
            .or_else(|_| backend::find_module_info(pid, "fm.exe"))?
            .2;
        let version = sniff_version(&sniff_path)?;
        crate::fields::set_active_version(version);
        let (gp_base, gp_size, gp_path) = backend::find_module_info(pid, version.module_name())?;
        // Probe one read up front so a blocked cross-process access path
        // (e.g. Linux yama ptrace_scope) fails the attach with the OS hint
        // instead of erroring on the first field read later.
        backend::read_bytes(&handle, gp_base, 2)
            .map_err(|e| format!("attached to pid {pid} but cannot read its memory: {e}"))?;
        let dll_hash = hash_dll_header(&gp_path);
        let known = version.schemas().known_dll_hash;
        let dll_hash_notice = if known == 0 {
            // Schema not yet calibrated — allow attach but surface the hash.
            Some(format!(
                "DLL hash not calibrated (current: 0x{dll_hash:016X}). \
                 Set known_dll_hash: 0x{dll_hash:016X} in src/fields/{}/mod.rs \
                 to enable the version gate.",
                version.short_name(),
            ))
        } else if known != dll_hash {
            return Err(format!(
                "FM version mismatch — schema built for 0x{known:016X}, \
                 running 0x{dll_hash:016X}. Re-verify offsets against the \
                 new patch before use."
            ));
        } else {
            None
        };
        Ok(Self {
            handle,
            pid,
            gp_base,
            gp_end: gp_base + gp_size,
            version,
            dll_hash_notice,
        })
    }

    /// The version-specific schema bundle for the attached process.
    ///
    /// Every reader / UI panel that used to reach into `crate::offsets::*`
    /// or global `fields::field()` helpers should pull field data, RVAs,
    /// and struct sizes through here.
    pub fn schemas(&self) -> &'static VersionedSchemas {
        self.version.schemas()
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
        String::from_utf8(buf[4..4 + len].to_vec())
            .map_err(|e| format!("name UTF-8 error: {e}"))
    }
}

/// Determine the game version from the full path of `game_plugin.dll`.
///
/// The Football Manager install directory always contains the release year
/// (e.g. `…\Football Manager 2026\game_plugin.dll`), which is the only
/// reliable distinguisher when both versions share the `fm.exe` exe name.
fn sniff_version(dll_path: &str) -> Result<GameVersion, String> {
    let lower = dll_path.to_lowercase();
    // Match both "Football Manager 2026" and "Football Manager 26" naming conventions.
    if lower.contains("2027") || lower.contains("manager 27") {
        Ok(GameVersion::Fm27)
    } else if lower.contains("2026") || lower.contains("manager 26") {
        Ok(GameVersion::Fm26)
    } else if lower.contains("2024") || lower.contains("manager 24") {
        Ok(GameVersion::Fm24)
    } else {
        Err(format!(
            "cannot determine game version from game_plugin.dll path: {dll_path}"
        ))
    }
}

/// FNV-1a hash of the first 64 KB of a DLL file.
///
/// Used as a lightweight version fingerprint to detect when FM has been
/// patched and the schema should be re-verified. Returns 0 if the file
/// cannot be read (attach continues; the gate is skipped rather than
/// hard-failing on a transient I/O error).
fn hash_dll_header(dll_path: &str) -> u64 {
    let Ok(bytes) = std::fs::read(dll_path) else {
        return 0;
    };
    let slice = &bytes[..bytes.len().min(65536)];
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in slice {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000000001b3);
    }
    h
}
