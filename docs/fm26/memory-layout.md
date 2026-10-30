# fm-editor — Memory Layout Notes

This file holds the reverse-engineering context that **isn't** in `src/fields/`.

The code is the source of truth for every confirmed field offset and encoding — `src/fields/{player,staff,club}.rs` define them, `src/offsets.rs::rva` holds the array RVAs, and the various reader modules wire them up. Everything in here is supplementary: discovery context, witnesses to verify against, things we know exist but haven't located yet, and the special cases that don't fit a flat `Field { offset, encoding }` representation.

If you want to look up "where is the Crossing attribute stored?" — read `src/fields/player.rs`, not this file. If you want to look up "what's still unmapped, who can I verify against, and what's already been searched without success?" — that's what this file is for.

## Entity arrays

Confirmed RVAs are in `src/fields/fm26/mod.rs` (`FM26`). This is the discovery context for those.

**2026-10 patch.** Every RVA moved by a constant **+0x4F7F0** (e.g. players 0x4DF9E88 → 0x4E49678). The player pair was re-found by `Tools/scan-arrays.sh` (exact 542102 match); staff, clubs and the stadium container were found by applying the same delta and confirmed against witnesses (Harry Kane, Josep Guardiola, FC Bayern, Tottenham). Field offsets inside player, staff and club structs did not move. If a future patch breaks attach with "bad pointer array", rerun `scan-arrays.sh`, find the players pair, and try the same constant-delta trick for the rest.

| Entity | Count | Array RVA (data/end) |
|--------|-------|---------------------|
| Players | 542,102 | gp+0x4E49678 / +0x4E49680 |
| Staff | 193,051 | gp+0x4E496C0 / +0x4E496C8 |
| Clubs | 48,011 | gp+0x4E49830 / +0x4E49838 |
| Stadiums | 41,621 | gp+0x4E495F8 (container chain, see below) |

**Player RVA discovery.** A pair 0x18 bytes after the player RVA gives ~496k entries — it points to a *filtered subset* whose filter we haven't characterized. The full player array that matches both FMLE's and FMRTE's player counts (542102) is the std::vector begin/end/cap at the player RVA (begin at +0x00, end at +0x08, capacity at +0x10). Found via `src/bin/scan_arrays.rs` scanning the data segment for any (data_ptr, end_ptr) pair whose `(end-data)/8` count matches a target value.

**FMRTE 113894 "teams" vs our 48008 clubs.** Expected — a club is the top-level entity (name, stadium, finances) and each club can have multiple teams (senior, U23, U18, B-team, etc.). The club RVA gives the senior-club count, which is what we want for the club search. If we ever need the per-team data, scan for the 113894-count array specifically.

## Encodings — derivation

The encoding constants are in `src/fields::Encoding` (re-exported from `src/fields/mod.rs`) and decode/encode logic is in `FieldLocation::{decode_u8, encode_u8}`. The formulas:

- **Encoded attributes** (`displayed = (raw + 2) / 5`). Most player attributes and the staff "encoded block" use this. The game stores 0..98 internally and divides by 5 to get the 1..20 display range, with +2 to round-to-nearest. Inverse: `raw = displayed * 5 - 2`. This is lossy — `decode(encode(v)) != v` for most values, so emitters should only write when the *displayed* value changed (see `field_widget::attr_edit`).
- **Direct attributes** (`displayed == raw`). Player positions, personality, role ratings, and the staff "direct block" all use raw 1..20.
- **Dates** (`[u16 day_of_year][u16 year]` little-endian pair). Day-of-year is 1..366; converting to/from day-of-month is in `field_widget::dob_edit`.
- **FM strings** (`u64 ptr → [u32 len][UTF-8 bytes]`). Pointer can be null — for player names ~25% use a direct `p_name` FmString; ~65% use block-1 sub-records; ~10% have null block-1 and use a secondary block-2 FmString/sub-records. See "Player name sub-records" below.
- **Name-component sub-records** — see the dedicated section below.
- **PPM bitmask** (`u64`, bit N = PPM ID N+1). 64 possible PPMs. Bits 49-50 are career preferences ("Will Retire At Top", "Will Play Football As Long As Possible") — stored in the same bitmask but not shown on the in-game traits screen. Bits 62-63 are "Demands To Take Free Kicks" / "Demands To Take Penalties" — hidden slots in the original PPM table.

## Human manager — hybrid +0x350 shift

**The human manager is the LAST entry of the `ptr_staff` array.** He is NOT reachable via any static `game_plugin.dll` pointer.

