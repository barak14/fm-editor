//! Read-only access to the exact preview checks used by the GUI policy trial.
use crate::club_policy::ClubIdentity;
use crate::debug::DebugState;
use serde_json::{Value, json};

pub fn run(args: Value, state: &DebugState) -> Result<String, String> {
    let pid = u32::try_from(super::arg_u64(&args, "pid")?).map_err(|_| "PID out of range")?;
    let identity = ClubIdentity {
        address: super::arg_u64(&args, "expected_address")?,
        index: usize::try_from(super::arg_u64(&args, "index")?)
            .map_err(|_| "Index out of range")?,
        uid: u32::try_from(super::arg_u64(&args, "expected_uid")?)
            .map_err(|_| "UID out of range")?,
    };
    let guard = state.fm.lock().map_err(|_| "Attachment lock poisoned")?;
    let fm = guard.as_ref().ok_or("Call attach_status first")?;
    if fm.pid != pid {
        return Err("Stale attachment PID".into());
    }
    // Epoch zero is reserved for diagnostics and cannot match a GUI worker.
    let proposal = crate::club_policy::prepare(fm, 0, identity)?;
    serde_json::to_string_pretty(&json!({
        "pid":pid,"build":fm.build.display_name(),"module_header_hash":format!("0x{:016X}",fm.module_header_hash()?),
        "club_uid":identity.uid,"club_address":format!("0x{:X}",identity.address),
        "game_date":fm.game_date().map(|d|json!({"day":d.day,"year":d.year})),
        "current_policy_codes":proposal.current_codes,"proposed_input_before":proposal.before,"proposed_input_after":proposal.after,
        "proposed_address":format!("0x{:X}",proposal.address()),"width_bytes":1,
        "writes_enabled":false,"conversion_supported":false,
        "limits":"Read-only proposal. No writable token is exposed. One club policy input is proposed; player contracts and financial terms are not changed. A live Apply/Undo/reload/day trial has not passed."
    })).map_err(|e|e.to_string())
}
