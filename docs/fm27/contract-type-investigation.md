# FM27 contract inspection and Part Time offer policy trial

A club-policy workaround now produces selectable Part Time offers in the observed
Farnham test career. It changes one permitted-type input, then lets FM construct
and negotiate the contract. **It is still an experimental trial:** the first live
Undo was refused by an overly broad guard. The user has since confirmed Undo
after opening/changing an offer, reapplication, save/reload and a day advance
with the corrected build. Read-only checks confirmed the policy after reload
and on the next day. A subsequently accepted Part Time contract now matches FM
and read-only current-contract checks. Its survival after save/reload and salary
payments remain unverified. No direct conversion of an existing contract is claimed.

The implementation applies to **any selected human-managed club that satisfies
the eligibility checks**, without club names or witness UIDs in runtime rules.
It deliberately does not extend to arbitrary policies, unmanaged clubs or builds.

## Verified build and scope

| Item | Observed value |
|---|---|
| Platform/storefront | Windows x86-64, Steam |
| Module version / Steam build ID | 27.0.0+2486695 / 26227640 |
| Module | fm_Data/Plugins/x86_64/game_plugin.dll |
| Module bytes | 433,986,560 |
| Module SHA-256 | bc940e472418fc5c214208d7cf30fecf600f689b6f9d71c11ba19a2df0fe057f |
| Existing editor header fingerprint | 0x74A331ED3E28B06C |

The historical header-hash multiplier is preserved; it is not standard FNV-1a.
The global FM27 `known_dll_hash` remains zero (uncalibrated). Successful attachment
therefore does not verify the full layout. Narrow inspection/policy metadata has
its own exact build/hash/class gates. Game Pass is not covered; policy writes are
also disabled on Linux/Proton. Neither compatibility checks nor localhost binding
were relaxed.

The baseline was clean at `23b94fbada6d4acb860d72dfcdd5ab776e90f827`, origin
`git@github.com:barak14/fm-editor.git`. Investigation and implementation use a
separate feature worktree. No game update, speculative discovery write, save-file
parser, contract copying, Steam achievement or editor-history manipulation was used.

## Current-contract evidence

The existing schema resolves player +0x330 to the current contract. Contract
+0x08 must point back to the player's embedded person at player +0x288. Exact
current-contract class RVA is **0x4B00AE8**. This is not a pending offer or a
future contract.

| Player UID | Independently supplied FM/user label | Observed context |
|---|---|---|
| 5131229, Sam Hutchinson | Part Time Contract | Farnham, 8/12 Sep 2026 |
| 2000544627, Jack Dean | Part Time Contract | Farnham, 8/12 Sep 2026 |
| 2000144100, Roman Charles-Cook | Part Time Contract | Dover, 8 Sep 2026 |
| 28106526, Brad House | Non-Contract | Farnham, 8/12 Sep 2026 |
| 2000445820, Aaron Williams | Non-Contract | Farnham, 8/12 Sep 2026 |
| 29111433, David Raya | Full Time, user-confirmed | Arsenal, 8/13 Sep 2026 |
| 37063644, Jurri?n Timber | Full Time, user-confirmed | Arsenal, 8/13 Sep 2026 |
| 2000144100, Roman Charles-Cook | Non-Contract | Farnham after user signing, 12/13 Sep 2026 |

Bounded repeated snapshots distinguish the three groups at **one byte +0xC3**:
0 = Part Time, 1 = Full Time, 4 = Non-Contract. The setter store at RVA
0x2342E73 and serializer at 0x2337E0A/0x2337E33 independently establish width 1.
Other values remain unknown; no masking or fallback enum is used. **+0x26 is a
job code**, and +0x57 cannot distinguish Full Time from Non-Contract.

Metadata is in `src/fields/fm27/contract.rs`. Generic writable
`p_contract_type` stays unmapped. The Contract tab clearly shows unsupported
conversion/Unknown rather than inventing a value. The debug snapshot can label
verified records, while rejecting missing, truncated, foreign-owned, stale and
unwitnessed player-coach contracts. It exposes no write authorization.

Wages are **GBP/week**, monetary bonuses **GBP/event**. A game's dollar/year
presentation is separate; no current FX rate or financial terms are chosen.
Dates retain packed flags. Roman's Non-Contract expiry `02 00 6C 07` is not a
genuine fixed-term date. Editing wage/expiry cannot establish a genuine conversion.

## Open negotiation research: evidence, not a supported mapping

Native RTTI identifies separate `CONTRACT_DETAILS_EDIT_SESSION` records (vtable
RVA **0x4E03A08**). Source tracing identifies five components at +0x10, +0x620,
+0xC30, +0x1240 and +0x1850, with type byte +0x603 and wage +0x5EC. Session mode
+0x481 is not contract type. These are research findings, not a writable schema.

