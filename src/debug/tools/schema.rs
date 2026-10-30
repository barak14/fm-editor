//! `get_known_schema` — return all already-mapped fields for an entity type.

use serde_json::{json, Value};

use crate::fields::{self, Entity};

pub fn get_known_schema(args: Value) -> Result<String, String> {
    let entity_type = args["entity_type"].as_str().unwrap_or("player");

    let entity = match entity_type {
        "staff" => Entity::Staff,
        "club"  => Entity::Club,
        _       => Entity::Player,
    };

    let mut results: Vec<Value> = fields::fields_for(entity)
        .into_iter()
        .filter(|f| f.offset.is_some())
        .map(|f| {
            let (offset_hex, encoding) = match &f.offset {
                Some(loc) => (
                    format!("0x{:03X}", loc.offset()),
                    format!("{:?}", loc.encoding()),
                ),
                None => ("?".into(), "?".into()),
            };
            json!({
                "key":         f.key,
                "label":       f.label,
                "offset":      offset_hex,
                "encoding":    encoding,
                "category":    f.category.to_string()
            })
        })
        .collect();

    // Sort by offset for easy reading.
    results.sort_by_key(|v| {
        v["offset"].as_str()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(u64::MAX)
    });

    Ok(serde_json::to_string_pretty(&results).unwrap())
}
