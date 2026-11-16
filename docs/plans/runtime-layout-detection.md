# Plan: runtime layout detection for unrecognised builds

Status: proposed, not started. Written 2026-11-13, revised 2026-11-16
(schema split, two phases, gaps from review).

## Goal

Attach to any build of a supported release (FM24, FM26, FM27), including
storefronts and executables the editor has no recorded schema for (FM24/FM26
Game Pass, FM26/FM27 Epic, a patched or re-signed exe), by checking or finding the
build-specific addresses at runtime. A build whose addresses match a recorded
schema can be edited straight away; one whose addresses were found at new
locations can be edited after the user accepts an "unvalidated build" warning;
when only part of the layout is found, the build opens read-only and the
fields that were found can be edited after a stronger "edit anyway" warning.
Every detected build is marked by an amber version label in the status bar.

Out of scope: unknown releases (FM28) and patches that change record layouts.
Detection must refuse those rather than guess.

The work ships in two phases:

- **Phase 1: schema split, validation gate, recorded-address fast path.** No
  memory scanning. Covers any build whose addresses equal a recorded build's
  (e.g. an FM27 Steam patch that did not move them). FM24 Epic is already a
  recorded build (branch `fm24-epic`, below).
- **Phase 2: address scanning, the warning dialog and accepted builds.**
  Covers builds whose addresses moved.

Each phase is independently mergeable and verifiable.

## Why this is feasible

A schema holds two kinds of data:

1. **Record layouts** (field offsets, encodings, struct sizes, sub-records,
   player lists, shifts). These are fixed per release. FM24 and FM26 are
   end-of-life, so their layouts will not change again. FM27 Game Pass
   matched Steam's record layouts (`FM27_GAMEPASS` overrides only addresses,
   `src/fields/fm27/mod.rs`).
2. **Module-relative addresses**, which differ per binary:
   - `rva_humans_data/end`, `rva_staff_data/end`, `rva_clubs_data/end`
   - `rva_stadium_base`
   - `rva_current_date`
   - `full_finance_vtable_rva`, `player_coach_vtable_rva`,
     `human_manager_vtable_rva`
   - `known_dll_hash` (fingerprint, not an address)

Only group 2 needs checking or detecting. Reference data from FM27 Steam vs
Game Pass:

| Address | Steam | Game Pass | Shift |
|---|---|---|---|
| humans data | 0x57037D0 | 0x53B7DE0 | -0x34B9F0 |
| staff / clubs | same cluster | same cluster | -0x34B9F0 (block) |
| stadium root | 0x5703750 | 0x53B7D60 | -0x34B9F0, assumed; not yet checked |
| current date | 0x56AD378 | 0x53597C8 | -0x353BB0 |
| full finance vtable | 0x4CF0CC8 | 0x4A2F9E8 | -0x2C12E0 |
| player-coach vtable | 0x4F75A08 | 0x4C5FE88 | -0x315B80 |
| human-manager vtable | 0x4CC086C | 0x4A072BC | -0x2B95B0 |

The entity-array cluster moves as a block, so finding one array finds the rest,
but the date and each vtable move independently and need their own detection.

