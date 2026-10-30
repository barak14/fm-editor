# FM27 Memory Layout

Bring-up started 2026-10-30 against a fresh FM27 save (day 1 of the first
season) under Proton. The code is the source of truth (`src/fields/fm27/`);
this file holds the discovery context, witnesses and what is still unverified.

`src/fields/fm27/` began as a copy of the FM26 (2026-10 patch) schema. Only the
items listed under "Verified" below have been checked against FM27; every other
offset is inherited from [FM26](../fm26/memory-layout.md) and unverified.

## Module

FM27 is a Unity build like FM26: game logic lives in
`fm_Data/Plugins/x86_64/game_plugin.dll`, and every entity RVA is relative to
its base. The DLL is wrapped (non-standard sections `.srdata`/`.arch`/`.sdata`/
`.code`, a 337 MB RWX `.code`); the entity-array cluster sits in `.sdata`
(RVA 0x566F000, ~1 MB).

Under Proton, Wine maps only the DLL's PE header page file-backed and loads the
sections into anonymous memory (file alignment 0x200 < page size), so
`/proc/<pid>/maps` shows a single 4 KB mapping for `game_plugin.dll`. The Linux
backend takes the module size from the PE header's `SizeOfImage` (0x19E69000)
instead of the file-backed span.

Version gate: `known_dll_hash` is 0 (not calibrated); attach reports the hash.

## Entity arrays (RVA relative to game_plugin.dll)

The FM26 array cluster is intact, moved as one block by **+0x8BA158**.

| Entity | Count (this save) | data / end | FM26 data |
|--------|-------------------|------------|-----------|
| Players | 106,868 | 0x57037D0 / 0x57037D8 | 0x4E49678 |
| Staff | 51,466 | 0x5703818 / 0x5703820 | 0x4E496C0 |
| Clubs | 33,570 | 0x5703988 / 0x5703990 | 0x4E49830 |
| Stadium root | — | 0x5703750 | 0x4E495F8 |

Found by sampling elements of every heap `(data, end)` pair in `.sdata` and
decoding names at FM26 offsets: players at `p_name` 0x2C8 / subs 0x2D8/0x2E0,
staff at `s_name` 0x140 / subs 0x150/0x158, clubs at `c_full_name` 0xC0 /
`c_name` 0xC8. The stadium root is the same shift applied and is unverified.

Other pairs in the cluster:
- 0x57037B8 (157,943) — points 0x128 into person objects (players + staff);
  names at +0x28/+0x30 there. Not an entity array the editor uses.
- 0x57037E8 — same count as the player array (106,868).
- 0x57039A0 (5,116), 0x57039E8 (30,771), 0x5703A90 (2,799) — club subsets.

`Tools/scan-arrays.sh` scans gp+0x4000000..0x5400000 (the FM26 window) and
does not reach the FM27 cluster.

## Verified

### Player

- Erling Haaland: DOB 21 Jul 2000, height 195, left foot 20, finishing 19,
  CA 187 / PA 195, joined 2022 — all at FM26 offsets. Confirmed in-game by the
  user.
- Name subs: first 0x2D8, last 0x2E0 (Kylian / Mbappé; full name FmString
  0x2C8 "Kylian Mbappé Lottin").
- **Common name sub 0x2E8** (new): Lamine Yamal (first "Lamine Yamal", last
  "Nasraoui", common "Lamine Yamal"), Cristiano Ronaldo (last "Aveiro", common
  "Cristiano Ronaldo"), Vinícius Ciganha Marques Gouvêa (common "Vinícius").
- +0xF0 holds a direct FmString that looks like the shirt name ("Mbappé",
  "E.Mbappé", "Ronaldo", "Jobe"); not mapped.

Display name order (`player::read_player_name`): common name, then first +
last subs, then the full-name FmString.

### Staff

