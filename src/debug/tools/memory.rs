//! `dump_struct`, `probe_range`, `scan_for_value` — raw memory inspection tools.

use serde_json::{json, Value};

use crate::debug::DebugState;
use super::{arg_u64, arg_usize, Encoding};

// ---------------------------------------------------------------------------
// dump_struct
// ---------------------------------------------------------------------------

/// Hex-dump a memory region as offset | hex | ASCII (16 bytes per row).
/// Optional: pass `entity_type` ("player"|"staff"|"club") and `struct_base`
/// (defaults to `address`) to append a known-field annotation table.
pub fn dump_struct(args: Value, state: &DebugState) -> Result<String, String> {
    let address     = arg_u64(&args, "address")?;
    let size        = arg_usize(&args, "size")?.min(4096);
    let entity_type = args["entity_type"].as_str().map(|s| s.to_string());
    // struct_base defaults to address; accepts decimal integer or "0x..." hex string.
    let struct_base: u64 = args.get("struct_base")
        .and_then(|v| {
            v.as_u64().or_else(|| {
                v.as_str().and_then(|s| {
                    let s = s.trim_start_matches("0x").trim_start_matches("0X");
                    u64::from_str_radix(s, 16).ok()
                })
            })
        })
        .unwrap_or(address);

    let raw = {
        let fm = state.fm.lock().unwrap();
        let fm = fm.as_ref().ok_or("Not attached.")?;
        fm.read_bytes(address, size)
            .map_err(|e| format!("ReadProcessMemory: {e}"))?
    };

    let mut out = format_hex_dump(address, &raw);

    if let Some(ref et) = entity_type {
        use crate::fields::{self, Entity};
        let entity = match et.as_str() {
            "staff" => Entity::Staff,
            "club"  => Entity::Club,
            _       => Entity::Player,
        };
        let mut annotations: Vec<(u64, String)> = fields::fields_for(entity)
            .into_iter()
            .filter_map(|f| {
                let loc = f.offset.as_ref()?;
                let off = loc.offset();
                let abs = struct_base + off;
                if abs >= address && abs < address + size as u64 {
                    Some((off, format!("+0x{off:03X}  {:<26} {:?}", f.key, loc.encoding())))
                } else {
                    None
                }
            })
            .collect();
        annotations.sort_by_key(|(off, _)| *off);
        if !annotations.is_empty() {
            out.push_str("\n--- Known fields in range ---\n");
            for (_, ann) in annotations {
                out.push_str(&ann);
                out.push('\n');
            }
        }
    }

    Ok(out)
}

pub fn format_hex_dump(_base_addr: u64, data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 4);
    for (row, chunk) in data.chunks(16).enumerate() {
        let offset = row * 16;
        // Offset column
        out.push_str(&format!("{offset:04X}  "));
        // Hex columns (with mid-gap)
        for (i, b) in chunk.iter().enumerate() {
            if i == 8 { out.push(' '); }
            out.push_str(&format!("{b:02X} "));
        }
        // Pad short rows
        let pad = 16 - chunk.len();
        for i in 0..pad {
            if chunk.len() + i == 8 { out.push(' '); }
            out.push_str("   ");
        }
        // ASCII column
        out.push(' ');
        for b in chunk {
            out.push(if b.is_ascii_graphic() { *b as char } else { '.' });
        }
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// probe_range
// ---------------------------------------------------------------------------

/// Sweep offsets start..end (step bytes) and decode each with the given encoding.
pub fn probe_range(args: Value, state: &DebugState) -> Result<String, String> {
    let base         = arg_u64(&args, "base")?;
    let start_offset = arg_u64(&args, "start_offset")?;
    let end_offset   = arg_u64(&args, "end_offset")?;
    let step         = arg_usize(&args, "step")?.max(1);
    let enc_str      = args["encoding"].as_str().unwrap_or("u8");
    let enc          = Encoding::from_str(enc_str)?;

    if end_offset < start_offset {
        return Err("end_offset must be >= start_offset".into());
    }
    let region_size = ((end_offset - start_offset) as usize) + enc.size();
    if region_size > 4096 {
        return Err(format!("Range too large ({region_size} bytes, max 4096). Narrow start/end_offset."));
    }

    let fm = state.fm.lock().unwrap();
    let fm = fm.as_ref().ok_or("Not attached.")?;

    let raw = fm.read_bytes(base + start_offset, region_size)
        .map_err(|e| format!("ReadProcessMemory: {e}"))?;

    let mut results = Vec::new();
    let mut pos = 0usize;
    let mut off = start_offset;
    while off < end_offset && pos + enc.size() <= raw.len() {
        let slice = &raw[pos..pos + enc.size()];
        results.push(json!({
            "offset":  off,
            "raw":     Encoding::raw_hex(slice, enc.size()),
            "decoded": enc.decode(slice, fm)
        }));
        pos += step;
        off += step as u64;
    }

    Ok(serde_json::to_string_pretty(&results).unwrap())
}

// ---------------------------------------------------------------------------
// scan_for_value
// ---------------------------------------------------------------------------

/// Find all offsets in a struct where the decoded value matches `value`.
pub fn scan_for_value(args: Value, state: &DebugState) -> Result<String, String> {
    let base    = arg_u64(&args, "base")?;
    let size    = arg_usize(&args, "size")?.min(4096);
    let target  = args["value"].as_str().unwrap_or("").to_lowercase();
    let enc_str = args["encoding"].as_str().unwrap_or("u8");
    let enc     = Encoding::from_str(enc_str)?;

    let fm = state.fm.lock().unwrap();
    let fm = fm.as_ref().ok_or("Not attached.")?;

    let raw = fm.read_bytes(base, size)
        .map_err(|e| format!("ReadProcessMemory: {e}"))?;

    let w = enc.size();
    let mut results = Vec::new();

    for i in (0..raw.len()).step_by(w.max(1)) {
        if i + w > raw.len() { break; }
        let slice   = &raw[i..i + w];
        let decoded = enc.decode(slice, fm).to_lowercase();
        if decoded == target {
            results.push(json!({
                "offset": i,
                "raw":    Encoding::raw_hex(slice, w),
                "decoded": enc.decode(slice, fm)
            }));
        }
    }

    if results.is_empty() {
        Ok(format!("No matches for '{target}' ({enc_str}) in {size} bytes at 0x{base:X}"))
    } else {
        Ok(serde_json::to_string_pretty(&results).unwrap())
    }
}
