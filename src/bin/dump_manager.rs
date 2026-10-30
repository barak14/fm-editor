//! Dump the human manager struct (last entry of ptr_staff) and try to locate
//! the name pointer + personality block.
//!
//! Previous approach (gp+0x4DE7A08 singleton pointer) was wrong — see
//! bridge/entity_load.rs::load_human_manager() for the canonical
//! discovery path.
//!
//! Auto-detects which game is running via `FmProcess::attach()`: FM26
//! first, FM24 as fallback. RVAs and struct sizes come from the active
//! `VersionedSchemas` so the dump still works when a future version bump
//! shifts them.

use fm_editor::process::FmProcess;

/// How many bytes to dump from the manager struct. Standard staff is 576, but
/// if fields live outside that we want to see them too.
const DUMP_SIZE: usize = 2048;

fn hex_dump(data: &[u8], base_offset: usize) {
    for (i, chunk) in data.chunks(16).enumerate() {
        let off = base_offset + i * 16;
        print!("  +0x{off:04X} | ");
        for (j, &b) in chunk.iter().enumerate() {
            if j == 8 {
                print!(" ");
            }
            print!("{b:02X} ");
        }
        for j in chunk.len()..16 {
            if j == 8 {
                print!(" ");
            }
            print!("   ");
        }
        print!("| ");
        for &b in chunk {
            if (0x20..0x7F).contains(&b) {
                print!("{}", b as char);
            } else {
                print!(".");
            }
        }
        println!();
    }
}

/// Print every offset where `val` appears as a byte.
fn find_byte(data: &[u8], val: u8, label: &str) {
    let hits: Vec<usize> = data.iter().enumerate().filter(|&(_, &b)| b == val).map(|(i, _)| i).collect();
    if hits.is_empty() {
        println!("  {label} ({val}): no hits");
    } else if hits.len() <= 60 {
        print!("  {label} ({val}): {} hits at ", hits.len());
        for (i, h) in hits.iter().enumerate() {
            if i > 0 { print!(", "); }
            print!("+0x{h:03X}");
        }
        println!();
    } else {
        println!("  {label} ({val}): {} hits (too many)", hits.len());
    }
}

/// Find windows of 8 consecutive bytes where ALL bytes are in 1..=20 AND the
/// window contains both `a` and `b`. Personality candidate ranges.
fn find_personality_windows(data: &[u8], a: u8, b: u8) {
    println!("  Scanning for 8-byte windows with all bytes in 1..=20, containing {a} and {b}:");
    let mut found = 0;
    for off in 0..data.len().saturating_sub(8) {
        let w = &data[off..off + 8];
        if w.iter().all(|&b| (1..=20).contains(&b)) && w.contains(&a) && w.contains(&b) {
            println!("    +0x{off:03X} = {:?}", w);
            found += 1;
            if found > 20 {
                println!("    ... (more, stopping)");
                break;
            }
        }
    }
    if found == 0 {
        println!("    (none)");
    }
}

/// For every 8-byte-aligned u64 in `data`, try to dereference it as an fm_name.
/// Report any that successfully resolve.
fn scan_name_pointers(fm: &FmProcess, data: &[u8]) {
    println!("  Scanning 8-byte-aligned u64 fields for valid fm_name pointers:");
    let mut found = 0;
    let mut off = 0;
    while off + 8 <= data.len() {
        let ptr = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        if let Ok(name) = fm.read_fm_name(ptr) {
            println!("    +0x{off:03X} -> 0x{ptr:X} -> {:?}", name);
            found += 1;
        }
        off += 8;
    }
    if found == 0 {
        println!("    (no valid name pointers found)");
    }
}

