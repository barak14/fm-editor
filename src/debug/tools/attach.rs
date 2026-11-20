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
use crate::process::{self, DetectionReport, DetectionStatus, FmProcess};
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
        *state
            .save_identity
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
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
                *state
                    .save_identity
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = None;
                *guard = Some(fm);
            }
            Err(e) => {
                *guard = None;
                *state.mini_cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
                *state
                    .save_identity
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = None;
                return Ok(serde_json::to_string_pretty(&json!({
                    "attached":      false,
                    "error":         e,
                    "running_games": running,
                }))
                .unwrap());
            }
        }
    }

    let fm = guard.as_mut().ok_or("not attached")?;
    refresh_save_gate(fm, state);
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
        "schema_known_dll_hash": format!("0x{:016X}", fm.build.fingerprint()),
        "fingerprint_calibrated": fm.build.fingerprint() != 0,
        "detection": detection_json(&fm.detection),
        "pid":           fm.pid,
        "editor_pid":    state.editor_pid.load(Ordering::Relaxed),
        "running_games": running,
        "players":       players,
        "staff":         staff,
        "clubs":         clubs
    }))
    .unwrap())
}

pub(in crate::debug) fn require_validated_save(state: &DebugState) -> Result<(), String> {
    let mut guard = state.fm.lock().unwrap_or_else(|e| e.into_inner());
    let fm = guard
        .as_mut()
        .ok_or("Not attached. Call attach_status first.")?;
    refresh_save_gate(fm, state);
    gate_access(&fm.detection)
}

fn gate_access(report: &DetectionReport) -> Result<(), String> {
    if report.pinned || matches!(report.status, DetectionStatus::Validated) {
        return Ok(());
    }
    match &report.status {
        DetectionStatus::Failed(message) => Err(message.clone()),
        _ => Err("Layout validation pending (save_identity check): no readable loaded save. Call attach_status after loading a save.".into()),
    }
}

fn save_identity(fm: &FmProcess) -> Option<Vec<u64>> {
    let (data, _, count) = club::clubs_array(fm).ok()?;
    let size = count.min(8).checked_mul(8)?;
    if size == 0 || !player::is_valid_ptr(data) {
        return None;
    }
    let end = data.checked_add(u64::try_from(size).ok()?)?;
    if !player::is_valid_ptr(end) {
        return None;
    }
    let bytes = fm.read_bytes(data, size).ok()?;
    if bytes.len() != size {
        return None;
    }
    let identity: Vec<_> = bytes
        .chunks_exact(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("8-byte chunk")))
        .collect();
    identity
        .iter()
        .all(|&ptr| player::is_valid_ptr(ptr))
        .then_some(identity)
}

fn refresh_save_gate(fm: &mut FmProcess, state: &DebugState) {
    use crate::process::layout_validate::{select_waiting_candidate, validate_save};

    if fm.detection.pinned {
        return;
    }
    let mut checked = state
        .save_identity
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut identity = save_identity(fm);
    if identity.is_none() || *checked != identity {
        identity = if select_waiting_candidate(fm) {
            save_identity(fm)
        } else {
            None
        };
    }
    if identity.is_some() && *checked == identity {
        return;
    }
    *state.mini_cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *checked = None;
    if identity.is_none() {
        fm.detection.status = DetectionStatus::AwaitingSave;
        fm.detection.chosen = None;
        fm.detection.gate = None;
        return;
    }
    let initial = fm.build;
    if validate_save(fm).is_err() {
        fm.build = initial;
    }
    *checked = save_identity(fm);
}

fn detection_json(report: &DetectionReport) -> Value {
    let (status, failure) = match &report.status {
        DetectionStatus::Pinned => ("pinned", None),
        DetectionStatus::AwaitingSave => ("awaiting_save", None),
        DetectionStatus::Validated => ("validated", None),
        DetectionStatus::Failed(message) => ("failed", Some(message.as_str())),
    };
    let gate = report.gate.as_ref().map(|gate| {
        json!({
            "passed": gate.passed(),
            "summary": gate.summary(),
            "checks": gate.checks.iter().map(|check| json!({
                "name": check.name,
                "sampled": check.sampled,
                "passed": check.passed,
                "required_percent": check.required_percent,
                "ok": check.ok,
                "note": check.note,
            })).collect::<Vec<_>>(),
        })
    });
    json!({
        "release": report.release.display_name(),
        "storefront": report.storefront.display_name(),
        "module_hash": report.module_hash.map(|hash| format!("0x{hash:016X}")),
        "pinned": report.pinned,
        "candidates": report.candidates,
        "chosen": report.chosen,
        "status": status,
        "failure": failure,
        "gate": gate,
        "force_scan": report.force_scan,
        "scan": report.scan.as_ref().map(|scan| json!({
            "ok": scan.result.is_ok(),
            "missing": scan.missing,
            "summary": scan.summary(),
        })),
        "summary": report.summary(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_version::{GameVersion, Storefront};
    use crate::process::layout_validate::{GateCheck, GateReport};

    #[test]
    fn detection_json_preserves_failure_and_gate_checks() {
        let report = DetectionReport {
            release: GameVersion::Fm27,
            storefront: Storefront::Epic,
            module_hash: Some(42),
            pinned: false,
            force_scan: false,
            candidates: vec!["FM27", "FM27 Game Pass"],
            chosen: None,
            status: DetectionStatus::Failed("players failed".to_owned()),
            gate: Some(GateReport {
                checks: vec![GateCheck {
                    name: "players",
                    sampled: 256,
                    passed: 0,
                    required_percent: 95,
                    ok: false,
                    note: Some("unreadable".to_owned()),
                }],
            }),
            scan: None,
        };
        let value = detection_json(&report);
        assert_eq!(value["pinned"], false);
        assert_eq!(value["module_hash"], "0x000000000000002A");
        assert_eq!(value["storefront"], "Epic");
        assert_eq!(value["status"], "failed");
        assert_eq!(value["failure"], "players failed");
        assert_eq!(value["candidates"].as_array().unwrap().len(), 2);
        assert!(value["chosen"].is_null());
        assert_eq!(value["gate"]["passed"], false);
        assert_eq!(value["gate"]["checks"][0]["name"], "players");
        assert_eq!(value["gate"]["checks"][0]["note"], "unreadable");
        assert_eq!(gate_access(&report).unwrap_err(), "players failed");
    }

    #[test]
    fn entity_access_requires_a_passed_gate_unless_pinned() {
        let mut report = DetectionReport {
            release: GameVersion::Fm27,
            storefront: Storefront::Epic,
            module_hash: None,
            pinned: false,
            force_scan: false,
            candidates: vec![],
            chosen: None,
            status: DetectionStatus::AwaitingSave,
            gate: None,
            scan: None,
        };
        assert!(
            gate_access(&report)
                .unwrap_err()
                .contains("save_identity check")
        );
        report.status = DetectionStatus::Validated;
        assert!(gate_access(&report).is_ok());
        report.status = DetectionStatus::Failed("contracts failed".into());
        assert_eq!(gate_access(&report).unwrap_err(), "contracts failed");
        report.pinned = true;
        assert!(gate_access(&report).is_ok());
    }
}
