//! `attach_status` — connection probe, always the first tool called.
//!
//! FM24, FM26 and FM27 all run as `fm.exe`, so several games can be open.
//! The MCP server holds its own `FmProcess` and reads through that
//! process's schema. Which process it holds, in order of precedence:
//! an explicit `pid` argument, the current attachment, the PID the editor
//! GUI is attached to, and finally the only running game.

use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use crate::debug::DebugState;
use crate::process::{self, FmProcess};
use crate::{club, player, staff};

pub fn run(args: Value, state: &DebugState) -> Result<String, String> {
    let requested = match args.get("pid") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            u32::try_from(v.as_u64().ok_or("pid must be an unsigned integer")?)
                .map_err(|_| "pid out of range")?,
        ),
    };
    let running: Vec<Value> = process::running_games()
        .map(|games| {
            games
                .iter()
                .map(|g| json!({ "pid": g.pid, "version": g.build.version().display_name(), "storefront": g.build.storefront().display_name() }))
                .collect()
        })
        .unwrap_or_default();

    let mut guard = state.fm.lock().unwrap_or_else(|e| e.into_inner());
    // The debug attachment is independent of the GUI. Forget a dead process
    // and its address cache before deciding which running game to inspect.
    // Checking the handle as well as PID membership also catches PID reuse.
    if guard.as_ref().is_some_and(|fm| {
        !running
            .iter()
            .any(|game| game["pid"].as_u64() == Some(u64::from(fm.pid)))
            || fm.read_u8(fm.gp_base).is_err()
    }) {
        *guard = None;
        *state.mini_cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    let current = guard.as_ref().map(|fm| fm.pid);
    let target = match (requested, current) {
        (Some(pid), Some(cur)) if pid == cur => None,
        (Some(pid), _) => Some(Ok(pid)),
        (None, Some(_)) => None,
        (None, None) => match state.editor_pid.load(Ordering::Relaxed) {
            0 => Some(Err(())),
            pid => Some(Ok(pid)),
        },
    };
    if let Some(target) = target {
        let attached = match target {
            Ok(pid) => FmProcess::attach_pid(pid),
            Err(()) => FmProcess::attach(),
        };
        match attached {
            Ok(fm) => {
                // Entries in the mini cache belong to the previous process.
                *state.mini_cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
                *guard = Some(fm);
            }
            Err(e) => {
                *guard = None;
                *state.mini_cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
                return Ok(serde_json::to_string_pretty(&json!({
                    "attached":      false,
                    "error":         e,
                    "running_games": running,
                }))
                .unwrap());
            }
        }
    }

    let fm = guard.as_ref().ok_or("not attached")?;
    let players = player::humans_array(fm).map(|(_, _, c)| c).unwrap_or(0);
    let staff = staff::staff_array(fm).map(|(_, _, c)| c).unwrap_or(0);
    let clubs = club::clubs_array(fm).map(|(_, _, c)| c).unwrap_or(0);

    Ok(serde_json::to_string_pretty(&json!({
        "attached":      true,
        "version":       fm.build.version().display_name(),
        "storefront":    fm.build.storefront().display_name(),
        "module_path":   fm.module_path,
        "module_base":   format!("0x{:X}", fm.gp_base),
        "module_end":    format!("0x{:X}", fm.gp_end),
        "module_header_hash": fm.module_header_hash().ok().map(|h| format!("0x{h:016X}")),
        "schema_known_dll_hash": format!("0x{:016X}", fm.schemas().known_dll_hash),
        "fingerprint_calibrated": fm.schemas().known_dll_hash != 0,
        "compatibility_notice": fm.dll_hash_notice,
        "pid":           fm.pid,
        "editor_pid":    state.editor_pid.load(Ordering::Relaxed),
        "running_games": running,
        "players":       players,
        "staff":         staff,
        "clubs":         clubs
    }))
    .unwrap())
}