Canonical discovery path:
```
data    = *(gp + STAFF_DATA)        // gp+0x4DF9ED0
end     = *(gp + STAFF_END)         // gp+0x4DF9ED8
manager = *(end - 8)                // last 8 bytes of the array
```

**Why the "last entry" works:** the staff array is pre-populated with AI staff (180,061 entries) at world generation, and the human manager is appended as the final entry when a save is created. FMLE uses this same path.

The human manager struct is **larger** than a standard 576-byte AI staff and has a hybrid layout. The first ~576 bytes mostly match the AI staff layout — attributes, CA/PA, and reputations are all at standard offsets — **but the "identity" fields are zeroed at their standard offsets and instead duplicated at `standard_offset + 0x350`**:

| Field | Standard offset | Manager offset |
|-------|----------------|----------------|
| Attributes (direct + encoded) | +0x010..+0x043 | **same** |
| Person base (reps, CA, PA) | +0x0CE..+0x0E0 | **same** |
| Staff ID + dup | +0x10C / +0x110 | **+0x45C / +0x460** |
| Name pointer | +0x140 | **+0x490** (null at +0x140) |
| Personality | +0x170..+0x177 | **+0x4C0..+0x4C7** (filler `[1,1,1,1,1,1,1,1]` at +0x170) |
| DOB day + year | +0x188 / +0x18A | **+0x4D8 / +0x4DA** |
| Role sub-record ptr | +0x1D0 | **+0x520** (null at +0x1D0) |

**Implementation.** `bridge::entity_load::load_human_manager` reads `STAFF_STRUCT_SIZE + 0x350` bytes, then copies each shifted range back to its standard offset before constructing the `StaffRecord`. After this patch the buffer can be treated as a normal staff struct by generic reader code. Writes are redirected by `staff_panel::show_manager` — for any `FieldEdit` whose relative offset falls inside a shifted range (per `bridge::MANAGER_SHIFTED_FIELDS`), `HUMAN_MANAGER_SHIFT` is added to the target address before the write goes through.

