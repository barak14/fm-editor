//! `read_name_debug` — diagnose name field resolution for a player or staff entity.
//!
//! Shows every name-related field: struct offset, whether it falls inside the
//! struct read window, the raw pointer value, whether the pointer is valid, and
//! what `read_fm_name` resolves to for FmString fields. Finishes with the
//! display name that the normal read path returns.
//!
//! Use this to diagnose blank names — typically caused by a field offset that
//! falls outside the `player_struct_size` / `staff_struct_size` read window.

use serde_json::{json, Value};

use crate::{player, staff};
use crate::debug::DebugState;
use super::arg_u64;

pub fn read_name_debug(args: Value, state: &DebugState) -> Result<String, String> {
    let address     = arg_u64(&args, "address")?;
    let entity_type = args["entity_type"].as_str().unwrap_or("player");

    let fm_guard = state.fm.lock().unwrap();
    let fm       = fm_guard.as_ref().ok_or("Not attached.")?;
    let schemas  = fm.schemas();

    let struct_size = match entity_type {
        "staff" => schemas.staff_struct_size,
        _       => schemas.player_struct_size,
    };

    let raw = fm.read_bytes(address, struct_size)
        .map_err(|e| format!("read struct @ 0x{address:X}: {e}"))?;

    // Name-related field keys per entity type.
    let name_keys: &[&str] = match entity_type {
        "staff" => &[
            "s_first_name", "s_last_name",
            "s_name_first_sub", "s_name_last_sub",
        ],
        _ => &[
            "p_name",           "p_name_first_sub",  "p_name_last_sub",
            "p_name2",          "p_name2_first_sub", "p_name2_last_sub",
        ],
    };

    let mut field_info = serde_json::Map::new();

    for &key in name_keys {
        let entry = match schemas.field(key) {
            None => json!({ "error": "field not in schema for this game version" }),
            Some(f) => match f.offset.as_ref() {
                None => json!({ "error": "no offset mapped" }),
                Some(loc) => {
                    let off     = loc.offset() as usize;
                    let enc_str = format!("{:?}", loc.encoding());
                    let in_bounds = off + 8 <= raw.len();

                    if !in_bounds {
                        json!({
                            "offset":     format!("0x{off:03X}"),
                            "encoding":   enc_str,
                            "in_struct":  false,
                            "note":       format!(
                                "OUTSIDE struct window (struct_size=0x{struct_size:X}={struct_size}) — \
                                 this is why the name is blank"
                            )
                        })
                    } else {
                        let ptr   = u64::from_le_bytes(raw[off..off + 8].try_into().unwrap());
                        let valid = ptr >= 0x10000 && ptr <= 0x7FFFFFFFFFFF;

                        // Try read_fm_name for FmString-encoded fields.
                        let decoded = if valid && enc_str.to_lowercase().contains("fmstring") {
                            Some(fm.read_fm_name(ptr)
                                .unwrap_or_else(|e| format!("(read error: {e})")))
                        } else {
                            None
                        };

                        let mut m = serde_json::Map::new();
                        m.insert("offset".into(),    json!(format!("0x{off:03X}")));
                        m.insert("encoding".into(),  json!(enc_str));
                        m.insert("in_struct".into(), json!(true));
                        m.insert("ptr".into(),       json!(format!("0x{ptr:016X}")));
                        m.insert("ptr_valid".into(), json!(valid));
                        if let Some(s) = decoded {
                            m.insert("string_value".into(), json!(s));
                        }
                        Value::Object(m)
                    }
                }
            }
        };
        field_info.insert(key.to_string(), entry);
    }

    // Final name via the normal read path.
    let resolved = match entity_type {
        "staff" => staff::read_staff_name(fm, &raw)
            .unwrap_or_else(|| "(blank — name fields unresolvable)".to_string()),
        // Through `read_player_raw` so a player-coach's moved person block
        // resolves (the per-field dump above shows the raw object as-is).
        _ => player::read_player_raw(fm, address).ok()
            .and_then(|(schema_raw, _)| player::read_player_name(fm, &schema_raw))
            .unwrap_or_else(|| "(blank — name fields unresolvable)".to_string()),
    };

    let result = json!({
        "address":         format!("0x{address:X}"),
        "entity_type":     entity_type,
        "struct_size":     struct_size,
        "struct_size_hex": format!("0x{struct_size:X}"),
        "fields":          Value::Object(field_info),
        "resolved_name":   resolved
    });

    Ok(serde_json::to_string_pretty(&result).unwrap())
}
