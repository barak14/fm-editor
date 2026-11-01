//! Bounded evidence capture. There is deliberately no writer or conversion API.

use serde_json::{Value, json};

use crate::debug::DebugState;
use crate::fields::fm27::contract::{ContractInspection, STEAM_INSPECTION};
use crate::fields::{Encoding, FieldLocation, SubRecordParent, VersionedSchemas};
use crate::game_version::GameBuild;
use crate::player::is_valid_ptr;

type ReadMemory<'a> = dyn Fn(u64, usize) -> Result<Vec<u8>, String> + 'a;

fn inspection(build: GameBuild, hash: u64) -> Result<&'static ContractInspection, String> {
    if build != GameBuild::Fm27Steam || hash != STEAM_INSPECTION.module_header_hash {
        return Err(
            "Contract inspection unsupported for this build/fingerprint; no conversion is enabled"
                .into(),
        );
    }
    Ok(&STEAM_INSPECTION)
}

fn range(base: u64, offset: usize, size: usize) -> Result<u64, String> {
    let address = base.checked_add(u64::try_from(offset).map_err(|_| "offset overflow")?);
    let last = address.and_then(|a| a.checked_add(u64::try_from(size.checked_sub(1)?).ok()?));
    match (address, last) {
        (Some(a), Some(b)) if is_valid_ptr(base) && is_valid_ptr(a) && is_valid_ptr(b) => Ok(a),
        _ => Err("Invalid inspection memory range".into()),
    }
}

fn read_exact(
    read: &ReadMemory<'_>,
    base: u64,
    offset: usize,
    size: usize,
) -> Result<Vec<u8>, String> {
    if size == 0 || size > 4096 {
        return Err("Inspection read must be bounded to 1..=4096 bytes".into());
    }
    let bytes = read(range(base, offset, size)?, size)?;
    if bytes.len() != size {
        return Err("Truncated inspection read".into());
    }
    Ok(bytes)
}

fn bytes_at<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], String> {
    bytes
        .get(offset..offset.checked_add(N).ok_or("offset overflow")?)
        .and_then(|b| b.try_into().ok())
        .ok_or("Truncated inspection field".into())
}

fn pointer(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let value = u64::from_le_bytes(bytes_at(bytes, offset)?);
    if !is_valid_ptr(value) {
        return Err("Invalid inspection pointer".into());
    }
    Ok(value)
}

fn direct_offset(
    s: &'static VersionedSchemas,
    key: &str,
    encoding: Encoding,
) -> Result<usize, String> {
    match s.field(key).and_then(|f| f.offset) {
        Some(FieldLocation::Direct {
            offset,
            encoding: e,
        }) if e == encoding => usize::try_from(offset).map_err(|_| "schema offset overflow".into()),
        _ => Err(format!("Unsupported inspection schema: {key}")),
    }
}

fn contract_bytes<'a>(
    s: &'static VersionedSchemas,
    raw: &'a [u8],
    key: &str,
    width: usize,
) -> Option<&'a [u8]> {
    let FieldLocation::SubRecord {
        container: "contract",
        offset,
        ..
    } = s.field(key)?.offset?
    else {
        return None;
    };
    let offset = usize::try_from(offset).ok()?;
    raw.get(offset..offset.checked_add(width)?)
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    player_raw: Vec<u8>,
    contract_address: u64,
    contract_raw: Vec<u8>,
    owner: u64,
}

