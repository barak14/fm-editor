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

Three entries in the club array are not clubs: UIDs 2000000000, 2000000002
and 2000000003 use their own class (vtable +0x4F65538; real clubs use
+0x4CDA1F8), have no name strings and no players (2000000000 has six empty
teams). The club list skips entries with no name. One real club, "C.D.
35600" (UID 2000567841), has a full name but no short name and is listed
under its full name.

`Tools/scan-arrays.sh` scans every writable, non-executable section from the
module's PE section table (`.sdata`, `.tls$`, `.debug` on this build), so it
reaches the FM27 cluster. Re-run 2026-10-30 against a later save in the same
Steam build: players 0x57037D0 (103,517), staff 0x5703818 (49,197) and clubs
0x5703988 (33,471) all listed.

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

The human manager is an object of its own staff class (vtable +0x4CC086C;
AI staff use +0x4D8BE08, player-coach staff +0x4F759F0). The staff array holds
two of them: the manager, and the entry FM creates for the player's Steam
profile (named "(<account name>)", staff id 2002106548, no club). Both keep
their identity block at +0x340, including the name sub-records at
+0x150..+0x167 (the Steam entry has only a last-name record), and listed as
nameless ID-0 staff until the staff reader patched the shift back for this
class (`human_manager_vtable_rva`).