On Roman's 8 September offer, the current Dover contract remained type 0/wage
500 GBP/week while Farnham's working draft was type 4/wage 0. After the user's
**Use Existing Contract** click, the working component became type 0/wage 500;
its baseline component remained 4/0. FM still rejected length and offered only
zero salary choices. Copying the type into the draft therefore did not remove
the separate permission restriction.

Native acceptance paths also coordinate wage/no-expiry state for type 4
(RVAs 0x1889458/0x1889572). A one-byte current-contract change is not established
as a complete conversion. No native game functions were invoked.

## Club-policy evidence

The existing club-info pointer is club +0x100. Native option builder RVA
**0x2330300** reads seven byte inputs at info **+0xE0..+0xE6**, deduplicating
permitted types. Its caller 0x2331A60/0x23323C3 produces the Cops/Cosa options
consumed by the negotiation UI. Other eligibility branches still exist.

| Club UID | Original 8 Sep policy | Fresh 13 Jul standard-database control |
|---|---|---|
| 8325096, Farnham | 4/4/4/4/4/4/4 | 4/4/4/4/4/4/4 |
| 647, Dover | 0/0/0/0/4/4/4 | 0/0/0/0/4/4/4 |
| 602, Arsenal | 1/1/1/1/1/1/1 | 1/1/1/1/1/1/1 |

The fresh career reproduced Farnham's disabled salary/length on Sam Rak-Sakyi
(UID 2000108696), whose current-contract pointer was null and draft type was 4.
An independent Dover offer to Jay Emmanuel-Thomas (28010572) explicitly showed
Part Time, with draft type 0/wage 617 GBP/week. The fresh manager/employer chains
were checked independently for both clubs. Both semi-professional clubs have
the same separate observed status byte; status alone does not explain the split.

Property dispatch RVA 0x10A20D5 and one-byte store 0x10A3330 map these inputs:

| Property | Info offset |
|---|---|
| Ccsp | +0xE0 |
| Ccsk | +0xE1 |
| Ccsf | +0xE2 |
| Ccsr | +0xE3 |
| Ccsb | +0xE4 |
| Ccsh | +0xE5 |
| Ccsy | +0xE6 |

Modern bucket labels and the origin/intent of Farnham's initial data remain
unconfirmed. The user reports a standard database and no professional-status
change. A plugin-free control has not been run; FMMatchLens was present, although
its inspected source revealed no negotiation patch. Do not call this a confirmed
SI defect or a complete explanation of all contract restrictions.

The original career also had limited payroll headroom: the supplied finance
screen showed $29,502/year remaining, below the imported $35,000/year salary
before bonuses. That separate affordability issue does not explain a missing
Part Time type or salary choices restricted to zero.

## Guarded club-policy workaround: implemented, live trial outstanding

The proposed change is **Ccsp only: info +0xE0, 4 -> 0**. The other six inputs
remain 4. This supplies Part Time alongside Non-Contract to the ordinary option
builder; it does not select financial terms or bypass other rules. Exact club
class RVA is **0x4CDA1F8**; metadata lives in `src/fields/fm27/policy.rs`.

Eligibility requires the exact supported platform/build/hash/class, valid
club-array membership/UID/info pointer, **all seven inputs equal to 4**, and a
validated human-manager contract/employer chain owning the selected club. The
code uses no particular club/player UID, staff-array position or fuzzy club name.
Unknown/mixed/already-Part-Time inputs and unmanaged clubs are refused.

Preparation retains array roots/sample and selected slots, club prefix, entire
info buffer, validated full packed game date, manager class/UID/contract pointer, and its
contract/team ownership buffers. Reads are bounded/exact and checked twice.
The opaque guard is bound to PID and worker attachment epoch. Apply/Undo recheck
the guard and original byte immediately before writing, then confirm the whole
guard by readback, allowing only that one byte to differ. This detects observable
stale contexts; raw memory access is not an atomic FM transaction.

The existing WriteTracker/acknowledged bridge/Undo path carries that guard.
Rendering, preparation and MCP never write. Default startup refuses policy
writes. Failed Undo remains retryable; an uncertain attempted write is reported
honestly. An API error with fully confirmed desired readback retains Undo and a
visible warning. Native short reads/writes are errors.

### Observed live result and Undo limitation

On the disposable fresh career at 13 July 2026, the user explicitly applied the
trial. Independent readback confirmed **0/4/4/4/4/4/4**, the same club identity,
info pointer/date and unchanged save hashes. The editor acknowledged its whole
guard readback; the independent pre-Apply capture retained only policy inputs,
identity and target address, not a full info preimage.

