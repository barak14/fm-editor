//! Scan game_plugin.dll's static data for (data_ptr, end_ptr) pointer pairs
//! whose (end-data)/8 count matches the active schema's FMRTE reference
//! counts.
//!
//! Historically this tool was used to find the correct full-array RVAs for
//! clubs/players/staff/humans after discovering the existing RVAs pointed
//! at filtered subsets. Re-run whenever a game version bump shifts the
//! arrays — auto-detects which game is attached via `FmProcess::attach()`
//! and pulls reference counts from the active `VersionedSchemas`.

use fm_editor::process::FmProcess;

fn is_heap_ptr(p: u64) -> bool {
    (0x10000..0x7FFF_FFFF_FFFF).contains(&p)
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
    let gp_size = fm.gp_end - fm.gp_base;

    // Scan window — we know the existing cluster is around +0x4DF9D00..+0x4DFA050
    // on FM26. Widen generously on both sides to catch parallel pointer clusters.
    // The whole game_plugin.dll is ~472MB; scanning 64MB around the cluster is
    // a reasonable compromise (takes a few seconds).
    let scan_start: u64 = 0x4000000;
    let scan_end: u64 = 0x5400000u64.min(gp_size);
    println!(
        "\nScanning gp+0x{scan_start:X}..0x{scan_end:X} ({} MB) for pointer pairs...",
        (scan_end - scan_start) / (1024 * 1024)
    );

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