The Steam profile entry is not a person in the game world: its DOB is FM's
empty date (1 Jan 1900; the manager's is 1980), and its nation (+0x4A8),
birth city (+0x4C0), contract (+0x4E8) and name FmString (+0x480) pointers are
null where the manager's are set (compared 2026-10-30). The staff list leaves
out any human-manager-class entry with an empty DOB
(`staff::is_account_placeholder`).

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
there (+0x8A9; Arsenal 19 = "Excellent"). The editor writes both coaching and
recruitment copies; that write shows in game (see "Live write verification"
below). FM26's "youth improvement" (+0x126) and "youth academy"
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
| Listing flags | contract+0x57 (bit 0 transfer, bit 1 loan) | all five unlisted; Mikel Merino (Arsenal, 67211760) reads 1 and FM shows Transfer status "Listed", Loan status "Not set" (2026-10-31) |
| Job code | contract+0x26 (FM26 "contract type") | 1 = Player for all five |
| Asking price (i32, -1 unset) | +0x238 | -1 for all five, and also -1 for Merino, whom FM lists with a £45M transfer value; that £45M is not stored as an i32 in his first 0x800 bytes or his contract. Mapping unconfirmed |
| Club name | team (+0x130) → club (team+0x30) → +0xC0 | Manchester City / Barcelona / R. Madrid |
| Nationality | nation (+0x2F0) → +0x18 | Norway / Spain / France / England / Portugal |
| Place of birth | city (+0x308) → +0x18 | Leeds / Esplugues de Llobregat / Bondy / London / Funchal |
| UID (u32) | +0x294 (FM26 read it as u16) | Haaland 29179241, Mbappé 85139014 |
| Weekly wage | FM26 +0x248 reads 13748 for Haaland — unmapped; the panel falls back to the contract wage | |

**Staff** (Arteta / Flick / Xabi Alonso / Maresca / the user's manager):

| Field | Location | Values |
|-------|----------|--------|
| Contract pointer | +0x1A8 (manager +0x1A8+0x340); contract+0x08 → person | all five; Guardiola, Slot, Ancelotti have none |
| Job code | contract+0x26 | 16 for all five; FM27 titles the job "Manager" (the user's profile, 2026-10-30) |
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
| Current wage total incl. bonuses (i32 £/week) | finance+0x814 | 4,440,326 / 4,355,776 / 5,352,976 / 5,404,502. FM's Finances > Overview Wages card labels it "Current wage total (incl. bonuses)": Spurs £4,362,011 in the 2026-10-31 save. It is spending, not a budget; FM's Portal shows it as "Spending Wage" |
| Wage budget (i32 £/week) | finance+0x808 and +0x834 (equal for every witness; which is current and which next season is unsettled) | 4,940,326 / 4,505,776 / 5,852,976 / 5,754,502. The same Wages card shows Spurs' "Budget" £4,559,136 (2026-10-31 save; both offsets read 4,559,136) |
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

## Game Pass build

The PC Game Pass (Xbox app) release of FM27 is a separate binary built on the
Microsoft GDK instead of Steamworks. It has its own schema,
`FM27_GAMEPASS` in `src/fields/fm27/mod.rs`, selected when the install is
recognised as Game Pass: a `MicrosoftGame.config` next to `fm.exe`, or a path
under `XboxGames` or `WindowsApps`. Everything else is treated as Steam.

**Nothing in `FM27_GAMEPASS` is verified.** It is a copy of the Steam schema
with `known_dll_hash` 0. The module hash and the module-relative values (entity
array RVAs, stadium root, finance / player-coach / human-manager vtable RVAs)
are expected to differ. Struct field offsets probably match, since the source
and compiler are the same, but that is unconfirmed.

Bring-up checklist (Windows, Game Pass FM27 with a save loaded, debug build via
`cargo build`). Compare against a Steam build at the same patch where possible.

1. **Attach.** The toolbar should show `FM27 Game Pass (PID n)`, and the
   status bar the notice `FM27 Game Pass game_plugin.dll hash not calibrated
   (current: 0x…)`. Record the hash. If attach fails, record the error:
   - `fm.exe is not running` — the Game Pass exe has another name; note the
     process name and path from Task Manager.
   - `cannot determine game version` — the install path has no release year.
   - `cannot read …` — the module file is access-restricted (`WindowsApps`).
   - Shown as plain `FM27` — storefront detection missed; note the exe path.
2. **Arrays.** fm-debug `attach_status` reports the player, staff and club
   counts and the module path. Zero counts or a `bad pointer array` error mean
   the RVAs moved: run `bash Tools/scan-arrays.sh` (Git Bash), find the
   players / staff / clubs pairs, and confirm each by decoding names
   (`dump_struct` on an element) as in "Entity arrays" above.
3. **Fields.** Pick witnesses (a well-known player, a staff member, a club)
   and check that name, CA/PA, DOB, attributes, contract and club finances
   match the game. `compare_entities` against the same entity in a Steam save
   shows any shifted offsets.
4. **Vtables.** Check the player-coach (James Milner), full-finance club
   (Spurs) and human-manager classes. On Steam, FM creates a staff entry for
   the Steam profile; note what Game Pass creates instead.
5. **Write.** Edit one witness field, confirm it in game, then undo.
6. **Record.** Put the hash and every changed value in `FM27_GAMEPASS`, and
   the evidence in this section, in one commit.

## Unverified / known broken

- `p_name2` block-2 names: removed on FM27 (see above).
- Reputation is memory-checked only; FM shows reputation as words
  ("Worldwide", "Continental"), not numbers.
- Staff role sub-record pointer (0x1D0).
- Stadium array root (`rva_stadium_base`) and stride.
- Asking price (+0x238) for a listed player (see the player table).
- Own-club budget writes (transfer budget, balance) are confirmed in memory
  only: FM had already shown Spurs' finances (see the caching note below).
- Day advance was not tested: whether FM refreshes its cached display after
  time passes is unknown.

### Club search membership (2026-10-30, Steam Huelva save)

Read-only verification against FM27 Steam, game date 2 August 2026.
Current club remains player +0x130 -> team +0x30 -> cached club identity.
Ownership is player +0x330 -> contract +0x10 -> team +0x30 -> club.
Every hop is a little-endian u64 pointer. Player-coach person shifts apply to
the contract pointer through the existing schema resolver.

Independently checked in the game's player profiles:
- Mateo Izaguirre (2000159662): parent Juventud (130848), on loan at Huelva (1751).
- Samu Cortés (2000402336): parent Granada (1714), on loan at Huelva (1751).
Both current-team links resolve Huelva while both contract links resolve their
different parent clubs. Jero Lario (67210974) resolves Huelva through both.

Reserve teams use a u64 pointer vector at club +0x30 (begin), +0x38 (end).
Each entry is a team, whose +0x30 resolves its cached club identity.
The game's squad navigation independently shows:
- Huelva (1751): Huelva U19 and Huelva B; reserve vector resolves Huelva B (814499).
- Granada (1714): Granada U19 and Granada B; reserve vector resolves Granada B (67040821).
Youth squads belonging to the same club require no separate club UID.
Membership expansion follows these reserve vectors only, never fuzzy names or
general affiliates. Cycles are deduplicated; malformed or incomplete reserve
graphs make My clubs unavailable. These additions are enabled only for FM27
Steam; Game Pass and other builds have not been verified for this capability.
No game-memory writes were performed for this read-only feature.

Verified module SHA-256: `BC940E472418FC5C214208D7CF30FECF600F689B6F9D71C11BA19A2DF0FE057F`.
On FM27 Steam, manager discovery now requires the already-mapped human-manager vtable and excludes the platform-account placeholder. Game Pass keeps the previous heuristic because its inherited vtable RVA is unverified.

Live feature UI: manual current Club ID 1751 returned 24 players; My clubs returned 41, including the reserve group and both incoming-loan witnesses. Counts are save-specific, not constants.

## Live write verification (2026-10-31, Steam, Spurs save)

FM27 Steam under Proton, run in the headless session (`Tools/fm-headless.sh`),
with a debug editor on the same display. Game date Mon 13 Jul 2026, 09:00. Each
write went through the editor's UI and acknowledged-write path, was read back
through fm-debug, compared with FM's own screens, then undone. No test value is
left in memory.

**FM caches displayed entities.** Once FM has shown a player, staff member or
club in a session, it keeps showing those values. Navigating away, opening
another entity and reopening it through search does not refresh them. Edit
before FM first displays the entity, and expect FM to keep showing the edited
value after Undo. Witnesses:

- Virgil van Dijk (37024025): Crossing (+0x15F, encoded) 4 (raw 20) → 10 (raw
  48). FM's profile, opened after the edit, shows 10. After Undo memory reads 4
  (raw 20), but FM still shows 10 after reopening his profile.
- Spurs (728): facilities shown before the edit stayed "Good" in FM while both
  coaching copies read 18, then 3.

| Write | Witness (never shown in FM before the edit) | Memory | FM after the edit | Undo |
|-------|--------------------------------------------|--------|-------------------|------|
| Player attribute | Van Dijk Crossing | 4 → 10 | 10 on the profile | 4 (raw 20) |
| Academy coaching, both copies (info +0x12C, finance +0x8A6) | Chelsea (630) | 19/19 → 4/4 (recruitment 18 untouched) | Academy Coaching "Fairly Basic"; Youth Recruitment (18) "Excellent" | 19/19, two undo entries, newest first |
| Staff coaching attribute | Mikel Arteta (6701089) Fitness (+0x34, encoded) | 7 (raw 35) → 12 (raw 58) | "Good" (on the same profile 10–11 Average, 12–14 Good, 15–17 Very Good, 18–19 Outstanding) | 7 (raw 35) |
| Transfer budget (finance +0x7C4) | Spurs, already shown | 25,900,000 → 40,000,000 | still £25.9M (cached) | 25,900,000 |

Read checks in the same session: Van Dijk's 36 visible attributes, height,
DOB, caps 96/13 and U21 3/0 match FM. His wage, 350,000, lies within FM's
scouted range of £325K–£375K. Arteta's coaching, mental and knowledge labels
match, as do his contract (£375K p/w, 30/6/2030) and DOB. Squad status 4 =
"Squad Player" (Merino) and 1 = "Star Player" (Van Dijk). Squad numbers
Merino 23 and Rice 41 match their Arsenal shirts. Spurs' balance (finance+0x0C)
reads 173,975,262 and FM shows £173,975,262. Facility labels: Academy Coaching
16 "Good" (Spurs) and 17 "Excellent" (Arsenal); Youth Recruitment 18 "Excellent"
(Chelsea).
