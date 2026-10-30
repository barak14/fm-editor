//! `get_entity_address` and `search_entities` — entity discovery tools.

use serde_json::{json, Value};

use crate::debug::{DebugState, EntityEntry, MiniCache};
use crate::{club, fields, player, staff};
use super::arg_u64;

// ---------------------------------------------------------------------------
// get_entity_address
// ---------------------------------------------------------------------------

/// Returns `{ address, index }` for the Nth entity or the entity with a given UID.
pub fn get_entity_address(args: Value, state: &DebugState) -> Result<String, String> {
    let entity_type = args["entity_type"].as_str().unwrap_or("player");

    // UID lookup: try mini-cache first (O(1) if warm), then fall back to raw scan.
    if let Some(uid_val) = args.get("uid") {
        let uid = uid_val.as_u64().ok_or("uid must be an integer")? as u32;

        // Fast path: search mini-cache if available (releases lock before fm lock).
        {
            let cache_g = state.mini_cache.lock().unwrap();
            if let Some(cache) = cache_g.as_ref() {
                let entries = match entity_type {
                    "staff" => &cache.staff,
                    "club"  => &cache.clubs,
                    _       => &cache.players,
                };
                if let Some(e) = entries.iter().find(|e| e.uid == uid) {
                    return Ok(serde_json::to_string_pretty(&json!({
                        "address": e.addr,
                        "index":   e.index,
                        "uid":     uid,
                        "source":  "cache"
                    })).unwrap());
                }
            }
        } // cache lock released

        // Slow path: linear scan of the raw entity array.
        let fm_guard = state.fm.lock().unwrap();
        let fm = fm_guard.as_ref().ok_or("Not attached. Call attach_status first.")?;
        return find_by_uid(entity_type, uid, fm);
    }

    let index = arg_u64(&args, "index")? as usize;

    let fm_guard = state.fm.lock().unwrap();
    let fm = fm_guard.as_ref().ok_or("Not attached. Call attach_status first.")?;

    let (array_data, _, count) = array_bounds(entity_type, fm)?;
    if index >= count {
        return Err(format!("index {index} out of range (count={count})"));
    }

    let ptr = fm.read_u64(array_data + index as u64 * 8)
        .map_err(|e| format!("read pointer: {e}"))?;

    Ok(serde_json::to_string_pretty(&json!({
        "address": ptr,
        "index":   index
    })).unwrap())
}

fn find_by_uid(entity_type: &str, uid: u32, fm: &crate::process::FmProcess) -> Result<String, String> {
    let (array_data, _, count) = array_bounds(entity_type, fm)?;
    let uid_off = uid_offset(entity_type)?;
    let uid_bytes = uid.to_le_bytes();

    for i in 0..count {
        let ptr = fm.read_u64(array_data + i as u64 * 8).unwrap_or(0);
        if ptr < 0x10000 || ptr > 0x7FFFFFFFFFFF { continue; }
        let chunk = fm.read_bytes(ptr + uid_off, 4).unwrap_or_default();
        if chunk == uid_bytes {
            return Ok(serde_json::to_string_pretty(&json!({
                "address": ptr,
                "index":   i,
                "uid":     uid
            })).unwrap());
        }
    }

    Err(format!("UID {uid} not found in {entity_type} array"))
}

// ---------------------------------------------------------------------------
// search_entities
// ---------------------------------------------------------------------------

/// Search by name substring or numeric UID. Returns up to 10 matches.
/// Builds a mini-cache on first call (may take ~10s on a large save).
/// Pass `rebuild_cache: true` to force a fresh scan (e.g. after loading a new save).
pub fn search_entities(args: Value, state: &DebugState) -> Result<String, String> {
    let query       = args["query"].as_str().unwrap_or("").to_lowercase();
    let entity_type = args["entity_type"].as_str().unwrap_or("player");
    let rebuild     = args["rebuild_cache"].as_bool().unwrap_or(false);

    // Hold fm lock for entire duration (cache build + search).
    let fm_guard    = state.fm.lock().unwrap();
    let fm          = fm_guard.as_ref().ok_or("Not attached. Call attach_status first.")?;
    let mut cache_g = state.mini_cache.lock().unwrap();

    if rebuild || cache_g.is_none() {
        *cache_g = Some(build_mini_cache(fm)?);
    }

    let cache = cache_g.as_ref().unwrap();
    let entries = match entity_type {
        "staff" => &cache.staff,
        "club"  => &cache.clubs,
        _       => &cache.players,
    };

    // If query is a pure integer, search by UID; otherwise search by name.
    let results: Vec<_> = if let Ok(target_uid) = query.trim().parse::<u32>() {
        entries.iter()
            .filter(|e| e.uid == target_uid)
            .take(10)
            .collect()
    } else {
        entries.iter()
            .filter(|e| e.name.to_lowercase().contains(&query))
            .take(10)
            .collect()
    };

    let out: Vec<Value> = results.iter().map(|e| json!({
        "name":    e.name,
        "address": e.addr,
        "index":   e.index,
        "uid":     e.uid
    })).collect();

    Ok(serde_json::to_string_pretty(&out).unwrap())
}

