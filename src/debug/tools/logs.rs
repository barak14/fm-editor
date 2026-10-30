//! `read_logs` — drain the debug log buffer.

use crate::debug::DebugState;

/// Return up to 100 most-recent log entries and clear the buffer.
pub fn read_logs(state: &DebugState) -> Result<String, String> {
    let mut guard = state.log.lock().unwrap();

    let entries: Vec<serde_json::Value> = guard
        .drain(..)
        .take(100)
        .map(|e| serde_json::json!({
            "level":     e.level,
            "message":   e.message,
            "timestamp": e.timestamp
        }))
        .collect();

    Ok(serde_json::to_string_pretty(&entries).unwrap())
}
