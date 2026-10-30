//! `follow_pointers`, `probe_pointer_chain` — pointer chain traversal tools.

use serde_json::{json, Value};

use crate::debug::DebugState;
use crate::player::is_valid_ptr;
use super::{arg_u64, arg_usize};
use super::memory::format_hex_dump;

// ---------------------------------------------------------------------------
// follow_pointers
// ---------------------------------------------------------------------------

/// Find all 8-byte-aligned valid pointers in a region and preview 64 bytes at each.
pub fn follow_pointers(args: Value, state: &DebugState) -> Result<String, String> {
    let base = arg_u64(&args, "base")?;
    let size = arg_usize(&args, "size")?.min(4096);

    let fm = state.fm.lock().unwrap();
    let fm = fm.as_ref().ok_or("Not attached.")?;

    let raw = fm.read_bytes(base, size)
        .map_err(|e| format!("ReadProcessMemory: {e}"))?;

    let mut results = Vec::new();

    for i in (0..raw.len().saturating_sub(7)).step_by(8) {
        let candidate = u64::from_le_bytes(raw[i..i + 8].try_into().unwrap());
        if !is_valid_ptr(candidate) { continue; }

        // Read 64-byte preview at the target.
        let preview = fm.read_bytes(candidate, 64)
            .map(|b| format_hex_dump(candidate, &b))
            .unwrap_or_else(|_| "(unreadable)".into());

        results.push(json!({
            "offset":  i,
            "pointer": format!("0x{candidate:X}"),
            "preview": preview
        }));
    }

    if results.is_empty() {
        Ok(format!("No valid pointers found in {size} bytes at 0x{base:X}"))
    } else {
        Ok(serde_json::to_string_pretty(&results).unwrap())
    }
}

// ---------------------------------------------------------------------------
// probe_pointer_chain
// ---------------------------------------------------------------------------

/// Follow a chain: base → deref(+offsets[0]) → deref(+offsets[1]) → ...
/// Returns the resolved address and raw hex at the final hop.
pub fn probe_pointer_chain(args: Value, state: &DebugState) -> Result<String, String> {
    let mut current = arg_u64(&args, "base")?;

    let offsets: Vec<u64> = args["offsets"]
        .as_array()
        .ok_or("offsets must be an array")?
        .iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_u64().ok_or_else(|| format!("offsets[{i}] must be an integer"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let fm = state.fm.lock().unwrap();
    let fm = fm.as_ref().ok_or("Not attached.")?;

    let mut trace = vec![format!("0x{current:X}")];

    for (hop, &off) in offsets.iter().enumerate() {
        let read_addr = current.wrapping_add(off);
        let next = fm.read_u64(read_addr)
            .map_err(|e| {
                format!(
                    "Failed at hop {hop} (read u64 at 0x{read_addr:X} = 0x{current:X} + 0x{off:X}): {e}"
                )
            })?;

        if !is_valid_ptr(next) {
            return Err(format!(
                "Hop {hop}: deref(0x{current:X} + 0x{off:X}) = 0x{next:X} — not a valid pointer"
            ));
        }

        trace.push(format!("0x{next:X}"));
        current = next;
    }

    // Read 32 bytes at final address for a quick sanity preview.
    let raw = fm.read_bytes(current, 32).unwrap_or_default();
    let raw_hex: String = raw.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");

    Ok(serde_json::to_string_pretty(&json!({
        "resolved_address": format!("0x{current:X}"),
        "trace":            trace,
        "raw":              raw_hex
    })).unwrap())
}
