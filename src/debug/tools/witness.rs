//! `compare_entities` — cross-witness field verification.

use serde_json::{json, Value};

use crate::debug::DebugState;
use super::{arg_u64, arg_usize, Encoding};

/// Read the same offset across multiple entity addresses and decode each.
/// Primary confirmation tool: a candidate offset is trustworthy when 3+ witnesses
/// all show the expected value.
pub fn compare_entities(args: Value, state: &DebugState) -> Result<String, String> {
    let addresses: Vec<u64> = args["addresses"]
        .as_array()
        .ok_or("addresses must be an array")?
        .iter()
        .enumerate()
        .map(|(i, v)| v.as_u64().ok_or_else(|| format!("addresses[{i}] must be an integer")))
        .collect::<Result<Vec<_>, _>>()?;

    let offset  = arg_u64(&args, "offset")?;
    let size    = arg_usize(&args, "size")?.min(256);
    let enc_str = args["encoding"].as_str().unwrap_or("u8");
    let enc     = Encoding::from_str(enc_str)?;

    if addresses.is_empty() {
        return Err("addresses array is empty".into());
    }

    let fm = state.fm.lock().unwrap();
    let fm = fm.as_ref().ok_or("Not attached.")?;

    let results: Vec<Value> = addresses
        .iter()
        .map(|&addr| {
            let read_addr = addr.wrapping_add(offset);
            match fm.read_bytes(read_addr, size) {
                Ok(raw) => json!({
                    "address": format!("0x{addr:X}"),
                    "raw":     Encoding::raw_hex(&raw, size),
                    "decoded": enc.decode(&raw, fm)
                }),
                Err(e) => json!({
                    "address": format!("0x{addr:X}"),
                    "error":   e
                }),
            }
        })
        .collect();

    Ok(serde_json::to_string_pretty(&results).unwrap())
}