**Validator gotcha for `find_human_manager`.** Check CA/PA/reps at standard offsets AND check that the SHIFTED `name_ptr` (+0x490) and SHIFTED `role_ptr` (+0x520, with Manager role > 0) are populated. Checking the standard +0x140 / +0x1D0 slots would reject the real manager (they're null on the human) and pick a random AI staff record instead — this was the original bug.

**RVAs that were WRONG in early notes** (do not trust without dereferencing):
- `gp+0x4DE7A08` — does not point to the manager. Points to an unrelated staff struct.
- `gp+0x4DF9EB8 / +0x4DF9EC0` — same.

A live memory scan for the human manager's reputation bytes `28 23 3F 1F 88 13` at struct +0xD4 found the real manager at a heap address that doesn't appear anywhere in `game_plugin.dll`. A full scan of 472MB found zero static pointers to the manager struct.

**Verified against the human manager** (test save): CA=145, PA=200, home rep=9000, current rep=7999, world rep=5000, DOB verified, wage=67500, manager role=20, club=Tottenham, nationality=England, personality [14,10,10,10,10,10,10,6] (Adaptability, Ambition, Loyalty, Pressure, Professionalism, Sportsmanship, Temperament, Controversy).

## Club sub-records (not yet in src/fields/)

`src/fields/` doesn't have a representation for "field at offset X within a sub-record reached via pointer Y". Until it does, the sub-record offsets live here.

### Info sub-record (via ptr at club+0x100)

| Sub-offset | Size | Field |
|-----------|------|-------|
| +0x000 | str | Abbreviation 1 (e.g. "FCB", "Spurs") |
| +0x008 | str | Abbreviation 2 (e.g. "FCB", "TOT") |
| +0x010 | str | Abbreviation 3 (e.g. "BAY", "THFC") |
| +0x0B8 | u32 | Season Tickets |
| +0x0D0 | u32 | Year Founded |
| +0x0D6 | u16 | Owner Status |
| +0x0F0 | u32 | Social Media followers (approx) |
| +0x118 | u8 | Youth Facilities (direct 1-20) |
| +0x11E | u8 | Youth Improvement (direct 1-20) |
| +0x123 | u8 | Training (direct 1-20) |
| +0x124 | u8 | Youth Coaching (direct 1-20) |
| +0x125 | u8 | Youth Recruitment (direct 1-20) |

### Finance sub-record (via ptr at club+0x150)

| Sub-offset | Size | Field |
|-----------|------|-------|
| +0x014 | u32 | Balance |
| +0x018 | f32 | Avg Ticket Price |
| +0x01C | f32 | Avg Season Ticket Price |

Budget sub-sub-record (via ptr at finance+0x048):

| Sub-offset | Size | Field |
|-----------|------|-------|
| +0x004 | u32 | Transfer Budget |
| +0x008 | u32 | Transfer Budget (remaining) |
| +0x00C | u32 | Wage Budget |

### Stadium sub-record (via ptr at club+0xB0)

Despite the name this is the club's attendance/fan record, not the stadium entity.

| Sub-offset | Size | Field |
|-----------|------|-------|
| +0x068 | u32 | Average Attendance |
| +0x06C | u32 | Minimum Attendance |
| +0x070 | u32 | Maximum Attendance |
| +0x090 | u64 ptr→FmString | Club nickname (`c_nickname`) |

### Stadium entities (2026-10 patch layout)

Flat inline array of 0xC0-byte entities: `GP+0x4E495F8 → container → +0x80 → type_desc → vec_hdr → data_ptr`, `entity[i] = data_ptr + i*0xC0`. 41,621 contiguous entities. Before the patch the stride was 0x170 with a +0x40 header; every field offset changed.

**Club → home stadium:** `club+0x18` (team vector data) → `[0]` senior team → `+0x78` stadium entity. Schema sub-records `teams` / `senior_team` / `home_stadium`. Resolves for 1779/2000 sampled clubs; the rest are null. `club+0x140` is NOT the home stadium — it points at the training ground (Bayern → Säbener Straße, Spurs → Hotspur Way, Madrid → Ciudad Deportiva de Valdebebas).

Field offsets (`src/fields/fm26/stadium.rs`), witnessed against Allianz-Arena, Tottenham Hotspur Stadium, Etihad, Bernabéu, Wembley, MCG and Michigan: UID +0x0C, name FmString +0x40, alt name +0x48, capacity/seated/used/all-seater/expansion u32 at +0x7C/+0x80/+0x84/+0x88/+0x8C, owner club ptr +0x90 (null for non-club owners), pitch length/width u16 ×10 at +0x98/+0x9A. Standing, environment and condition are unmapped in the new layout.

### Staff role ratings sub-record (via ptr at staff+0x1D0)

15 bytes at sub+0x000, direct 1-20. Null pointer for medical-only staff. Position 8 appears to be an internal/calculated value (Guardiola=15, all others=1) — not displayed in the in-game UI; `staff_panel::ROLE_LABELS` skips it.

| Sub-offset | Role |
|-----------|------|
| +0x00 | Manager |
| +0x01 | Asst Manager |
| +0x02 | Coach |
| +0x03 | Physio |
| +0x04 | Scout |
| +0x05 | GK Coach |
| +0x06 | Fitness Coach |
| +0x07 | Set Piece Coach |
| +0x08 | (internal) |
| +0x09 | Director of Football |
| +0x0A | Head of Youth Dev |
| +0x0B | Loan Manager |
| +0x0C | Data Analyst |
| +0x0D | Sports Scientist |
| +0x0E | Technical Director |

## Player name sub-records

Players have **two name blocks**, tried in order:

| Block | Offset | Field key | Contents |
|-------|--------|-----------|----------|
| 1 | `+0x2C8` | `p_name` | `u64 → [u32 len][UTF-8]` direct full name (nullable) |
| 1 | `+0x2D8` | `p_name_first_sub` | `u64 → 5×24-byte first-name candidate list` |
| 1 | `+0x2E0` | `p_name_last_sub`  | `u64 → 5×24-byte last-name candidate list` |
| 2 | `+0x3C0` | `p_name2` | `u64 → [u32 len][UTF-8]` secondary direct name (nullable) |
| 2 | `+0x3D0` | `p_name2_first_sub` | `u64 → 5×24-byte first-name candidate list` |
| 2 | `+0x3D8` | `p_name2_last_sub`  | `u64 → 5×24-byte last-name candidate list` |

Reading order: try block-1 FmString → block-1 sub-records → block-2 FmString → block-2 sub-records.

In a 50k-human save (~2000 sampled): ~25% use block-1 FmString, ~65% use block-1 sub-records, ~10% have null block-1 ptrs and require block-2. All 100% have a name after applying both blocks.

**Each 24-byte sub-record entry layout:**

```
[u64 ptr][u32 id][u16 nation][u16 type/score][u32 flags][u32 pad]
```

`ptr` points **directly** at the `name_len` field inside a 32-byte inline name-component struct (i.e. `ptr == struct_base + 12`):

```
[u32 size_or_tag][u32 nation_id][u32 type][u32 name_len][UTF-8 name_bytes (padded to 16)]
```

Reading: `name_len = read_u32(ptr)`, `name_bytes = read_bytes(ptr+4, name_len)`.

The **first entry** (index 0) is the chosen name; subsequent entries are alternates ranked by frequency score. Display name = `first_name_candidates[0] + " " + last_name_candidates[0]`.

**Witnesses verified:**

| Player | UID | Block | Direct name | first_sub[0] | last_sub[0] | Combined |
|--------|-----|-------|-------------|--------------|-------------|---------|
| Litmanen | 352 | 1 | "Jari Olavi Litmanen" | "Jari" | "Litmanen" | "Jari Litmanen" |
| Kane | 28049320 | 1 | "Harry Edward Kane" | "Harry" | "Kane" | "Harry Kane" |
| Leif Arne Brekke | 484 | 1 | (null) | "Leif Arne" | "Brekke" | "Leif Arne Brekke" |
| Gary Hamilton | 302711825 | 2 | (null in blk1) | "Gary" | "Hamilton" | "Gary Hamilton" |
| James Henry | 118227982 | 2 | (null in blk1) | "James" | "Henry" | "James Henry" |
| Tommi Björn Paavola | 286131467 | 2 | "Tommi Björn Paavola" | "Tommi" | "Paavola" | "Tommi Björn Paavola" |
| Ioan Viorel Ganea | 319882766 | 2 | (null in blk1) | "Ioan" | "Ganea" | "Ioan Viorel Ganea" |

Note: the block-1 direct FmString includes the full birth name (middle names); the sub-record gives the display name. Block-2 players typically have DOB year = 0 (newgen/uninitialized).

## Staff name sub-records

Staff have **one name block** using the same pattern as player block-1, but at lower offsets:

| Offset | Field key | Contents |
|--------|-----------|----------|
| `+0x140` | `s_name` | `u64 → [u32 len][UTF-8]` direct full name (nullable for ~72% of staff) |
| `+0x148` | — | 8-byte padding / always null |
| `+0x150` | `s_name_first_sub` | `u64 → 5×24-byte first-name candidate list` |
| `+0x158` | `s_name_last_sub`  | `u64 → 5×24-byte last-name candidate list` |

Sub-record entry layout and reading rules are identical to the player name sub-records above.

Reading order: try `s_name` FmString → sub-records at `+0x150`/`+0x158`.

In a 2000-staff sample: ~28% use the direct FmString; ~72% have null FmString and require sub-record lookup. All 100% have a name after applying the fallback.

**Witnesses verified (2000-staff sample):**

| Staff | Addr (session) | FmString | first_sub | last_sub | Combined |
|-------|---------------|----------|-----------|----------|---------|
| Frank Dauwen | 0x22034070010 | null | "Frank" | "Dauwen" | "Frank Dauwen" |
| Pascal De Vreese | 0x22034070250 | null | "Pascal" | "De Vreese" | "Pascal De Vreese" |
| Zvonimir Boban | 0x22034072410 | null | "Zvonimir" | "Boban" | "Zvonimir Boban" |
| René Henriksen | 0x22034071210 | null | "René" | "Henriksen" | "René Henriksen" |
| Danny Hesp | 0x22034073850 | null | "Danny" | "Hesp" | "Danny Hesp" |
| Mauro Tassotti | 0x22034072AD0 | null | "Mauro" | "Tassotti" | "Mauro Tassotti" |

## Unmapped fields — worklist

### Player
- **Sharpness, Fitness, Fatigue** — dynamic condition values. Need in-game values to triangulate (varies day-to-day).
- **Current club resolved name** — accessible via player+0x130 → sub+0x00C (u32 club UID), but the club lookup isn't wired into the player UI yet.

### Staff
- **Formation preferences (8 x u8 at +0x0DF..+0x0E6)** — stored as formation IDs (direct, ~0-64 range). Order is [Pref In, 2nd In, Def In, Att In, Pref Out, 2nd Out, Def Out, Att Out]. Confirmed ID 4 = "4-3-3 DM Wide" via Spinelli/Lisboa. Some formation variants (e.g. "4-4-2 2DM" / "4-4-2 Narrow Diamond") may share IDs — needs full formation ID table.
- **Unmapped struct regions:** 0x044-0x0CD (pointers, IDs), 0x0E7-0x10B (person base tail), 0x110-0x13F, 0x148 (8-byte null padding after s_name FmString), 0x160-0x16F, 0x18C-0x1CF.

### Club
- **Reputation** — NOT in main struct or info sub-record. Extensive search done; all candidate hits were false positives from pointer bytes. Likely lives in a competition/league sub-record we haven't dereferenced yet.
- **Corporate facilities** — not found near other facility ratings in the info sub-record.
- **Nation** — not yet searched.
- **Status (Pro/Semi-Pro/Amateur)** — not yet searched. Stub dropdown in club search UI.
- **Human-managed flag** — not yet searched. Stub checkbox in club search UI.

### Other
- The `find_human_manager` validator (`bridge::find_human_manager`, in `src/bridge/entity_load.rs`) currently re-derives the manager every time. If we ever need to track "is the human still alive / hasn't been sacked", that's a separate field on the staff struct that we haven't located.

## Known RVAs not yet used

Found by `scan_arrays` but never dereferenced as their suspected entity type. Pre-2026-10-patch RVAs; add +0x4F7F0 for the current build (unverified).

| RVA | Suspected contents |
|-----|---------------------|
| gp+0x4DF9E58 / +0x4DF9E60 | FNationOffset — 186 nations |
| gp+0x4DF9D88 / +0x4DF9D90 | Unknown — 748 entries (competitions?) |
| gp+0x4DF9DA0 / +0x4DF9DA8 | Unknown — 396 entries (regions?) |
| gp+0x4DF9DB0 / +0x4DF9DB8 | Unknown — 242 entries (currencies?) |

## Witnesses for triangulation

Use these when verifying a new field offset. Always cross-reference at least 2 — single-witness "confirmation" produces false positives.

| Name | Entity | Identifier | Used for |
|------|--------|-----------|----------|
| Harry Kane | Player | UID 28049320 | Attributes, positions, DOB, CA/PA, PPMs (6 traits) |
| Joshua Kimmich | Player | Name search | Positions, PPMs (7 traits + PPM50) |
| Jari Litmanen | Player | Index 0 | Positions, DOB, PPMs (4 traits + PPM50) |
| Josko Gvardiol | Player | DOB 23/1/2002 + DC=20 | Positions (DL/WBL) |
| Mikey Moore | Player | DOB 11/8/2007 + AMC=18 | Positions (ML/MR, AML/AMR) |
| Pep Guardiola | Staff | Personality [20,20,15,18,20,16,14,8] | All staff attributes |
| Jorge Batista | Staff | ID 6300178 | All staff attributes |
| Ivor Peixoto | Staff | ID 2002077124 | Staff attributes (3rd witness) |
| Alessandro Greco | Staff | Personality + CA=63 | Staff role search (GK Coach=20) |
| Marc Overmars | Staff | ID 355 | Staff roles (DoF=20, TD=20, Scout=12) |
| Bert Konterman | Staff | Personality + CA=116 | Staff roles (Coach=20, Scout=20, Asst=10, DoF=10) |
| Wesley Lisboa | Staff | Personality + CA=104 | Staff roles (DataAnalyst=20, Scout=12, LoanMgr=12, etc.) |
| Takuya Takeuchi | Staff | Personality + CA=63 | Staff roles (Physio=20, SportsSci=20, Fitness=12) |
| Gianluca Spinelli | Staff | Personality + CA=165 | Staff roles (GK Coach=20, confirms pos 5) |
| FC Bayern | Club | UID 915 | Club sub-records: info, finance, stadium |
| Tottenham Hotspur | Club | UID 728 | Club sub-records: info, finance, stadium (2nd witness) |
| The human manager (test save) | Human Manager | last entry of ptr_staff | Verified the +0x350 hybrid layout |

## Discovery tools

- **`bash Tools/scan-arrays.sh`** — wraps `src/bin/scan_arrays.rs`. Walks the data segment of `game_plugin.dll` for 8-byte-aligned `(data, end)` u64 pairs whose `(end-data)/8` count matches a target. Use this when an array RVA is wrong or when looking for a new entity array. Edit the target counts at the top of `scan_arrays.rs` to match what you're hunting.
- **`bash Tools/dump-manager.sh`** — wraps `src/bin/dump_manager.rs`. Dumps the human manager struct as hex with annotations and runs heuristic searches for personality blocks, name pointers, and role sub-records. The model for any per-entity dump tool — copy and adapt for new entity types.
- **rust-analyzer-mcp** — configured in `.mcp.json`. Use `rust_analyzer_diagnostics` after every code edit; use `rust_analyzer_references` / `rust_analyzer_definition` for code navigation instead of grep.
