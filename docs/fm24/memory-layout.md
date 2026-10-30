# FM24 Memory Layout

Mapped 2026-10-22 against a fresh FM24 save (day 0, 3 Jul 2023) under Proton.
The code is the source of truth (`src/fields/fm24/`); this file holds the
discovery context, witnesses and what is still unmapped.

## Module

FM24 has **no `game_plugin.dll`**. Game logic and every entity-array RVA live
in `fm.exe` itself (image base 0x140000000), so `gp_base` is the fm.exe base on
this version (`GameVersion::module_name`). The exe is wrapped (non-standard
sections `.arch`/`.xtls`/`.sxdata`, a 360 MB RWX `.arch`), so static analysis is
not useful; everything below came from live memory.

Under Proton FM24's `/proc/<pid>/comm` is `Main Thread`, not `fm.exe`; the
Linux backend also matches the basename of argv[0] (`S:\…\fm.exe`).

Version gate: `known_dll_hash` = hash of the first 64 KB of `fm.exe`.

## Entity arrays (RVA relative to fm.exe)

| Entity | Count | data / end | Object vtable | Stride |
|--------|-------|------------|---------------|--------|
| Players | 83,426 | 0x642EDD0 / 0x642EDD8 | fm.exe+0x5A4E958 | 0x3E8 |
| Staff | 33,835 | 0x642EDE8 / 0x642EDF0 | fm.exe+0x5A4C2E8 | 0x268 |
| Clubs | 36,725 | 0x642EF58 / 0x642EF60 | fm.exe+0x5A51F38 | 0x170 spacing |

Found by scanning fm.exe's writable sections for `(data, end)` pointer pairs,
then locating witness records in the heap (Guardiola by his personality bytes,
Kane by his UID) and finding which arrays point at them.

- 0x642EDB8 (116,856) points 0x28 into a different object type; not players.
- 0x642EDF0 (24,775) is a subset of the staff array.
- 0x642EF88 (36,498) is a strict subset of the club array.
- 99.8% of player-array entries carry the player vtable.

The stadium array root is not located; stadiums are reached per club through
the `home_stadium` sub-record, so `rva_stadium_base` is 0.

## Player (vs FM26)

- Positions + attributes keep the FM26 order shifted **+0xB8**: positions
  0x208..0x216, attributes 0x217..0x24C.
- Individually moved: home/current/world rep 0x1FA/0x1FC/0x1FE, CA/PA
  0x200/0x202, height 0x14E, UID 0x284, name FmString 0x2C0, name subs
  0x2D0/0x2D8, DOB 0x2BC, personality 0x2F0..0x2F7, club ptr 0x3B0.
- Names: 40% direct FmString, 59% first/last sub-records, 0.5% neither.

## Staff (vs FM26)

- Attribute block (< 0x50) is FM26 **+8**. Seven fields disagree with the
  Guardiola witness at +8 and are left unmapped: judge staff, data analysis,
  sports science, versatility, business, interference, resources.
- Reps 0xD0/0xD2/0xD4, CA/PA 0xD6/0xD8 (4 earlier than FM26), staff ID 0x104,
  name 0x140, name subs 0x150/0x158, DOB 0x13C, personality 0x170 (unchanged),
  role-ratings ptr 0x1F8.

### Human manager

Last entry of the staff array (vtable fm.exe+0x5A6A318). Identity fields live
at `offset + 0x358` (FM26: 0x350): IDs, DOB, name subs, personality, role ptr.
Unlike FM26 the direct name FmString is null in both slots — the name comes
from the sub-records. Verified on the test save's human manager (role ratings Manager 20).

## Club (vs FM26)

- Top level identical: UID 0x0C, full/short name 0xC0/0xC8, index 0x08.
- Info sub-record (ptr +0x100) is FM26 **+8**: abbreviations 0x00/0x08/0x10,
  season tickets 0xC0, founded 0xD8, owner status 0xDE, social 0xF8,
  supporter % 0x106..0x10B, supporter attributes 0x114..0x119, facilities
  0x120..0x12D.
- Finance (+0x150), budget (finance+0x48) and attendance (stadium-sub +0xB0,
  0x68/0x6C/0x70) unchanged.
- Home stadium: club+0x18 → [0] senior team → +0x78, same chain as FM26.
- Not mapped: gender (+0x161 holds 0/2/4/8/10/32, not FM26's 0/1), nickname
  (stadium-sub +0x90 is a short name here), team UID, training ground,
  stand types, reputation, nation.

## Stadium entity (stride 0xB0)

UID 0x0C, name 0x38, alt name 0x40, capacity/seated/used/all-seater/expansion
u32 0x6C/0x70/0x74/0x78/0x7C, owner club ptr 0x80, pitch length/width ×10
0x88/0x8A. Standing, environment, condition unmapped.

## Witnesses

| Entity | Identifier | Confirmed |
|--------|-----------|-----------|
| Harry Kane | Player UID 28049320 | name, DOB 209/1993, height 188, CA/PA 184/185, reps 9250/9200/8750, positions (ST 20, AMC 15, MC/AML 12, AMR 10), attributes, club → Tottenham |
| Josep Guardiola | Staff ID 1120 | name, DOB 18/1971, CA/PA 190/200, reps 9500, personality, roles, 31/38 attributes |
| The human manager (test save) | Human manager | shifted identity block, roles |
| FC Bayern | Club UID 915 | names, abbreviations, founded 1900, facilities, supporters, attendance |
| Tottenham | Club UID 728 | as Bayern; ticket prices 65 / 1296 |
| Allianz-Arena, Tottenham Hotspur Stadium | Stadium | capacity, expansion, owner, pitch |

Sample checks (1500 players): GK position vs handling r=0.93, handling/reflexes
r=0.89, DL/WBL r=0.82, attribute bytes ≤ 100, max(left, right foot) = 20 for all.
1500 staff: CA ≤ PA, attribute bytes ≤ 100, personality 1..20, plausible DOBs.

## Unmapped worklist

Player: value, wage, contract sub-record, morale, traits, apps/goals, dates
(transfer/join/debut/listing), nation ptr, secondary name block. Staff: the
seven attributes above. Club and stadium: see above.