FM24 Epic is a recorded build (`FM24_EPIC`, branch `fm24-epic`): FM24 Steam's
layout and addresses with the Epic fingerprint `0x8FA113DB9DCD8883`, product
version `24.4.2+2081827`, from evidence contributed through the fork at
github.com/mehmetaligumusler/fm-editor (see docs/fm24/memory-layout.md, "Epic
build"). It is the first example of a different binary of a release keeping
its recorded addresses, which is what Phase 1's fast path relies on.

### Addresses outside the schema

Two FM27 features keep their own build-specific addresses and fingerprints
outside `VersionedSchemas`:

- club policy: `src/fields/fm27/policy.rs` (`club_vtable_rva`, `hash`), gated
  in `src/club_policy.rs:208` on `GameBuild::Fm27Steam` plus that hash;
- debug contract inspection: `src/fields/fm27/contract.rs`
  (`contract_vtable_rva`, `module_header_hash`), gated in
  `src/debug/tools/contracts.rs:14` the same way.

They are not detected. On every detected build they stay unavailable, and
they do not count towards the Complete/Partial outcomes below. After the
`GameBuild` change, those `== GameBuild::Fm27Steam` checks must remain true
only for the recorded FM27 Steam build (test required, see Phase 1).

## Schema split (Phase 1)

Replace the single `VersionedSchemas` with two types:

- **`ReleaseLayout`**: group 1 plus everything else that is per release:
  field lists, sub-records, player lists, person profile, labels, struct and
  read sizes, `stadium_entity_stride`, `human_manager_shift`,
  `manager_shifted_fields`, `player_person_offset`, `player_coach_shift`,
  `age_reference_year`. Exactly one `static` per release (`fm24::FM24`,
  `fm26::FM26`, `fm27::FM27`). `field()`/`offset_of()` keep their per-layout
  key map; layouts are always static, so the existing pointer-keyed lookup
  stays correct and O(1) without a fallback scan.
- **`ModuleAddresses`**: group 2 as a small `Copy` value (array RVAs, stadium
  root, date, the three class vtable RVAs). 0 keeps its current meaning
  ("unknown / not used").

Recorded builds become `RecordedBuild` statics: `{ version, storefront,
layout: &'static ReleaseLayout, addresses: ModuleAddresses, fingerprint: u64,
fmrte_* counts }`. FM27 Steam and Game Pass share `fm27::FM27` and differ only
in `addresses` and `fingerprint`. The `fmrte_*` counts are diagnostics for the
mapping save and live on the recorded build, not the layout.

`GameBuild` stays `Copy` and becomes:

```rust
pub struct GameBuild {
    pub version: GameVersion,
    pub storefront: Storefront,
    pub layout: &'static ReleaseLayout,
    pub addresses: ModuleAddresses,
    pub recognition: Recognition,
}
pub enum Recognition { Recorded, Detected(DetectionOutcome) }
```

with associated constants (`GameBuild::FM24_STEAM`, `FM24_EPIC`, `FM26_STEAM`,
`FM27_STEAM`, `FM27_GAMEPASS`) for the recorded builds and tests, and derived
`PartialEq` over all fields. A detected build therefore never compares equal
to a recorded constant, which keeps the policy/contract gates recorded-only.
No schema is ever built at runtime, so nothing is leaked or interned.

Call sites (about 300 uses of `schemas()`): `fm.schemas()` and
`build.schemas()` return `&'static ReleaseLayout`, so field-lookup call sites
keep compiling. The ~60 address reads in `src/process/mod.rs`, `src/player.rs`,
`src/staff.rs`, `src/club.rs`, `src/bridge/entity_load.rs`,
`src/club_policy.rs`, `src/debug/tools/{attach,contracts}.rs` and
`src/bin/{scan_arrays,dump_manager}.rs` move to `fm.addresses()` /
`build.addresses`. The compiler finds them all once the fields leave the
layout type.

`src/fields/<release>/mod.rs` and the matching `docs/<release>/memory-layout.md`
change together (AGENTS.md); the docs only need their schema/RVA section
headings updated to the new type names. Values do not change, and existing
per-build tests (`src/fields/mod.rs:809`, `:1045`, `:1134`, `:1151`) move to
the recorded-build statics unchanged in value.

## Validation gate (Phase 1)

Decode a sample of entities end to end with the candidate `(layout,
addresses)`. The thresholds below are fixed so the gate gives a quick yes or
no: fixed sample sizes, one pass, a target of under a second on a loaded
save. They are not tuned per build. Change a value only if a forced run on
a build available here gives the wrong answer, and record the change here:

- **Players:** 256 entries spread evenly over the array (fewer only when the
  array is smaller). At least 95% must have a name that resolves to a valid
  FmString, a non-zero UID unique within the sample, a date of birth between
  1900 and the release year, CA in 1..=200 and PA valid under the release's
  PA encoding (negative range codes count as valid).
- **Staff:** 128 entries, at least 95% with a valid name, unique non-zero UID
  and date of birth in range.
- **Clubs:** 128 entries, at least 95% with a valid name. Where the address
  set has a non-zero `full_finance_vtable_rva`, at least one sampled or
  scanned club must carry that class and decode plausible budget values.
- **Related records:** for sampled players with a contract pointer, at least
  90% must resolve and decode the contract. Where `rva_stadium_base` is
  non-zero, the stadium chain must resolve and its first entries decode.
- Every dereference uses the existing pointer and range checks; a read error
  counts as a failed sample, never a panic.

The gate's report names each check, its count and pass rate, so a refusal
message can name the failing step.

The gate runs on every attach and on every save load or reload, including
when addresses come from a cache (Phase 2). On a recorded build whose
fingerprint matches, a failure is logged and shown as a warning but does not
block, because the fingerprint is existing evidence. On every other build a
failure refuses the attachment.

## Attach and save timing

Entity arrays are empty until a save is loaded, so neither the gate nor
detection can run at the main menu. Today the worker attaches, reports
`Attached`, and `PingTransition::SaveReady` (`src/bridge/worker.rs:115`)
promotes `WaitingForSave` to `Ready` once counts are stable.

- Phase 1: candidate addresses are read during `WaitingForSave`. A candidate
  whose array words are null or `data == end` means "no save yet"; one whose
  words are not a valid pointer pair is rejected at once and the next
  candidate is tried. On `SaveReady`, run the gate; promote to `Ready` only
  if it passes.
- Phase 2: when no candidate set reads as empty-or-valid, the worker reruns
  the scan while waiting for a save, at most every 5 s.
- Every save reload or reconnect reruns the gate (and detection if the
  addresses are not yet known). Undo entries are never carried across, as
  today.

## Phase 1: recorded-address fast path

Attach flow:

1. Read release and storefront as today. `GameBuild::resolve` no longer fails
   for a known release on an unrecognised storefront (FM26/FM27 Epic,
   FM24/FM26 Game Pass); it returns candidates instead of an error. Unknown releases are
   still refused.
2. **Pinned recorded build:** storefront and fingerprint match a recorded
   build whose fingerprint is non-zero. Use it with `Recognition::Recorded`.
3. **Otherwise** (fingerprint mismatch, unrecognised storefront, or a recorded
   build with fingerprint 0 such as FM27 Steam today): try the address sets
   of every recorded build of that release, the storefront's own first (FM27:
   Steam then Game Pass). The first set that passes the gate gives
   `Recognition::Detected(MatchesRecorded)`. None passing gives `Failed` and
   the attach is refused with the gate's message.