// ---------------------------------------------------------------------------
// Mini-cache builder
// ---------------------------------------------------------------------------

fn build_mini_cache(fm: &crate::process::FmProcess) -> Result<MiniCache, String> {
    let p_uid_off = uid_offset("player")?;
    let s_uid_off = uid_offset("staff")?;
    let c_uid_off = uid_offset("club")?;

    // Players go through `read_player_raw` so player-coaches (person block
    // moved) decode in the same schema layout as plain players.
    let staff_size  = fm.schemas().staff_struct_size;
    let c_name_off  = fields::field("c_name")
        .ok_or("c_name missing")?.offset.ok_or("c_name no offset")?.offset();

    // -- Players --
    let (hd, _, hc) = player::humans_array(fm)?;
    let mut players = Vec::with_capacity(hc.min(600_000));
    for i in 0..hc {
        let ptr = fm.read_u64(hd + i as u64 * 8).unwrap_or(0);
        if ptr < 0x10000 || ptr > 0x7FFFFFFFFFFF { continue; }
        let Ok((raw, _)) = player::read_player_raw(fm, ptr) else { continue };
        let uid  = read_u32_le(&raw, p_uid_off as usize);
        let name = player::read_player_name(fm, &raw).unwrap_or_default();
        players.push(EntityEntry { uid, addr: ptr, index: i, name });
    }

    // -- Staff --
    let (sd, _, sc) = staff::staff_array(fm)?;
    let mut staff_entries = Vec::with_capacity(sc.min(300_000));
    for i in 0..sc {
        let ptr = fm.read_u64(sd + i as u64 * 8).unwrap_or(0);
        if ptr < 0x10000 || ptr > 0x7FFFFFFFFFFF { continue; }
        let raw = match fm.read_bytes(ptr, staff_size) { Ok(b) => b, Err(_) => continue };
        let uid  = read_u32_le(&raw, s_uid_off as usize);
        let name = staff::read_staff_name(fm, &raw).unwrap_or_default();
        staff_entries.push(EntityEntry { uid, addr: ptr, index: i, name });
    }

    // -- Clubs --
    let (cd, _, cc) = club::clubs_array(fm)?;
    let mut clubs = Vec::with_capacity(cc.min(200_000));
    for i in 0..cc {
        let ptr = fm.read_u64(cd + i as u64 * 8).unwrap_or(0);
        if ptr < 0x10000 || ptr > 0x7FFFFFFFFFFF { continue; }
        let uid      = fm.read_u32(ptr + c_uid_off).unwrap_or(0);
        let name_ptr = fm.read_u64(ptr + c_name_off).unwrap_or(0);
        let name     = fm.read_fm_name(name_ptr).unwrap_or_default();
        clubs.push(EntityEntry { uid, addr: ptr, index: i, name });
    }

    Ok(MiniCache { players, staff: staff_entries, clubs })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(super) fn array_bounds(
    entity_type: &str,
    fm: &crate::process::FmProcess,
) -> Result<(u64, u64, usize), String> {
    match entity_type {
        "staff" => staff::staff_array(fm),
        "club"  => club::clubs_array(fm),
        _       => player::humans_array(fm),
    }
}

fn uid_offset(entity_type: &str) -> Result<u64, String> {
    let key = match entity_type {
        "staff" => "s_id",
        "club"  => "c_uid",
        _       => "p_uid",
    };
    Ok(fields::field(key)
        .ok_or_else(|| format!("{key} field missing"))?
        .offset
        .ok_or_else(|| format!("{key} has no offset"))?
        .offset())
}

fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    buf.get(off..off + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .unwrap_or(0)
}