/// Repeated reads detect observable races; this is not an atomic snapshot or a
/// reusable write/Undo authorization token. No mini-cache addresses are trusted.
fn capture(
    read: &ReadMemory<'_>,
    s: &'static VersionedSchemas,
    layout: &ContractInspection,
    module: u64,
    index: usize,
    address: u64,
    uid: u32,
) -> Result<Snapshot, String> {
    let data_at = range(
        module,
        usize::try_from(s.rva_humans_data).map_err(|_| "RVA overflow")?,
        8,
    )?;
    let end_at = range(
        module,
        usize::try_from(s.rva_humans_end).map_err(|_| "RVA overflow")?,
        8,
    )?;
    let data = read_exact(read, data_at, 0, 8)?;
    let end = read_exact(read, end_at, 0, 8)?;
    let begin = pointer(&data, 0)?;
    let end_ptr = pointer(&end, 0)?;
    let span = end_ptr.checked_sub(begin).ok_or("Reversed player array")?;
    let count = span / 8;
    if span % 8 != 0 || count > 1_000_000 || index as u64 >= count {
        return Err("Invalid player array/index; rediscover the player".into());
    }
    let slot = range(begin, index.checked_mul(8).ok_or("index overflow")?, 8)?;
    let slot_raw = read_exact(read, slot, 0, 8)?;
    if pointer(&slot_raw, 0)? != address {
        return Err("Stale player address/index; rediscover the player".into());
    }
    let raw = read_exact(read, address, 0, s.player_struct_size)?;
    let uid_offset = direct_offset(s, "p_uid", Encoding::RawU32)?;
    if uid == 0 || u32::from_le_bytes(bytes_at(&raw, uid_offset)?) != uid {
        return Err("Stale player UID; rediscover the player".into());
    }
    let contract = s
        .player_sub_records
        .iter()
        .find(|s| s.id == "contract")
        .ok_or("Unmapped contract")?;
    if !matches!(contract.parent, SubRecordParent::Direct) {
        return Err("Unsupported contract pointer chain".into());
    }
    let contract_offset = direct_offset(s, "p_contract_ptr", Encoding::Pointer)?;
    if contract_offset as u64 != contract.ptr_offset {
        return Err("Inconsistent contract schema".into());
    }
    let contract_address = pointer(&raw, contract_offset)?;
    let contract_raw = read_exact(read, contract_address, 0, contract.size)?;
    let expected_vtable = module
        .checked_add(layout.contract_vtable_rva)
        .ok_or("vtable overflow")?;
    if pointer(&contract_raw, layout.vtable_ptr_offset)? != expected_vtable {
        return Err("Contract class is not verified for inspection on this build".into());
    }
    let owner = pointer(&contract_raw, layout.owner_ptr_offset)?;
    if owner != range(address, s.player_person_offset, 1)? {
        return Err("Contract owner does not match the player's person object".into());
    }
    // Validate every retained byte, array root and membership slot again, even
    // when a reload happened to leave the UID and contract pointer unchanged.
    if read_exact(read, contract_address, 0, contract.size)? != contract_raw
        || read_exact(read, address, 0, s.player_struct_size)? != raw
        || read_exact(read, slot, 0, 8)? != slot_raw
        || read_exact(read, data_at, 0, 8)? != data
        || read_exact(read, end_at, 0, 8)? != end
    {
        return Err("Memory changed during inspection; rediscover and retry".into());
    }
    Ok(Snapshot {
        player_raw: raw,
        contract_address,
        contract_raw,
        owner,
    })
}

fn argument(args: &Value, key: &str) -> Result<u64, String> {
    args.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{key} must be an unsigned integer"))
}

fn name_at(read: &ReadMemory<'_>, object: u64, offset: usize) -> Result<String, String> {
    let ptr = pointer(&read_exact(read, object, offset, 8)?, 0)?;
    let length = u32::from_le_bytes(bytes_at(&read_exact(read, ptr, 0, 4)?, 0)?) as usize;
    if !(1..=256).contains(&length) {
        return Err("Invalid name length".into());
    }
    String::from_utf8(read_exact(read, ptr, 4, length)?).map_err(|e| e.to_string())
}

fn club_info(
    read: &ReadMemory<'_>,
    s: &'static VersionedSchemas,
    team: u64,
) -> Result<Value, String> {
    let club = s
        .player_sub_records
        .iter()
        .find(|r| r.id == "club")
        .filter(|r| matches!(r.parent, SubRecordParent::In("team")))
        .ok_or("Unmapped team club")?;
    let offset = usize::try_from(club.ptr_offset).map_err(|_| "club offset overflow")?;
    let address = pointer(&read_exact(read, team, offset, 8)?, 0)?;
    let name =
        direct_offset(s, "c_full_name", Encoding::FmString).and_then(|o| name_at(read, address, o));
    Ok(json!({"address":format!("0x{address:X}"),"name":name.ok()}))
}

