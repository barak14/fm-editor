//! Bulk entity cache loader — reads every player, staff member, and club
//! from game memory in one sequential pass and builds an `EntityCache`.
//!
//! Called by the worker immediately after `Connected`; also triggered by
//! the Reload Save button.  Sends `CacheProgress` every 5 000 entities so
//! the progress bar stays responsive.  Returns the completed cache wrapped
//! in an `Arc`; the worker sends `CacheReady(arc)`.

use std::collections::HashMap;
use std::sync::{mpsc, Arc};

use crate::cache::{CachedClub, CachedPlayer, CachedStaff, EntityCache};
use crate::fields;
use crate::process::FmProcess;

use super::cmd::BridgeResult;
use super::worker::ArrayCache;

const PROGRESS_INTERVAL: usize = 5_000;

pub(super) fn load_entity_cache(
    fm: &FmProcess,
    ac: &ArrayCache,
    tx: &mpsc::Sender<BridgeResult>,
) -> Result<Arc<EntityCache>, String> {
    let total = ac.humans_count + ac.staff_count + ac.clubs_count;
    let mut done: usize = 0;

    // ------------------------------------------------------------------
    // Resolve field offsets once — avoids repeated schema lookups in the
    // hot loops below.
    // ------------------------------------------------------------------

    // Player offsets (p_name not needed here — read_player_name handles both name blocks)
    let p_uid_off    = fields::field("p_uid") .ok_or("p_uid missing") ?.offset.ok_or("p_uid no offset") ?.offset();
    let p_ca_off     = fields::field("p_ca")  .ok_or("p_ca missing")  ?.offset.ok_or("p_ca no offset")  ?.offset();
    let p_pa_off     = fields::field("p_pa")  .ok_or("p_pa missing")  ?.offset.ok_or("p_pa no offset")  ?.offset();
    // p_dob is a DateDayYear field; year is the u16 at offset+2.
    let p_dob_year_off = fields::field("p_dob").ok_or("p_dob missing")?.offset.ok_or("p_dob no offset")?.offset() + 2;

    // Staff offsets (s_name not needed here — read_staff_name handles FmString + sub-record fallback)
    let s_id_off       = fields::field("s_id")      .ok_or("s_id missing")      ?.offset.ok_or("s_id no offset")      ?.offset();
    let s_ca_off       = fields::field("s_ca")      .ok_or("s_ca missing")      ?.offset.ok_or("s_ca no offset")      ?.offset();
    let s_pa_off       = fields::field("s_pa")      .ok_or("s_pa missing")      ?.offset.ok_or("s_pa no offset")      ?.offset();
    let s_dob_year_off = fields::field("s_dob")     .ok_or("s_dob missing")     ?.offset.ok_or("s_dob no offset")     ?.offset() + 2;
    // Optional: FM27 has no mapped role-ratings pointer.
    let s_role_ptr_off = fields::offset_of("s_role_ptr");

    // Club offsets
    let c_uid_off       = fields::field("c_uid")      .ok_or("c_uid missing")      ?.offset.ok_or("c_uid no offset")      ?.offset();
    let c_name_off      = fields::field("c_name")     .ok_or("c_name missing")     ?.offset.ok_or("c_name no offset")     ?.offset();
    let c_full_name_off = fields::field("c_full_name").ok_or("c_full_name missing")?.offset.ok_or("c_full_name no offset")?.offset();
    // Optional: FM24 has no mapped gender flag.
    let c_gender_off    = fields::offset_of("c_gender").map(|o| o as u64);

    let staff_size  = fm.schemas().staff_struct_size;

    // ------------------------------------------------------------------
    // Phase 1 — players
    // ------------------------------------------------------------------
    let mut players = Vec::with_capacity(ac.humans_count);

    for i in 0..ac.humans_count {
        let ptr = fm.read_u64(ac.humans_data + i as u64 * 8).unwrap_or(0);
        if ptr < 0x10000 || ptr > 0x7FFFFFFFFFFF {
            done += 1;
            continue;
        }

        let (raw, person_shift) = match crate::player::read_player_raw(fm, ptr) {
            Ok(r)  => r,
            Err(_) => { done += 1; continue; }
        };

        let uid      = read_u32_le(&raw, p_uid_off as usize);
        let ca       = raw.get(p_ca_off as usize).copied().unwrap_or(0);
        let pa       = raw.get(p_pa_off as usize).copied().unwrap_or(0);
        let dob_year = read_u16_le(&raw, p_dob_year_off as usize);

        let name = crate::player::read_player_name(fm, &raw).unwrap_or_default();

        players.push(CachedPlayer {
            addr: ptr, index: i, name, uid, ca, pa, dob_year, raw, person_shift,
            nation_id: None, club_id: None, value: None,
        });

        done += 1;
        if done % PROGRESS_INTERVAL == 0 {
            let _ = tx.send(BridgeResult::CacheProgress { done, total });
        }
    }

    // ------------------------------------------------------------------
    // Phase 2 — staff
    // ------------------------------------------------------------------
    let mut staff = Vec::with_capacity(ac.staff_count);

    for i in 0..ac.staff_count {
        let ptr = fm.read_u64(ac.staff_data + i as u64 * 8).unwrap_or(0);
        if ptr < 0x10000 || ptr > 0x7FFFFFFFFFFF {
            done += 1;
            continue;
        }

        let raw = match fm.read_bytes(ptr, staff_size) {
            Ok(b)  => b,
            Err(_) => { done += 1; continue; }
        };

        let uid      = read_u32_le(&raw, s_id_off as usize);
        let ca       = raw.get(s_ca_off as usize).copied().unwrap_or(0);
        let pa       = raw.get(s_pa_off as usize).copied().unwrap_or(0);
        let dob_year = read_u16_le(&raw, s_dob_year_off as usize);

        // Try s_name FmString → sub-records at +0x150/+0x158.
        // ~72% of staff have null FmString; sub-records supply their names.
        let name = crate::staff::read_staff_name(fm, &raw).unwrap_or_default();

        let role_ptr = s_role_ptr_off.map_or(0, |o| read_u64_le(&raw, o));
        let roles = if role_ptr > 0x10000 && role_ptr < 0x7FFFFFFFFFFF {
            fm.read_bytes(role_ptr, 15)
                .ok()
                .and_then(|b| <[u8; 15]>::try_from(b).ok())
                .unwrap_or([0u8; 15])
        } else {
            [0u8; 15]
        };

        staff.push(CachedStaff {
            addr: ptr, index: i, name, uid, ca, pa, dob_year,
            roles, role_ptr, raw,
            nation_id: None, club_id: None,
        });

        done += 1;
        if done % PROGRESS_INTERVAL == 0 {
            let _ = tx.send(BridgeResult::CacheProgress { done, total });
        }
    }

    // ------------------------------------------------------------------
    // Phase 3 — clubs
    // ------------------------------------------------------------------
    let mut clubs = Vec::with_capacity(ac.clubs_count);

    for i in 0..ac.clubs_count {
        let ptr = fm.read_u64(ac.clubs_data + i as u64 * 8).unwrap_or(0);
        if ptr < 0x10000 || ptr > 0x7FFFFFFFFFFF {
            done += 1;
            continue;
        }

        let uid    = fm.read_u32(ptr + c_uid_off).unwrap_or(0);
        let gender = c_gender_off.and_then(|o| fm.read_u8(ptr + o).ok()).unwrap_or(0);

        let short_ptr = fm.read_u64(ptr + c_name_off).unwrap_or(0);
        let full_ptr  = fm.read_u64(ptr + c_full_name_off).unwrap_or(0);
        let name = fm.read_fm_name(short_ptr)
            .or_else(|_| fm.read_fm_name(full_ptr))
            .unwrap_or_default();

        let transfer_budget = super::entity_load::read_club_field_u32(fm, ptr, "c_transfer_budget");

        clubs.push(CachedClub {
            addr: ptr, index: i, uid, name, gender, transfer_budget,
            nation_id: None, status: None, reputation: None,
        });

        done += 1;
        if done % PROGRESS_INTERVAL == 0 {
            let _ = tx.send(BridgeResult::CacheProgress { done, total });
        }
    }

    // ------------------------------------------------------------------
    // Build club_names lookup and wrap
    // ------------------------------------------------------------------
    let club_names: HashMap<u32, String> = clubs
        .iter()
        .map(|c| (c.uid, c.name.clone()))
        .collect();

    Ok(Arc::new(EntityCache { players, staff, clubs, club_names }))
}

// ---------------------------------------------------------------------------
// Byte-slice helpers — avoids pulling in byteorder for three simple reads
// ---------------------------------------------------------------------------

fn read_u16_le(buf: &[u8], off: usize) -> u16 {
    buf.get(off..off + 2)
        .and_then(|b| b.try_into().ok())
        .map(u16::from_le_bytes)
        .unwrap_or(0)
}

fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    buf.get(off..off + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .unwrap_or(0)
}

fn read_u64_le(buf: &[u8], off: usize) -> u64 {
    buf.get(off..off + 8)
        .and_then(|b| b.try_into().ok())
        .map(u64::from_le_bytes)
        .unwrap_or(0)
}