- Name subs first 0x150, last 0x158 (FM26 offsets).
- **Common name sub 0x160** (new): Luis Enrique Martínez García ("Luis
  Enrique"), Joaquim Silva ("Quim"), Jon Pérez Alonso ("Bolo").

### Human manager — +0x340 shift

Still the last entry of the staff array, with the FM26 hybrid layout, but the
identity block shift is **0x340** (FM26: 0x350). Witness: the test save's human
manager, located by searching the heap for his name FmString and tracing
the single pointer to it (manager+0x480 = 0x140 + 0x340):

| Field | Standard | At +0x340 | Value |
|-------|----------|-----------|-------|
| staff id | 0x10C / 0x110 | 0x44C / 0x450 | 2002106549 |
| name FmString | 0x140 | 0x480 | the manager's name |
| personality | 0x170 | 0x4B0 | 14,10,10,10,10,10,10 |
| DOB | 0x188 | 0x4C8 | matches the in-game profile |

At +0x350 the same slots read zeros and unrelated data. The role sub-record
pointer (0x1D0 + shift) is unverified: it reads an unaligned value
(0x263009D03), as does a normal staff member's 0x1D0.

### Stadium (home stadium via club → senior team → +0x78)

The FM26 chain and entity offsets hold. Names are FmStrings, shown on the
club Stadium tab:

| Club | `st_name` +0x40 | `st_name2` +0x48 | `st_capacity` +0x7C |
|------|-----------------|------------------|---------------------|
| Spurs | Tottenham Hotspur Stadium | (null) | 62,850 |
| Arsenal | Emirates Stadium | Arsenal Stadium | 60,704 |
| Man City | Etihad Stadium | City of Manchester Stadium | 60,450 |

Spurs' capacity also matches the editor's Stadium tab. The training ground
(club+0x140) resolves too: Hotspur Way, London Colney, The City Football
Academy. The stadium array root (`rva_stadium_base`) is still unverified.

### Club

- **Gender flag moved to club+0xB9** (FM26: +0x161, which reads 10 for every
  club on FM27). 0 = men, 1 = women; it splits the club array 30,771 /
  2,799, exactly the two club sub-arrays at gp+0x57039E8 / gp+0x5703A90.
  Witness pairs (men 0 / women 1): Spurs, Arsenal, Man City, Liverpool.
- **Info sub-record (club+0x100): everything after the abbreviations is FM26
  +8.** Abbreviations at +0x00/+0x08/+0x10 are unchanged.

| Field | FM26 | FM27 | Spurs | Arsenal | Man City | Liverpool |
|-------|------|------|-------|---------|----------|-----------|
| Season tickets (u32) | 0xB8 | 0xC0 | 50,000 | 42,000 | 36,000 | 30,000 |
| Year founded (u16) | 0xD0 | 0xD8 | 1882 | 1886 | 1894 | 1892 |
| Social media (u32) | 0xF0 | 0xF8 | 6.8M | 26.2M | 59.0M | 42.7M |
| Supporter % ×6 | 0xFE | 0x106 | 10/37/15/19/5/14 | 7/26/24/17/4/22 | 6/32/18/22/3/19 | 7/30/26/15/4/18 |
| Supporter traits ×6 | 0x10C | 0x114 | 18/14/7/14/7/14 | 16/9/12/17/15/15 | 18/10/10/9/11/8 | 19/18/14/12/17/16 |
| Training facilities | 0x123 (labelled youth) | 0x120 | 20 | 19 | 20 | 20 |
| Youth facilities | 0x118 (labelled training) | 0x12B | 20 | 16 | 20 | 20 |
| Academy coaching | 0x124 | 0x12C | 16 | 17 | 19 | 17 |
| Youth recruitment | 0x125 | 0x12D | 16 | 15 | 20 | 16 |

Supporter percentages sum to 100 for every witness; Spurs' and City's
supporter traits are identical to the FM26 witnesses. Year founded is a u16:
Liverpool's following u16 is non-zero, so a u32 read gave 119,932,772. Women's
sides read 1985/1987/1988/1989.

FM26's "owner status" (+0xD6, now +0xDE) reads 10000 (Spurs, City), 9000
(Arsenal, Liverpool) and 0 for every women's team. That is not an owner
status; the field is unmapped on FM27 until its meaning is known.

Facility labels were checked against FM27's Club Info > Facilities screen for
Arsenal: Training "Superb" = +0x120 (19), Youth "Excellent" = +0x12B (16),
Academy Coaching "Excellent" = +0x12C (17), Youth Recruitment "Good" = +0x12D
(15). The FM26 schema had Training and Youth swapped. Academy coaching and
recruitment have a second copy in the finance object (+0x8A6 / +0x8A7, equal
to the info copy for all four witnesses), and corporate facilities live only
there (+0x8A9; Arsenal 19 = "Excellent"). The facility screen reads the
finance copies, so an edit to the info copy alone may not show in game; this
is unverified. FM26's "youth improvement" (+0x126) and "youth academy"
(+0x121) match nothing on the screen and are unmapped.

Cross-reference: the GPL-3 FM27€0 editor by Anze Alexander
(sortitoutsi.net) publishes an FM27 profile. Its "person" object is our
player base + 0x288 (staff: our base + 0x100), and every offset we had
mapped independently matches it. It was used only as a source of leads for
the facility order above; no code was taken from it.

### Player and finance fixes (checked through the fm-debug MCP server)

| Field | FM26 schema | FM27 | Witnesses |
|-------|-------------|------|-----------|
| Club | `p_club_ptr` +0x128 (no club) | team ptr +0x130 → club at team+0x30 (`p_team_ptr`) | Haaland → Manchester City, Yamal → Barcelona, Mbappé → R. Madrid |
| International caps / goals | "senior apps/goals" +0x3BC / +0x3BE | u16 +0x3BC / u16 +0x3BE | Haaland 55 / 62 (confirmed in game), Yamal 33 / 7, Mbappé 106 / 66 |
| U21 caps / goals | — | u8 +0x3C0 / u8 +0x3C1 | Haaland 3 / 0 |
| Secondary name | `p_name2` +0x3C0 | does not exist (+0x3C0 is U21 caps) | reads an invalid pointer for every player |
| Club balance | finance+0x14 | finance+0x0C | Spurs £173.9M (confirmed in game), Arsenal £333.1M, Man City £478.5M, Liverpool £185.6M |
| Avg ticket price | finance+0x18 (f32) | f32 finance+0x864 | Spurs 65, Arsenal 49, Man City 50, Liverpool 49 (+0x18 reads 0.6-1.4) |

### Second pass: contracts, names, budgets, reputation

Leads from the FM27€0 profile, each checked in this save on several
witnesses (Python /proc reads and the fm-debug MCP server). "Person" below is
player base + 0x288, staff base + 0x100.

**Player** (witnesses Haaland / Yamal / Mbappé / Saka / Ronaldo):

| Field | Location | Values |
|-------|----------|--------|
| Contract pointer | +0x330; contract+0x08 → person (validates the chain) | all five |
| Contract wage (i32 £/week) | contract+0x20 | 472,500 / 343,612 / 576,610 / 300,000 / 3,413,698 |
| Expiry | contract+0x48 (date) | 30 Jun 2034 / 2031 / 2029 / 2031 / 2027 |
| Signed | contract+0x44 (date) | Haaland 17 Jan 2025, Mbappé 1 Jul 2024 |
| Squad status (u8) | contract+0x54 (FM26 +0x24 reads 11 for all) | 1 Star / 2 Important / 1 Star / 2 Important / 5 Impact Sub |
| Squad number (u8, 255 = none) | contract+0x5D | 9 / 10 / 10 / 7 / 255 |
| Listing flags | contract+0x57 (bit 0 transfer, bit 1 loan) | all unlisted; not checked on a listed player |
| Job code | contract+0x26 (FM26 "contract type") | 1 = Player for all five |
| Asking price (i32, -1 unset) | +0x238 | -1 for all five |
| Club name | team (+0x130) → club (team+0x30) → +0xC0 | Manchester City / Barcelona / R. Madrid |
| Nationality | nation (+0x2F0) → +0x18 | Norway / Spain / France / England / Portugal |
| Place of birth | city (+0x308) → +0x18 | Leeds / Esplugues de Llobregat / Bondy / London / Funchal |
| UID (u32) | +0x294 (FM26 read it as u16) | Haaland 29179241, Mbappé 85139014 |
| Weekly wage | FM26 +0x248 reads 13748 for Haaland — unmapped; the panel falls back to the contract wage | |

**Staff** (Arteta / Flick / Xabi Alonso / Maresca / the user's manager):

| Field | Location | Values |
|-------|----------|--------|
| Contract pointer | +0x1A8 (manager +0x1A8+0x340); contract+0x08 → person | all five; Guardiola, Slot, Ancelotti have none |
| Job code | contract+0x26 | 16 Head Coach for all five |
| Employer | contract+0x10 → team → +0x30 club | Arsenal / Barcelona / Chelsea / Manchester City / Tottenham Hotspur |
| Wage (i32 £/week) | contract+0x20 | 375,000 / 99,218 / 210,000 / 150,000 / 120,000 |
| Expiry | contract+0x48 | 2030 / 2028 / 2030 / 2029 / 2028 |
| Nationality | +0x168 → +0x18 (manager shifted) | Spain / Germany / Spain / — / England |
| Place of birth | +0x180 → +0x18 | San Sebastián / Heidelberg / Tolosa |
| Attributes | all 23 staff attribute offsets match FM27€0's | |

The human-manager validator no longer requires the role-ratings pointer when
the schema leaves it unmapped (FM27).

**Club** (Arsenal / Spurs / Man City / Liverpool):

| Field | Location | Values |
|-------|----------|--------|
| Reputation (u16) | senior team (club+0x18 → [0]) + 0xA8 | 8950 / 7650 / 9150 / 8850 |
| Transfer budget (i32 £) | finance+0x7C4 | 152.7M / 25.9M / 48.9M / 43.3M (Spurs confirmed in game) |
| Spending Wage Budget (i32 £/week) | finance+0x814 | 4,440,326 / 4,355,776 / 5,352,976 / 5,404,502 (Spurs confirmed in game) |
| Total Wage Budget / Next Season's Budget | finance+0x808 and +0x834 (equal for every witness; which is which unsettled) | 4,940,326 / 4,505,776 / 5,852,976 / 5,754,502 (Spurs: total "£4.5M", next season £4,505,776 in game) |
| Youth grant income (i32 £) | finance+0x7DC | 180 / 900 / 720 / 990 (Spurs £900 in game) |
| Corporate facilities | finance+0x8A9 | 19 / 20 / 14 / 16 (Arsenal 19 = "Excellent" in game) |

FM26's finance+0x48 budget pointer is null on FM27. The finance sub-record
is read to +0x8B0.

**Two finance classes.** Only 885 clubs (Spurs, Arsenal, Man City,
Liverpool, Argentinos Jrs, …) have the full finance object, vtable
game_plugin+0x4CF0CC8 (FM27€0's FinanceVtRva). The other 32,682 use a
smaller class (vtable +0x4CF07F8) where +0x7C4/+0x808/+0x814 read packed
dates and floats. `full_finance_vtable_rva` gates the finance sub-record on
the full class, so minor clubs show no finance rows (their balance at +0x0C
is unverified). Editing Youth Coaching or Youth Recruitment also writes
the finance copy (two undo entries).

Not decoded: sponsorship deals (club+0x130 vector did not parse), morale /
condition / sharpness / injuries (FM27€0 also leaves these read-only), staff
role ratings, cup prize money.

### Player appearance

Bytes at person +0x8D..+0x90 (player +0x315..+0x318), shown in the 3D match:
ethnicity, hair colour, hair length, skin tone. Checked on nine players:

| Player | Ethnicity | Hair colour | Skin tone |
|--------|-----------|-------------|-----------|
| Haaland | 0 Northern European | 7 | 2 |
| De Bruyne | 0 Northern European | 7 | 4 |
| Ronaldo | 1 Mediterranean/Hispanic | 15 | 6 |
| Kubo | 10 East Asian | 15 | 7 |
| Salah | 2 North African/Middle Eastern | 15 | 9 |
| Mbappé | 3 African/Caribbean | 15 | 13 |
| Yamal | 2 North African/Middle Eastern | 9 | 14 |
| Vinícius Júnior | 9 Mixed Race | 15 | 17 |
| Saka | 3 African/Caribbean | 12 | 19 |

Hair length is 1 for all nine. Ranges (FM27€0's survey of every person):
ethnicity 0-10, hair colour 0-18, hair length 0-6, skin tone 0-19, 255 = not
set. Edits are not yet confirmed in a match.

### Dates carry flags

A date's u16 day-of-year word keeps flags above bit 8: Saka's contract-signed
date stores 7019 = 0x1A00 | 363 (29 Dec 2025). Readers mask the low 9 bits and
the date editor keeps the flag bits when it writes.

### Player-coaches

391 players in this save are player-coach objects (vtable game_plugin+0x4F75A08;
plain players use +0x4D27228). The object is the player part (0x288 bytes),
then 0xF8 bytes of staff data, then the person block. So every player field
at or past +0x288 — names, UID, DOB, nation, birthplace, personality, traits,
caps, contract pointer — sits 0xF8 later; attributes, CA/PA, reputations,
height and the team pointer (+0x130) do not move. FM26's "secondary name
block" (+0x3C0/+0x3D0/+0x3D8) is this layout.

Witnesses: James Milner (+0x3C0 "James Philip Milner", UID 5106363, born
4 Jan 1986 in Leeds, England, 61 caps / 1 goal), David Nugent, Keisuke Honda
(contract → FC Jurong, #10), Remko Pasveer (FC Twente, #22). Before handling
them, all 391 showed as blank rows in player search.

`player::read_player_raw` detects the vtable and copies the moved block back
to schema offsets, so readers see one layout; the player panel maps edits
at or past +0x288 to +0xF8, and person-relative sub-record pointers
(contract, nation, birth city) are resolved with the same shift. Every plain
player and every player-coach now has a name.

## Unverified / known broken

- `p_name2` block-2 names: removed on FM27 (see above).
- Budgets, reputation, wages and squad status are memory-checked only; the
  user confirmed balance and caps in game.
- Staff role sub-record pointer (0x1D0).
- Stadium array root (`rva_stadium_base`) and stride.
- Writes: only one round trip verified. The user's manager Fitness
  (`s_fitness`, manager+0x34, encoded): 13 → 14 in the editor, undo back to
  13, confirmed in memory (raw 65) and after reloading in the editor. No
  other field has been written.
