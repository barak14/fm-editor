//! Read club records from fm.exe via ptr_clubs.

use crate::process::FmProcess;

/// Read the club pointer array bounds.
pub fn clubs_array(fm: &FmProcess) -> Result<(u64, u64, usize), String> {
    let schema = fm.schemas();
    fm.read_ptr_array(schema.rva_clubs_data, schema.rva_clubs_end)
}

/// Find a club by UID and return its address + raw bytes.
pub fn find_club_by_uid(fm: &FmProcess, clubs_data: u64, count: usize, target_uid: u32) -> Result<Option<(usize, u64, Vec<u8>)>, String> {
    let schema = fm.schemas();
    let uid_offset = schema.field("c_uid").and_then(|f| f.offset).unwrap().offset();
    let uid_bytes = target_uid.to_le_bytes();
    for i in 0..count {
        let ptr = fm.read_u64(clubs_data + i as u64 * 8)?;
        if ptr < 0x10000 || ptr > 0x7FFFFFFFFFFF { continue; }
        let chunk = fm.read_bytes(ptr + uid_offset, 4)?;
        if chunk == uid_bytes {
            let raw = fm.read_bytes(ptr, schema.club_read_size)?;
            return Ok(Some((i, ptr, raw)));
        }
    }
    Ok(None)
}