/// For every 8-byte-aligned u64 in `data`, try to read 8 bytes at the pointer
/// and report if they look like a personality block (all bytes in 1..=20 and
/// contain both `a` and `b`).
fn scan_personality_via_subrecord(fm: &FmProcess, data: &[u8], a: u8, b: u8) {
    println!("  Scanning sub-record pointers for 8-byte personality blocks containing {a} and {b}:");
    let mut found = 0;
    let mut off = 0;
    while off + 8 <= data.len() {
        let ptr = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        if (0x10000..0x7FFF_FFFF_FFFF).contains(&ptr) {
            // Read a generous window around the pointer so a personality block
            // anywhere in the first 512 bytes of the sub-record is visible.
            if let Ok(sub) = fm.read_bytes(ptr, 512) {
                for sub_off in 0..sub.len().saturating_sub(8) {
                    let w = &sub[sub_off..sub_off + 8];
                    if w.iter().all(|&b| (1..=20).contains(&b)) && w.contains(&a) && w.contains(&b) {
                        println!(
                            "    struct+0x{off:03X} -> 0x{ptr:X} + 0x{sub_off:03X} = {:?}",
                            w
                        );
                        found += 1;
                        if found > 40 {
                            println!("    ... (more, stopping)");
                            return;
                        }
                    }
                }
            }
        }
        off += 8;
    }
    if found == 0 {
        println!("    (none)");
    }
}