pub fn run(args: Value, state: &DebugState) -> Result<String, String> {
    let pid = u32::try_from(argument(&args, "pid")?).map_err(|_| "PID out of range")?;
    let uid = u32::try_from(argument(&args, "expected_uid")?).map_err(|_| "UID out of range")?;
    let index = usize::try_from(argument(&args, "index")?).map_err(|_| "index out of range")?;
    let address = argument(&args, "expected_address")?;
    let guard = state
        .fm
        .lock()
        .map_err(|_| "debug attachment lock poisoned")?;
    let fm = guard
        .as_ref()
        .ok_or("Not attached. Call attach_status first")?;
    if pid != fm.pid {
        return Err("Stale attachment PID; call attach_status again".into());
    }
    let hash = fm.module_header_hash()?;
    let layout = inspection(fm.build, hash)?;
    if crate::player::person_shift(fm, address) != 0 {
        return Err("Player-coach contract inspection is not witnessed on this build".into());
    }
    let s = fm.schemas();
    let read = |a, n| fm.read_bytes(a, n);
    let snapshot = capture(&read, s, layout, fm.gp_base, index, address, uid)?;
    let raw = &snapshot.contract_raw;
    let contract_type = inspected_type(layout, raw);
    let hex = |bytes: &[u8]| super::Encoding::raw_hex(bytes, bytes.len());
    let date = |key| {
        contract_bytes(s, raw, key, 4).map(|b| json!({"raw_hex": hex(b), "decoded": crate::game_date::GameDate::from_bytes(b).map(|d| json!({"day":d.day,"year":d.year})), "sentinel_semantics_verified": false}))
    };
    let wage = contract_bytes(s, raw, "p_contract_wage", 4)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(u32::from_le_bytes);
    let current_team = direct_offset(s, "p_team_ptr", Encoding::Pointer)
        .ok()
        .and_then(|o| pointer(&snapshot.player_raw, o).ok());
    let contract_team = s
        .player_sub_records
        .iter()
        .find(|s| s.id == "contract_team")
        .filter(|s| matches!(s.parent, SubRecordParent::In("contract")))
        .and_then(|s| usize::try_from(s.ptr_offset).ok())
        .and_then(|o| pointer(raw, o).ok());
    let name = direct_offset(s, "p_name", Encoding::FmString)
        .and_then(|o| name_at(&read, address, o))
        .ok();
    let current_club = current_team.map(|t| club_info(&read, s, t));
    let owning_club = contract_team.map(|t| club_info(&read, s, t));
    // Names are supplemental pointer reads; reject if the bound record changed
    // while resolving them. This report still cannot be used as a write token.
    if capture(&read, s, layout, fm.gp_base, index, address, uid)? != snapshot {
        return Err("Record changed while resolving names; rediscover and retry".into());
    }
    let result = json!({
        "pid":pid, "build":fm.build.display_name(), "storefront":fm.build.storefront().display_name(),
        "module_path":fm.module_path, "module_header_hash":format!("0x{hash:016X}"),
        "schema_fingerprint_calibrated":s.known_dll_hash != 0, "compatibility_notice":fm.dll_hash_notice,
        "game_date":fm.game_date().map(|d| json!({"day":d.day,"year":d.year})),
        "index":index, "uid":uid, "name":name, "current_club":current_club, "owning_club":owning_club,
        "player_address":format!("0x{address:X}"), "player_raw_hex":hex(&snapshot.player_raw),
        "scope":"current_contract_only", "pending_offer":null, "future_contract":null,
        "contract_address":format!("0x{:X}",snapshot.contract_address), "owner_address":format!("0x{:X}",snapshot.owner),
        "owner_matches_person":true, "current_team_address":current_team.map(|a|format!("0x{a:X}")),
        "contract_team_address":contract_team.map(|a|format!("0x{a:X}")),
        "contract_raw_hex":hex(raw), "wage": {"raw_value":wage,"unit":"GBP/week", "sentinel_semantics_verified":false},
        "signed":date("p_contract_signed"), "expiry":date("p_contract_expiry"),
        "contract_type":contract_type, "conversion_supported":false,
        "limits":"Read-only repeated reads, not atomic. Only exact witnessed type-byte values are labelled; other values/bits remain unknown. Pending/future ownership and conversion requirements unverified. No write or Undo authorization. Wage/expiry do not establish contract type."
    });
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

fn inspected_type(layout: &ContractInspection, raw: &[u8]) -> Value {
    let value = raw.get(layout.type_byte_offset).copied();
    json!({
        "offset":format!("0x{:X}",layout.type_byte_offset), "width_bytes":1,
        "raw_value":value, "label":value.and_then(|v|layout.type_label(v)),
        "editable":false, "coverage":"Exact witnessed codes only; conversion unverified"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::BTreeMap;

    const MODULE: u64 = 0x10000000;
    const ARRAY: u64 = 0x20000000;
    const PLAYER: u64 = 0x30000000;
    const CONTRACT: u64 = 0x40000000;
    const UID: u32 = 2000144100;

    fn memory() -> BTreeMap<u64, Vec<u8>> {
        let s = GameBuild::Fm27Steam.schemas();
        let mut raw = vec![0xA5; s.player_struct_size];
        let uid = direct_offset(s, "p_uid", Encoding::RawU32).unwrap();
        raw[uid..uid + 4].copy_from_slice(&UID.to_le_bytes());
        let ptr = direct_offset(s, "p_contract_ptr", Encoding::Pointer).unwrap();
        raw[ptr..ptr + 8].copy_from_slice(&CONTRACT.to_le_bytes());
        let size = s
            .player_sub_records
            .iter()
            .find(|s| s.id == "contract")
            .unwrap()
            .size;
        let mut contract = vec![0xFF; size];
        let vtable = STEAM_INSPECTION.vtable_ptr_offset;
        contract[vtable..vtable + 8]
            .copy_from_slice(&(MODULE + STEAM_INSPECTION.contract_vtable_rva).to_le_bytes());
        let owner = STEAM_INSPECTION.owner_ptr_offset;
        contract[owner..owner + 8]
            .copy_from_slice(&(PLAYER + s.player_person_offset as u64).to_le_bytes());
        BTreeMap::from([
            (MODULE + s.rva_humans_data, ARRAY.to_le_bytes().to_vec()),
            (
                MODULE + s.rva_humans_end,
                (ARRAY + 8).to_le_bytes().to_vec(),
            ),
            (ARRAY, PLAYER.to_le_bytes().to_vec()),
            (PLAYER, raw),
            (CONTRACT, contract),
        ])
    }

    fn read(memory: &BTreeMap<u64, Vec<u8>>, a: u64, n: usize) -> Result<Vec<u8>, String> {
        memory
            .get(&a)
            .filter(|b| b.len() == n)
            .cloned()
            .ok_or("unreadable".into())
    }

    fn snap(
        read: &ReadMemory<'_>,
        index: usize,
        address: u64,
        uid: u32,
    ) -> Result<Snapshot, String> {
        capture(
            read,
            GameBuild::Fm27Steam.schemas(),
            &STEAM_INSPECTION,
            MODULE,
            index,
            address,
            uid,
        )
    }

    #[test]
    fn only_witnessed_steam_fingerprint_can_be_inspected() {
        for build in GameBuild::ALL {
            assert_eq!(
                inspection(build, STEAM_INSPECTION.module_header_hash).is_ok(),
                build == GameBuild::Fm27Steam
            );
            assert!(inspection(build, 0).is_err());
            assert!(inspection(build, STEAM_INSPECTION.module_header_hash ^ 1).is_err());
        }
        assert!(
            GameBuild::Fm27Steam
                .schemas()
                .field("p_contract_type")
                .unwrap()
                .offset
                .is_none()
        );
    }

    #[test]
    fn snapshot_preserves_every_byte_including_unknown_type_bits_and_date_flags() {
        let memory = memory();
        let before = memory.clone();
        let snapshot = snap(&|a, n| read(&memory, a, n), 0, PLAYER, UID).unwrap();
        assert_eq!(snapshot.contract_raw, memory[&CONTRACT]);
        assert_eq!(snapshot.player_raw, memory[&PLAYER]);
        assert_eq!(memory, before);
    }

    #[test]
    fn missing_truncated_and_foreign_owner_records_are_rejected() {
        for at in memory().keys().copied().collect::<Vec<_>>() {
            let mut missing = memory();
            missing.remove(&at);
            assert!(snap(&|a, n| read(&missing, a, n), 0, PLAYER, UID).is_err());
            let complete = memory();
            assert!(
                snap(
                    &|a, n| {
                        let mut b = read(&complete, a, n)?;
                        if a == at {
                            b.pop();
                        }
                        Ok(b)
                    },
                    0,
                    PLAYER,
                    UID
                )
                .is_err()
            );
        }
        for owner in [0u64, 0x10000, 0x7FFFFFFFFFFF, PLAYER + 1] {
            let mut foreign = memory();
            let off = STEAM_INSPECTION.owner_ptr_offset;
            foreign.get_mut(&CONTRACT).unwrap()[off..off + 8].copy_from_slice(&owner.to_le_bytes());
            assert!(snap(&|a, n| read(&foreign, a, n), 0, PLAYER, UID).is_err());
        }
    }

    #[test]
    fn stale_uid_address_index_and_observable_reload_are_rejected() {
        let memory = memory();
        let reader = |a, n| read(&memory, a, n);
        assert!(snap(&reader, 0, PLAYER, UID + 1).is_err());
        assert!(snap(&reader, 0, PLAYER + 8, UID).is_err());
        assert!(snap(&reader, 1, PLAYER, UID).is_err());
        for at in memory.keys() {
            let calls = Cell::new(0);
            let changing = |a, n| {
                let mut b = read(&memory, a, n)?;
                if a == *at {
                    let call = calls.get();
                    calls.set(call + 1);
                    if call > 0 {
                        b[0] ^= 1;
                    }
                }
                Ok(b)
            };
            assert!(snap(&changing, 0, PLAYER, UID).is_err(), "0x{at:X}");
        }
    }

    #[test]
    fn invalid_ranges_never_reach_the_reader() {
        for (base, offset, n) in [
            (0, 0, 8),
            (0x10000, 0, 8),
            (0x20000, usize::MAX, 8),
            (0x7FFFFFFFFFFE, 0, 8),
            (0x20000, 0, 4097),
            (0x20000, 0, 0),
        ] {
            assert!(read_exact(&|_, _| panic!("invalid read"), base, offset, n).is_err());
        }
    }

    #[test]
    fn only_exact_witnessed_type_bytes_are_labelled_without_mutation() {
        for value in 0u8..=255 {
            let mut bytes = vec![0xA5; STEAM_INSPECTION.type_byte_offset + 1];
            bytes[STEAM_INSPECTION.type_byte_offset] = value;
            let before = bytes.clone();
            let result = inspected_type(&STEAM_INSPECTION, &bytes);
            let expected = match value {
                0 => Some("Part Time Contract"),
                1 => Some("Full Time Contract"),
                4 => Some("Non-Contract"),
                _ => None,
            };
            assert_eq!(result["label"].as_str(), expected);
            assert_eq!(result["raw_value"].as_u64(), Some(u64::from(value)));
            assert_eq!(result["editable"], false);
            assert_eq!(bytes, before);
        }
        let truncated = inspected_type(&STEAM_INSPECTION, &[]);
        assert!(truncated["raw_value"].is_null());
        assert!(truncated["label"].is_null());
    }

    #[test]
    fn foreign_contract_classes_are_rejected_before_decoding_type() {
        let mut foreign = memory();
        let vtable = STEAM_INSPECTION.vtable_ptr_offset;
        foreign.get_mut(&CONTRACT).unwrap()[vtable..vtable + 8]
            .copy_from_slice(&0x50000000u64.to_le_bytes());
        assert!(
            snap(&|a, n| read(&foreign, a, n), 0, PLAYER, UID)
                .unwrap_err()
                .contains("class")
        );
    }
}