A subsequent Farnham offer to **Ryan Shotton (5129388)** showed **Part Time,
$1,400/year, 12 months (Jul 2026-Jun 2027)**. The user confirmed salary and
length dropdowns are changeable. The identity-matched working draft was type
0/wage 20 GBP/week; its second component remained 4/0. Ryan is a player-coach,
so the production current-contract inspector correctly refused that unwitnessed
class; research identity normalization did not enable current-contract editing.
No contract was submitted or accepted. The pre-change fresh-career offer was
Sam and the post-change offer Ryan; a same-player control remains outstanding.

The first live Undo was refused **before writing**. Independent readback found
the entire captured club prefix/info/date unchanged, with policy still applied.
The old guard retained mutable negotiation-session bytes in the manager prefix.
A regression reproduced that failure when a simulated offer changed those bytes;
the old generic message did not identify the exact stale live range.

The correction retains manager identity/contract fields individually, leaving
session state alone while preserving all other identity/ownership/build/date
checks. Error acknowledgements now distinguish a preflight refusal (no write
attempted) from uncertain post-write readback, identify the changed retained
range, and describe successful Undo as restoration.

On a new, separate test copy, the user subsequently confirmed that Apply and
Undo succeeded **after opening or changing an offer**, exercising the original
failure route. They then reapplied, saved and reloaded; Part Time remained
available. Independent read-only checks rediscovered the club at a new address
and confirmed **0/4/4/4/4/4/4** on 13 July (day 194). After the user advanced a
day, another repeated capture confirmed that policy on **14 July (day 195)**.
The original fresh save and external untouched backup still match their baseline
hashes. No agent memory writes or desktop operations were performed.

The intermediate restored buffer was not captured separately before the user
reapplied. That Undo result is user-confirmed and relies on the production
writer's guarded readback, rather than an independent full pre/post-Undo dump.
Full post-reload info/finance and post-day info/date windows are retained locally.
This verifies the reported Undo workflow and policy persistence; it does not
verify an accepted player contract by itself. The later accepted-contract evidence
below adds that observation; post-contract reload, salary payments and the
same-player before/after control remain outstanding.

### Accepted Part Time contract

The user finalised a deal for **Matthew Bondswell (UID 29210513)** and supplied
FM Contract Details showing Farnham, **Part Time Contract**, **$8K/year**,
start **25 July 2026**, expiry **30 June 2028** and **0.9% of overall payroll**.
Fresh identity discovery and repeated current-contract snapshots confirmed type
**0**, a matching embedded-person owner, and both current/owning club pointers
resolving to Farnham **8325096**. This is his current contract, not an offer or
future record. The wage is **114 GBP/week**; no external FX conversion was used.
Signed raw bytes `CE 46 EA 07` retain flags and decode to 25 July; expiry
`B6 00 EC 07` decodes to 30 June 2028. The club policy remains 0/4/4/4/4/4/4.

The current mapped wage total including bonuses reads 13,313 GBP/week, compared
with 13,198 on 13 July. The 115 GBP/week difference spans other game activity
and is not an isolated signing measurement or proof of a salary payment.
The FM payroll percentage demonstrates displayed payroll inclusion. Actual
salary deductions and this accepted contract's save/reload/day survival still
need checking. The editor did not supply financial terms or write the player's
contract; FM created the deal through the user's normal negotiation actions.

### Windows trial workflow

Use only a separate disposable save with an independently verified untouched
backup. Record full paths, distinct file identities, sizes and hashes. The editor
cannot establish the loaded save filename: confirm it in FM. Keep the backup
outside working saves if necessary, and rehash it before testing. A backup whose
hash changed must not be treated as untouched. Keep FM paused.

Build/run from the checkout:

```powershell
cargo build --bin fm-editor
.\target\debug\fm-editor.exe --mcp-port 7885 --enable-policy-trial
```

Choose an unused local port. In **Clubs**, select the actual managed club by
name/UID, open **Contract policy (trial)**, then **Prepare read-only policy
preview**. Unsupported records show a refusal and no Apply proposal. Review the
before/after club-wide effect; confirm the separate test copy/backup and click
Apply explicitly. Omitting the startup flag keeps policy writes disabled.

Before release/merge acceptance, perform these live checks on that test copy:

1. Capture full original info/identity/date buffers before Apply and, where
   possible, an unsubmitted same-player offer to compare before/after behaviour.
2. Apply; verify acknowledged and independent readback. Open a new unsubmitted
   negotiation for the same managed club and check Part Time/nonzero wage/length.
