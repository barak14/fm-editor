//! Scan the game module's static data for (data_ptr, end_ptr) pointer pairs
//! whose (end-data)/8 count matches the attached game's FMRTE reference
//! counts.
//!
//! Historically this tool was used to find the correct full-array RVAs for
//! clubs/players/staff/humans after discovering the existing RVAs pointed
//! at filtered subsets. Re-run whenever a game patch or a different build
//! (e.g. Game Pass) shifts the arrays — auto-detects the game via
//! `FmProcess::attach()` (pass `--pid <n>` when several fm.exe are running)
//! and pulls reference counts from that process's `VersionedSchemas`.
//!
//! The scan covers every writable, non-executable section of the module,
//! read from the PE section table in the process's memory, so it finds the
//! arrays wherever a build put them.

use fm_editor::process::FmProcess;

const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const IMAGE_SCN_MEM_WRITE: u32 = 0x8000_0000;

/// One PE section: name, RVA range, characteristics.
struct Section {
    name: String,
    start: u64,
    end: u64,
    flags: u32,
}

/// The module's section table, parsed from its PE header in memory.
fn sections(fm: &FmProcess) -> Result<Vec<Section>, String> {
    let head = fm.read_bytes(fm.gp_base, 0x1000)?;
    let u16_at = |o: usize| head.get(o..o + 2).map(|b| u16::from_le_bytes(b.try_into().unwrap()));
    let u32_at = |o: usize| head.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let pe = u32_at(0x3C).ok_or("short header")? as usize;
    if head.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err("no PE signature at the module base".into());
    }
    let count = u16_at(pe + 6).ok_or("short header")? as usize;
    let opt_size = u16_at(pe + 20).ok_or("short header")? as usize;
    let table = pe + 24 + opt_size;
    (0..count)
        .map(|i| {
            let s = table + i * 40;
            let raw = head.get(s..s + 8).ok_or("section table past header page")?;
            let name = String::from_utf8_lossy(raw).trim_end_matches('\0').to_string();
            let vsize = u32_at(s + 8).ok_or("short header")? as u64;
            let rva = u32_at(s + 12).ok_or("short header")? as u64;
            let flags = u32_at(s + 36).ok_or("short header")?;
            Ok(Section { name, start: rva, end: rva + vsize, flags })
        })
        .collect()
}

fn is_heap_ptr(p: u64) -> bool {
    (0x10000..0x7FFF_FFFF_FFFF).contains(&p)
}

