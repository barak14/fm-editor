//! `read_entity_fields` — decode all mapped (or selected) fields for an entity
//! at a given memory address and return a JSON array of { key, label, offset,
//! raw_hex, value } entries sorted by offset.
//!
//! The programmatic counterpart to the live-verification step: instead of
//! visually comparing the editor UI against in-game values, an MCP client
//! calls this tool and checks the returned values directly.

use serde_json::{json, Value};

use crate::debug::DebugState;
use crate::fields::{Encoding, Entity};
use crate::process::FmProcess;
use super::arg_u64;

pub fn read_entity_fields(args: Value, state: &DebugState) -> Result<String, String> {
    let address     = arg_u64(&args, "address")?;
    let entity_type = args["entity_type"].as_str().unwrap_or("player");

    // Optional filter: if "fields" is an array of key strings, only return those.
    let filter: Option<Vec<String>> = args["fields"].as_array().map(|arr| {
        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
    });

    let entity = match entity_type {
        "staff" => Entity::Staff,
        "club"  => Entity::Club,
        _       => Entity::Player,
    };

    // Resolve fields and read bytes under the same attached-process lock.
    let fm_guard = state.fm.lock().unwrap();
    let fm = fm_guard.as_ref().ok_or("Not attached.")?;
    let all_fields = fm.schemas().fields_for(entity);

    // Keep only Direct-location fields that have a known offset and pass the filter.
    let fields_to_read: Vec<_> = all_fields.iter()
        .filter(|f| {
            matches!(f.offset, Some(crate::fields::FieldLocation::Direct { .. }))
        })
        .filter(|f| {
            filter.as_ref().map_or(true, |keys| keys.iter().any(|k| k == f.key))
        })
        .collect();

    if fields_to_read.is_empty() {
        return Ok("[]".to_string());
    }

    // Compute how many bytes we need to cover all fields.
    let max_end = fields_to_read.iter()
        .filter_map(|f| match f.offset.as_ref()? {
            crate::fields::FieldLocation::Direct { offset, encoding } =>
                Some(offset + encoding_size(*encoding) as u64),
            _ => None,
        })
        .max()
        .unwrap_or(512)
        .min(8192);

    let raw = fm.read_bytes(address, max_end as usize)
        .map_err(|e| format!("ReadProcessMemory: {e}"))?;

    let mut results: Vec<Value> = fields_to_read.iter()
        .filter_map(|f| {
            let (off, enc) = match f.offset.as_ref()? {
                crate::fields::FieldLocation::Direct { offset, encoding } =>
                    (*offset as usize, *encoding),
                _ => return None,
            };
            Some(json!({
                "key":     f.key,
                "label":   f.label,
                "offset":  format!("0x{:03X}", off),
                "raw_hex": raw_hex_for(&raw, off, enc),
                "value":   decode_field(&raw, off, enc, fm),
            }))
        })
        .collect();

    drop(fm_guard);

    results.sort_by_key(|v| {
        v["offset"].as_str()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(u64::MAX)
    });

    Ok(serde_json::to_string_pretty(&results).unwrap())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn encoding_size(enc: Encoding) -> usize {
    match enc {
        Encoding::Direct | Encoding::Encoded | Encoding::RoleIndex => 1,
        Encoding::Raw | Encoding::RawCapped(_)                     => 2,
        Encoding::RawU32 | Encoding::DateDayYear | Encoding::F32Raw => 4,
        Encoding::Pointer | Encoding::FmString                     => 8,
    }
}

fn decode_field(raw: &[u8], off: usize, enc: Encoding, fm: &FmProcess) -> String {
    match enc {
        Encoding::Direct => {
            raw.get(off).map(|b| b.to_string()).unwrap_or_else(|| "(oob)".to_string())
        }
        Encoding::Encoded => {
            raw.get(off)
                .map(|b| ((*b as u16 + 2) / 5).to_string())
                .unwrap_or_else(|| "(oob)".to_string())
        }
        Encoding::Raw => {
            if off + 2 > raw.len() { return "(oob)".to_string(); }
            u16::from_le_bytes(raw[off..off + 2].try_into().unwrap()).to_string()
        }
        Encoding::RawCapped(_) => {
            if off + 2 > raw.len() { return "(oob)".to_string(); }
            u16::from_le_bytes(raw[off..off + 2].try_into().unwrap()).to_string()
        }
        Encoding::RawU32 => {
            if off + 4 > raw.len() { return "(oob)".to_string(); }
            u32::from_le_bytes(raw[off..off + 4].try_into().unwrap()).to_string()
        }
        Encoding::DateDayYear => {
            if off + 4 > raw.len() { return "(oob)".to_string(); }
            let day  = u16::from_le_bytes(raw[off..off + 2].try_into().unwrap());
            let year = u16::from_le_bytes(raw[off + 2..off + 4].try_into().unwrap());
            format!("day {} of {}", day, year)
        }
        Encoding::Pointer => {
            if off + 8 > raw.len() { return "(oob)".to_string(); }
            let ptr = u64::from_le_bytes(raw[off..off + 8].try_into().unwrap());
            format!("0x{:X}", ptr)
        }
        Encoding::FmString => {
            if off + 8 > raw.len() { return "(oob)".to_string(); }
            let ptr = u64::from_le_bytes(raw[off..off + 8].try_into().unwrap());
            if ptr == 0 { return "(null)".to_string(); }
            fm.read_fm_name(ptr)
                .unwrap_or_else(|_| format!("(invalid ptr 0x{:X})", ptr))
        }
        Encoding::F32Raw => {
            if off + 4 > raw.len() { return "(oob)".to_string(); }
            f32::from_le_bytes(raw[off..off + 4].try_into().unwrap()).to_string()
        }
        Encoding::RoleIndex => {
            raw.get(off).map(|b| b.to_string()).unwrap_or_else(|| "(oob)".to_string())
        }
    }
}

fn raw_hex_for(raw: &[u8], off: usize, enc: Encoding) -> String {
    raw.iter()
        .skip(off)
        .take(encoding_size(enc))
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