3. Undo before time advances; verify the policy and all unrelated captured bytes
   restore/persist as appropriate. Existing drafts and gameplay actions are not
   undone. Use a newly generated offer to test available types; an open draft
   may retain its own state.
4. Prepare/reapply; save under a new name, reload and advance one day. Check
   permitted types, an actual agreed fixed-term contract, employment and payroll.
   Choose terms within real board limits; no automatic salary/expiry is supplied.

New previews are required after reload/date changes. Byte-level Undo cannot
reverse game-world consequences after advancing time; the untouched backup is
the recovery path. Current-contract conversion remains disabled throughout.

### Read-only diagnostics

Debug builds also support a GUI-free server with no write bridge:

```powershell
.\target\debug\fm-editor.exe --mcp-only --mcp-port 7884
.\Tools\inspect-contract.ps1 -Uid 2000144100 -Port 7884
```

The UID is an example; use the intended player's UID. The client discovers the
actual schemas/port, refreshes identity and requests `snapshot_player_contract`.
`preview_club_policy_trial` runs production preparation checks read-only; it
exposes no reusable guard/token and reports `writes_enabled: false`. MCP remains
bound to **127.0.0.1**. The tracked `.mcp.json` is an example, not session setup.

## Verification and review state

The clean baseline passed **118** library tests. Final Windows checks passed
`cargo check --locked --all-targets`, `cargo test --locked` (**144 passed**, zero
failures), `cargo check --locked --release` and Windows debug/release builds. Release `--mcp-only` was also verified to
exit with an unsupported-mode error before any GUI or game attachment.
`cargo clippy --locked --all-targets` also completed: its 227 warning messages
(including duplicate targets) match the clean baseline, with no additional lint
categories/counts. Existing warnings were left outside this change's scope.
The PowerShell diagnostic client parsed and ran successfully against live FM,
confirming fresh UID/owner, Part Time code 0 and GBP/week units. The final
GUI-free server had no main window and listened only on 127.0.0.1; its policy
preview refused the already-applied pattern rather than offering a default.

Automated coverage includes unknown values, unsupported builds/platforms,
truncated/missing records, byte/date-flag preservation and invalid dates, no write on render,
stale identities/reloads/owners, failed and uncertain writes, acknowledged Undo,
mutable negotiation state, and desktop/narrow controls. Arbitrary-club UID and
multiple-manager cases explicitly exercise the generic identity-based design.

The earlier launch wrapper succeeded before this feature; it does not validate
the final trial. The user manually exercised the corrected GUI, including Undo
after negotiation, reapplication, save/reload and day advance. The agent did not
operate the desktop after the user's boundary. Native narrow-window workflow,
Linux CI, accepted-contract save/reload/day survival and salary-payment checks
remain outstanding. Current employment/ownership, fixed term and displayed
payroll inclusion have been observed for Bondswell.
Achievement compatibility remains **unconfirmed**.

## Files changed

- `src/club_policy.rs`: generic guarded preparation/write/readback and tests.
- `src/fields/fm27/policy.rs`, `src/fields/fm27/contract.rs`: narrow metadata.
- `src/fields/fm27/mod.rs`, `src/fields/fm27/player.rs`: registration and retained
  unmapped writable type.
- `src/bridge/cmd.rs`, `src/bridge/worker.rs`: preview, epochs, gate, staged failures.
- `src/undo.rs`, `src/write_tracker.rs`: acknowledged guard propagation.
- `src/app.rs`, `src/main.rs`, `src/lib.rs`: existing application integration.
- `src/ui/club_policy.rs`, `src/ui/club_panel.rs`, `src/ui/mod.rs`: generic trial UI.
- `src/ui/player_panel.rs`: explicit unsupported conversion/current-contract scope
  and monetary units, no-write rendering tests.
- `src/debug/tools/contracts.rs`, `src/debug/tools/club_policy.rs`: read-only tools.
- `src/debug/mcp_server.rs`, `src/debug/mod.rs`, `src/debug/tools/mod.rs`,
  `src/debug/tools/attach.rs`: discovery, GUI-free serving and attachment status.
- `src/process/mod.rs`, `src/process/win32.rs`: existing hash access and exact I/O.
- `Tools/inspect-contract.ps1`: bounded read-only Windows client.
- `README.md`, `docs/ui-guide.md`, `docs/fm27/memory-layout.md`,
  `docs/fm27/contract-type-investigation.md`: usage, witnesses and limits.

Raw addresses/captures, private save paths/hashes, executable artifacts and full
session chronology are local research outputs, excluded from the proposed PR.