fn main() {
    // `--pid <n>` picks one game when several fm.exe are running.
    let args: Vec<String> = std::env::args().collect();
    let pid = args
        .windows(2)
        .find(|w| w[0] == "--pid")
        .map(|w| w[1].parse::<u32>().map_err(|e| format!("--pid {}: {e}", w[1])));
    let attached = match pid {
        Some(Ok(pid)) => FmProcess::attach_pid(pid),
        Some(Err(e)) => Err(e),
        None => FmProcess::attach(),
    };
    let fm = match attached {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed: {e}");
            return;
        }
    };
    println!(
        "Attached: {}, pid {}, {} @ 0x{:X}",
        fm.build.display_name(), fm.pid, fm.module_path, fm.gp_base
    );

    let schema = fm.schemas();
    let gp_size = fm.gp_end - fm.gp_base;

    let all_sections = match sections(&fm) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read the module's section table: {e}");
            return;
        }
    };
    println!("\nSections:");
    let mut ranges = Vec::new();
    for s in &all_sections {
        let writable = s.flags & IMAGE_SCN_MEM_WRITE != 0;
        let exec = s.flags & IMAGE_SCN_MEM_EXECUTE != 0;
        let scanned = writable && !exec;
        println!(
            "  {:<8} gp+0x{:08X}..0x{:08X} ({:>4} MB) flags=0x{:08X} {}",
            s.name,
            s.start,
            s.end,
            (s.end - s.start) / (1024 * 1024),
            s.flags,
            if scanned { "scan" } else { "skip" },
        );
        if scanned {
            ranges.push((s.start, s.end.min(gp_size)));
        }
    }
    println!(
        "Schema {} RVAs: humans 0x{:X}, staff 0x{:X}, clubs 0x{:X}, stadium root 0x{:X}",
        fm.build.schema_name(),
        schema.rva_humans_data,
        schema.rva_staff_data,
        schema.rva_clubs_data,
        schema.rva_stadium_base,
    );
    let total: u64 = ranges.iter().map(|(s, e)| e.saturating_sub(*s)).sum();
    println!("\nScanning {} MB for pointer pairs...", total / (1024 * 1024));

    let fmrte_clubs = schema.fmrte_clubs;
    let fmrte_players = schema.fmrte_players;
    let fmrte_staff = schema.fmrte_staff;
    let fmrte_humans = schema.fmrte_humans;
    println!(
        "FMRTE reference counts for this version: clubs={fmrte_clubs} players={fmrte_players} staff={fmrte_staff} humans={fmrte_humans}"
    );

    let label_count = |count: u64| -> Option<String> {
        if count == fmrte_clubs {
            Some(format!("** FMRTE CLUBS ({fmrte_clubs}) **"))
        } else if count == fmrte_players {
            Some(format!("** FMRTE PLAYERS ({fmrte_players}) **"))
        } else if count == fmrte_staff {
            Some(format!("** FMRTE STAFF ({fmrte_staff}) **"))
        } else if count == fmrte_humans && count != 0 {
            Some(format!("** FMRTE HUMANS ({fmrte_humans}) **"))
        } else {
            None
        }
    };

    // Read in 1MB chunks
    const CHUNK: usize = 1024 * 1024;
    let mut matches_any: Vec<(u64, u64, u64, u64, String)> = Vec::new();
    let mut all_pairs: Vec<(u64, u64, u64, u64)> = Vec::new();

    for &(scan_start, scan_end) in &ranges {
        let mut rva = scan_start;
        while rva < scan_end {
            let want = CHUNK.min((scan_end - rva) as usize);
            let Ok(buf) = fm.read_bytes(fm.gp_base + rva, want) else {
                rva += CHUNK as u64;
                continue;
            };

            // Walk at 8-byte alignment, reading consecutive u64 pairs
            let mut off = 0usize;
            while off + 16 <= buf.len() {
                let a = u64::from_le_bytes(buf[off..off + 8].try_into().unwrap());
                let b = u64::from_le_bytes(buf[off + 8..off + 16].try_into().unwrap());
                if is_heap_ptr(a) && is_heap_ptr(b) && b > a {
                    let diff = b - a;
                    if diff % 8 == 0 && diff >= 8 && diff <= (10_000_000 * 8) {
                        let count = diff / 8;
                        if count >= 1 && count < 10_000_000 {
                            let pair_rva = rva + off as u64;
                            all_pairs.push((pair_rva, a, b, count));
                            if let Some(label) = label_count(count) {
                                matches_any.push((pair_rva, a, b, count, label));
                            }
                        }
                    }
                }
                off += 8;
            }

            rva += CHUNK as u64;
        }
    }

    println!(
        "\n=== Pairs matching a known count ({} hits) ===",
        matches_any.len()
    );
    for (pair_rva, a, b, count, label) in &matches_any {
        println!(
            "  gp+0x{pair_rva:08X}  data=0x{a:X}  end=0x{b:X}  count={count:>8}  {label}"
        );
    }

    println!(
        "\n=== All pairs with count in [10000, 1000000] ({} total candidate pairs scanned) ===",
        all_pairs.len()
    );
    let mut big: Vec<_> = all_pairs
        .iter()
        .filter(|(_, _, _, c)| (10_000..=1_000_000).contains(c))
        .collect();
    big.sort_by_key(|(rva, _, _, _)| *rva);
    for (pair_rva, a, b, count) in big {
        let hint = label_count(*count).unwrap_or_default();
        println!(
            "  gp+0x{pair_rva:08X}  data=0x{a:X}  end=0x{b:X}  count={count:>8}  {hint}"
        );
    }
}