FM27 Steam's fingerprint is 0 today, so it moves from "recorded with a
notice" to "matches recorded" with the amber label until it is calibrated.
`club_policy` already pins an FM27 Steam Windows fingerprint
(`policy.rs:16`); if it is the same function's output, it is a lead for
`known_dll_hash`, to be confirmed on a live FM27 Steam build before it is
recorded.

UI and reporting in Phase 1:

- The status bar and picker show the build with its storefront. Detected
  builds use the amber label with the tooltip "This build hasn't been
  validated. Back up your save before editing."
- `dll_hash_notice` is replaced by a structured detection report: release,
  storefront, product version, module hash, candidate tried, outcome and the
  gate report. `attach_status` in `src/debug/` exposes it, and it is logged.
- "Matches recorded" builds can be edited with no prompt (AGENTS.md).

Phase 1 code changes:

- `src/fields/mod.rs`, `src/fields/fm24|fm26|fm27/mod.rs`: the split above.
- `src/game_version.rs`: new `GameBuild`, `Recognition`, resolve returning
  candidates, constants for recorded builds.
- `src/process/mod.rs`: `attach_pid` candidate selection, the report,
  `fm.addresses()`.
- New `src/process/layout_validate.rs`: the gate, built from pure functions
  over byte buffers so it can be unit tested without a game.
- `src/bridge/worker.rs`: gate on `SaveReady` and on reload; refuse and
  report on failure.
- `src/app.rs` status bar/picker: amber label and tooltip.
- `src/debug/tools/attach.rs`: report in `attach_status`.

Phase 1 tests:

- Unit: every recorded build's layout and addresses equal the current values;
  `resolve` accepts FM24/FM26/FM27 on every storefront and rejects unknown
  releases (replaces `resolve_rejects_unmapped_storefront`,
  `src/game_version.rs:187`); a detected FM27 build is not
  `GameBuild::FM27_STEAM`, so `club_policy::layout_for` and the contract
  inspection refuse it; gate scoring on synthetic buffers (valid player
  array, garbage, a staff array read as players).
- Live, FM24 Steam, FM26 Steam, FM27 Game Pass: normal attach stays
  `Recorded` with no amber label; forcing a fingerprint mismatch
  (debug-only env var `FME_IGNORE_FINGERPRINT=1`) gives "matches recorded",
  the amber label, and an edit that round-trips through Undo; the gate
  refuses FM24's layout against FM26 (a debug-only override of the layout).