fn main() {
    let fm = match FmProcess::attach() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed: {e}");
            return;
        }
    };
    println!(
        "Attached: version {:?}, pid {}, game_plugin.dll @ 0x{:X}",
        fm.version, fm.pid, fm.gp_base
    );

    let schema = fm.schemas();

    // Canonical discovery: last entry of ptr_staff.
    let staff_data = fm.read_u64(fm.gp_base + schema.rva_staff_data).unwrap();
    let staff_end = fm.read_u64(fm.gp_base + schema.rva_staff_end).unwrap();
    let count = (staff_end - staff_data) / 8;
    println!(
        "\nptr_staff: data=0x{staff_data:X} end=0x{staff_end:X} count={count}"
    );
    let mgr_ptr = fm.read_u64(staff_end - 8).unwrap();
    println!("manager (last entry) ptr = 0x{mgr_ptr:X}");

    let raw = fm.read_bytes(mgr_ptr, DUMP_SIZE).unwrap();

    // Resolve staff field offsets from the schema so this tool follows
    // whichever game version is attached. Schema-driven lookups replace the
    // earlier FM26-hardcoded sanity-check prints.
    let off_of = |key: &str| -> Option<usize> {
        schema.field(key).and_then(|f| f.offset).map(|l| l.offset() as usize)
    };
    let home_rep = off_of("s_home_rep");
    let curr_rep = off_of("s_current_rep");
    let world_rep = off_of("s_world_rep");
    let ca_off = off_of("s_ca");
    let pa_off = off_of("s_pa");
    let id_off = off_of("s_id");
    let dob_off = off_of("s_dob");
    let name_off = off_of("s_name");
    let role_off = off_of("s_role_ptr");
    // Personality is a contiguous block of 8 direct bytes starting at
    // the first personality field. Use `s_adaptability` as the anchor
    // (same layout convention as the player personality block).
    let pers_off = off_of("s_adaptability");

    let read_u16 = |off: usize| -> u16 {
        u16::from_le_bytes([raw[off], raw[off + 1]])
    };
    let read_u32 = |off: usize| -> u32 {
        u32::from_le_bytes([raw[off], raw[off + 1], raw[off + 2], raw[off + 3]])
    };
    let read_u64 = |off: usize| -> u64 {
        u64::from_le_bytes(raw[off..off + 8].try_into().unwrap())
    };

    println!("\n=== Sanity check (standard staff offsets from schema) ===");
    if let Some(off) = home_rep  { println!("  +0x{off:03X} home rep  = {}", read_u16(off)); }
    if let Some(off) = curr_rep  { println!("  +0x{off:03X} curr rep  = {}", read_u16(off)); }
    if let Some(off) = world_rep { println!("  +0x{off:03X} world rep = {}", read_u16(off)); }
    if let Some(off) = ca_off    { println!("  +0x{off:03X} CA        = {}", raw[off]); }
    if let Some(off) = pa_off    { println!("  +0x{off:03X} PA        = {}", raw[off]); }
    if let Some(off) = id_off    { println!("  +0x{off:03X} id        = {}", read_u32(off)); }
    if let Some(off) = dob_off {
        println!("  +0x{off:03X} DOB day   = {}", read_u16(off));
        println!("  +0x{:03X} DOB year  = {}", off + 2, read_u16(off + 2));
    }
    if let Some(off) = role_off  { println!("  +0x{off:03X} role ptr  = 0x{:X}", read_u64(off)); }

    if let Some(off) = pers_off {
        println!("\n=== Current personality read (+0x{off:03X}..+0x{:03X}) ===", off + 7);
        for i in 0..8 {
            println!("  +0x{:03X} = {}", off + i, raw[off + i]);
        }
    }

    if let Some(off) = name_off {
        println!("\n=== Current name read (+0x{off:03X}) ===");
        let name_ptr = read_u64(off);
        println!("  +0x{off:03X} ptr = 0x{name_ptr:X}");
        match fm.read_fm_name(name_ptr) {
            Ok(n) => println!("  -> {:?}", n),
            Err(_) => println!("  -> invalid / null"),
        }
    }

    // Hypothesis: human manager has a SECOND staff-like block shifted by the
    // version's human_manager_shift relative to the standard AI staff layout.
    // All shifted offsets come from standard_offset + schema.human_manager_shift.
    let shift = schema.human_manager_shift;
    println!("\n=== Shifted staff block (+0x{shift:03X}) ===");

    if let Some(off) = id_off {
        let s = off + shift;
        println!("  +0x{s:03X} id        = {}", read_u32(s));
    }
    if let Some(off) = name_off {
        let s = off + shift;
        let name_ptr = read_u64(s);
        println!("  +0x{s:03X} name ptr  = 0x{name_ptr:X}");
        match fm.read_fm_name(name_ptr) {
            Ok(n) => println!("    -> {:?}", n),
            Err(_) => println!("    -> (failed to read as fm_name)"),
        }
    }
    if let Some(off) = pers_off {
        let s = off + shift;
        println!(
            "  +0x{s:03X} personality = [{}, {}, {}, {}, {}, {}, {}, {}]",
            raw[s], raw[s + 1], raw[s + 2], raw[s + 3],
            raw[s + 4], raw[s + 5], raw[s + 6], raw[s + 7]
        );
    }
    if let Some(off) = dob_off {
        let s = off + shift;
        println!("  +0x{s:03X} DOB day  = {}", read_u16(s));
        println!("  +0x{:03X} DOB year = {}", s + 2, read_u16(s + 2));
    }
    if let Some(off) = role_off {
        let s = off + shift;
        let role_ptr = read_u64(s);
        println!("  +0x{s:03X} role ptr  = 0x{role_ptr:X}");
        if let Ok(roles) = fm.read_bytes(role_ptr, 15) {
            println!(
                "    -> roles = [{}]",
                roles.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", ")
            );
        } else {
            println!("    -> (failed to read role sub-record)");
        }
    }

    println!("\n=== Known-value byte search ===");
    find_byte(&raw, 10, "Ambition=10");
    find_byte(&raw, 6, "Controversy=6");
    find_byte(&raw, 145, "CA=145");
    find_byte(&raw, 200, "PA=200");

    println!("\n=== Personality window search (main struct) ===");
    // Ambition=10 (index 1) and Controversy=6 (index 7). Every other slot is
    // unknown but must be in 1..=20.
    find_personality_windows(&raw, 10, 6);

    println!("\n=== Name pointer scan (main struct) ===");
    scan_name_pointers(&fm, &raw);

    println!("\n=== Sub-record personality scan ===");
    scan_personality_via_subrecord(&fm, &raw, 10, 6);

    println!("\n=== Hex dump (0x000..0x200) ===");
    hex_dump(&raw[..0x200], 0);

    println!("\n=== Hex dump (0x200..0x400) ===");
    hex_dump(&raw[0x200..0x400], 0x200);

    println!("\n=== Hex dump (0x400..0x600) ===");
    hex_dump(&raw[0x400..0x600], 0x400);

    println!("\n=== Hex dump (0x600..0x800) ===");
    hex_dump(&raw[0x600..0x800], 0x600);

    println!("\nDone.");
}
