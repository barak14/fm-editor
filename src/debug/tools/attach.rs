//! `attach_status` — connection probe, always the first tool called.

use crate::debug::DebugState;
use crate::{club, player, staff};

pub fn run(state: &DebugState) -> Result<String, String> {
    let mut guard = state.fm.lock().unwrap();

    // Auto-attach if not yet attached.
    if guard.is_none() {
        match crate::process::FmProcess::attach() {
            Ok(fm) => { *guard = Some(fm); }
            Err(e) => {
                return Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "attached": false,
                    "error":    e
                })).unwrap());
            }
        }
    }

    let fm = guard.as_ref().unwrap();
    let version = format!("{:?}", fm.version);

    let players = player::humans_array(fm).map(|(_, _, c)| c).unwrap_or(0);
    let staff   = staff::staff_array(fm)  .map(|(_, _, c)| c).unwrap_or(0);
    let clubs   = club::clubs_array(fm)   .map(|(_, _, c)| c).unwrap_or(0);

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "attached": true,
        "version":  version,
        "players":  players,
        "staff":    staff,
        "clubs":    clubs
    })).unwrap())
}