- Live: attach at the main menu, then load a save; the gate runs on
  `SaveReady` and again after loading a different save.

Phase 1 docs: `README.md` supported-versions section (recorded vs matched
builds, amber label, backups), `docs/ui-guide.md` (amber label, status
text), `docs/development.md` (detection report, forcing the fast path,
schema types), `CHANGELOG.md`.

## Phase 2: address detection

### Algorithm (new module `src/process/layout_detect.rs`)

Input: an attached process, its release layout and the recorded address sets
for that release. Runs only after Phase 1's fast path found no passing set.
Output: a detected `ModuleAddresses` plus the report, or an error naming the
failing step.

1. **Array cluster.** Scan the module's writable, non-executable sections
   (the section walk moves from `src/bin/scan_arrays.rs` into the library) for
   8-byte aligned `(data, end)` pointer pairs with `end > data`,
   `(end - data) % 8 == 0`, both valid heap pointers and a count in
   `[1_000, 1_000_000]`. For each candidate, sample ~64 elements and score
   them with the gate's player checks. The best-scoring pair with >90% valid
   samples is the humans array. Derive staff/clubs/stadium from a recorded
   build's relative offsets within the cluster and score each with its own
   checks; if one fails, scan for that array alone.
2. **Class identifiers (vtables).** Count vtables (first qword, pointing into
   the module) across each array:
   - player-coach: the class in the humans array whose identity fields decode
     only at `player_person_offset + player_coach_shift`;
   - human manager: the class in the staff array whose identity decodes only
     with `human_manager_shift` applied;
   - full finance: among club finance objects, the class whose budget fields
     decode as plausible money values (FM27: ~900 clubs; the small class
     shares the same base layout). On Steam and Game Pass the small class
     vtable sits 0x4D0 below the full one; a cross-check, not the rule.
   Only classes whose RVA is non-zero in the release's recorded builds are
   searched (FM24 and FM26 have none). A class not found gets RVA 0 and its
   dependent fields (finance sub-record, player-coach handling) are dropped
   rather than read with the wrong shape.
3. **Current date (best effort).** The date only feeds displayed ages and
   never affects the outcome. Scan for statics holding a plausible
   DateDayYear (as `scan_arrays --date` does) in one pass. Use the candidate
   when exactly one exists; otherwise `game_date()` returns None and ages use
   the season estimate (the UI already handles None). FM24 has two such
   statics at the start of a save (the clock and the start-date copy,
   `src/fields/fm24/mod.rs:68-73`), so None is the expected result there.
4. **Fingerprint.** Record the module hash in the report so a later commit
   can promote the build to a recorded one.
5. **Validation gate** (Phase 1) on the assembled addresses.

Detected addresses are cached on disk keyed by (module path, module hash);
a cached entry is never used for a different hash, and the gate still runs on
every attach and save load.

### Editing policy

Outcomes, stored in the report (`MatchesRecorded` comes from Phase 1):

- **Matches recorded:** a recorded address set passed the gate; only the
  fingerprint or storefront differs. Editing allowed with no warning.
- **Complete:** the entity arrays (humans, staff, clubs, plus the stadium root
  where the release's recorded builds have one) and every class identifier
  those builds use were found at new addresses, and the gate passed. Editing
  allowed after the warning below.
- **Partial:** the gate passed for the arrays, but at least one class
  identifier was not found. Dependent fields are dropped and the build opens
  read-only; the remaining fields can be edited after the "edit anyway"
  warning below.
- **Failed:** an entity array was not found or the gate failed. Attach is
  refused.

The date, club policy and contract inspection never affect the outcome.

Every detected build shows the amber label from Phase 1. Builds that are
read-only add "(read-only)" and the reason, e.g. "FM24 Game Pass (read-only:
finance class not found)".

Warning for **Complete** builds, a modal shown on the first edit attempt and
from a status-bar "Enable editing…" button:

> The version *FM24 Game Pass 24.4.2* hasn't been validated; editing may
> produce errors. Back up your save before editing and use at your own
> discretion.

Buttons: "Enable editing" and "Stay read-only". The choice is remembered per
module hash in settings, so a game update asks again. Settings lists
remembered builds with "Revoke". Until accepted, the build is read-only.

"Edit anyway" for **Partial** builds uses the same modal and storage, with
the reason added to the text and an explicit list of what stays disabled:

> Part of the layout for *FM24 Game Pass 24.4.2* wasn't found (finance class).
> Finance fields are disabled. The other fields haven't been validated;
> editing may produce errors. Back up your save before editing and use at
> your own discretion.

Buttons: "Edit anyway" and "Stay read-only". The acceptance records the
outcome and the list of missing classes, so it is asked again if a later
detection on the same hash finds a different set. Dropped fields are never
editable, whatever was accepted: they are removed from the schema view for
this attachment, and the worker refuses writes that fall inside them.

Enforcement is in the worker: Complete and Partial builds get a writable
handle (so acceptance needs no reattach), and the worker refuses writes
until it has received an acceptance message for this attachment's module
hash and outcome. Revoking
sends a message that takes effect immediately for the current attachment.
After a revoke, Undo of writes already acknowledged in this session stays
allowed, since it only restores recorded original values; new edits are
refused.

### Phase 2 code changes

- `src/process/layout_detect.rs` (new): steps 1-4, pure scoring functions over
  byte buffers.
- `src/bin/scan_arrays.rs`: use the shared section walk and date scan.
- `src/process/mod.rs`: run detection after the fast path; writable handle
  for Complete and Partial (writes gated by the worker); detection cache.
- `src/bridge/worker.rs`: rescan while waiting for a save; acceptance and
  revoke messages; write refusal without acceptance.
- `src/settings.rs`: `accepted_unvalidated_builds`, a list of
  `{ release, storefront, product_version, module_hash, outcome,
  missing_classes }`.
- `src/app.rs` or new `src/ui/unvalidated_build.rs`: the modal and the
  Settings list with "Revoke".
- `src/ui/navigation.rs` (`editing_enabled`): hide edit controls to match the
  worker.
- Progress in the UI ("Detecting layout…") while detection runs on the worker
  thread.

### Phase 2 tests

- Unit: array scoring (valid array, garbage, staff mistaken for players);
  class census; date selection (one candidate, several, none); cache keyed by
  hash.
- Live, FM24 Steam, FM26 Steam, FM27 Game Pass: force detection by ignoring
  the recorded addresses (debug-only `FME_FORCE_DETECT=1`). Detection must
  reproduce every recorded array, stadium and class RVA exactly; the date
  must match the recorded value or be None, never a different value.
- Live: on a forced Complete detection, the warning appears on the first
  edit, "Stay read-only" blocks the write in the worker, "Enable editing"
  allows an edit that round-trips through Undo, and Revoke blocks the next
  edit while Undo still works. Force a Partial detection (blank one class
  identifier): it opens read-only, "Edit anyway" allows an edit to a
  remaining field that round-trips through Undo, and a write to a dropped
  field (e.g. a finance budget) is refused by the worker.
- Other storefronts and builds without witnesses here: rely on user reports. Ask reporters for
  the detection report from `attach_status` or the game-detection log.

### Phase 2 docs

`README.md` (detected builds, the warning), `docs/ui-guide.md` (warning,
read-only reasons, revoking acceptance), `docs/development.md` (forcing
detection, promoting a detected build to a recorded one),
`docs/<release>/memory-layout.md` only when a build is promoted,
`CHANGELOG.md`.

## Decisions

- Label colour: amber for now; revisit later if it reads poorly in a theme.
- Partial detections get "edit anyway" for the fields that were not dropped
  (2026-11-16).
- Gate thresholds are fixed values chosen for a fast yes/no, not tuned per
  build (2026-11-16).

## Related: worth taking from the fork

From the review of github.com/mehmetaligumusler/fm-editor (fetched locally as
`refs/remotes/mehmet/main`), separate from this plan:

- History improvements, current session only: per-action and listed undo,
  delete entries, hide undone by default, open profile from older entries,
  change badges and the player-page undo-all summary.
- Currency/wage-period display, after fixing drag boxes that write a rounded
  or converted value when focused and blurred without typing.
- Turkish UI layer (`i18n.rs`), labels only, without the rustfmt churn in
  `src/fields/fm24/`.
- Skip: cross-session undo revival, injury healing (wrong absence offset,
  clears bans), heap-scan save-name detection, the committed exe.
- FM24 Epic is a recorded build from the fork's contributed evidence (branch
  `fm24-epic`). It keeps Steam's date `0x631D5BC`; the fork's `0x642F458`
  kept the start date on Steam.
