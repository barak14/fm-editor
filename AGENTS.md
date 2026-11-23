# fm-editor — project guidance

Real-time `fm.exe` memory editor for FM24/FM26/FM27: Win32 on Windows,
`process_vm_readv`/`process_vm_writev` against Proton on Linux. The goal is one
editor that works with every copy of these games the user owns, whatever the
storefront. Prefer making an unrecognised build work safely at runtime over
refusing it.

Schemas are selected per attached build (release + storefront), not globally.
A **recorded** build has a schema in `src/fields/` with its documented source. An
**unrecognised** build of a supported release reuses that release's record
layouts; its module addresses must be confirmed or detected at runtime and
validated by decoding before use (plan: `docs/plans/runtime-layout-detection.md`).
A detected build whose addresses match its release's recorded schema may be edited
without a prompt; one found at new addresses may be edited once the user accepts an
"unvalidated build" warning; a partially detected one is read-only until the user
accepts an "edit anyway" warning, and its undetected fields are never editable.
Detected builds are marked as unvalidated in the status bar.

## Safety invariants

- **Schema-only offsets.** Resolve memory offsets through the attached process or
  record's schema (`fm.schemas()` / `build.schemas()`, a `ReleaseLayout`). No hardcoded layout offsets
  in readers, writers or UI. Keep different attachments/builds independent.
- **Record mappings with their source.** A mapping may be recorded from any credible
  source: our own live research, a contributor, another editor's published offsets
  or a Cheat Engine table. Record where it came from, the date and the exact build,
  and mark it "not checked here" until it is confirmed against in-game values on that
  build. Live confirmation is welcome but not a precondition; the runtime decoding
  check, pointer validation and confirmed undo are the safety net.
- **Validate detected layouts.** Runtime-detected addresses must pass a decoding
  check on sample players, staff and clubs before any read is shown; refuse to
  attach when it fails rather than display guesses. Release-specific record layouts
  are never detected or guessed; an unknown release (e.g. FM28) is refused.
- **Schema + documentation together.** Update `src/fields/<version>/` and the matching
  `docs/<version>/memory-layout.md` in the same change. Record build, encoding and
  source; label coverage that has not been checked here honestly.
- **Validate pointers and buffers.** Every dereference needs
  `ptr > 0x10000 && ptr < 0x7FFFFFFFFFFF` plus checked read/range handling. Missing,
  invalid or unmapped data must not panic, fabricate a value or trigger a write.
- **Writes require confirmed undo.** Use the bridge's acknowledged-write/undo path;
  never write speculatively or as a side effect of rendering/clamping. Keep failed
  writes visible and retryable; disable editing while disconnected or awaiting a
  write acknowledgement. Never reuse undo addresses across saves/reconnections.
  Editing rules for detected builds (above) are enforced by the worker, not only
  the UI.
- **Bind UI data to identity.** Async results and retained buffers must match the
  active entity's type/address. Failed loads must not expose a previous entity's
  editable data. Resolve related clubs by cached object identity, not fuzzy names.
- **Derive relationships from the save.** Resolve the managed club and related
  squads from the attached save, and rebuild membership on reload/reconnection.
  Keep current club and owning club distinct for loans. Witness UIDs belong in
  tests and documentation, never as runtime rules for a particular save.

## Code map

- `src/fields/`, `src/game_version.rs`: per-release `ReleaseLayout` record layouts, per-build `RecordedBuild` addresses and fingerprints, and dispatch.
- `src/process/`: platform memory backends; `src/bridge/`: worker/UI messages.
- `src/player.rs`, `src/staff.rs`, `src/club.rs`: schema-based record reads.
- `src/app.rs`, `src/ui/`: egui application, search pages and detail panels.
- `src/debug/`: read-only fm-debug MCP server in debug builds.
- `src/bin/`: `scan_arrays` and `dump_manager` diagnostics.

## Verification

| Change | Required evidence |
|---|---|
| Rust code | `cargo check` and relevant tests; `cargo test` for integration |
| Runtime/UI behaviour | `bash Tools/build-and-launch.sh`, then exercise the affected workflow; inspect UI changes at desktop and narrow sizes |
| Field mapping | Live FM verification against in-game values where the build is available here; otherwise record the mapping's source and mark it not checked here. Read-only mappings require no test writes |
| Write path | Live FM verification on a build available here; round-trip an edit through Undo and confirm restoration |
| Layout detection | Force detection on each available build and confirm it reproduces that build's recorded addresses; confirm the validation check refuses a wrong layout |
| Documentation only | Check paths, links and consistency; no game launch needed |

Report what actually ran and any missing live evidence. A successful launch or an
old `.launch-verified` file does not prove the current behaviour or field layout.
Builds that cannot be run here (FM27 Steam, FM24/FM26 on other storefronts) are
supported through detection and user reports; say so rather than blocking on them.
Format only owned Rust modules (`rustfmt --config skip_children=true ...`).

## Working context

Use a feature branch/worktree for code changes to keep the main editor available.
Keep existing worktrees and unrelated changes intact; do not merge or remove them
without the user's direction. The harness owns planning, delegation and temporary
task state; no additional repository-specific agent ceremony is required.

- [Development procedures](docs/development.md): field research, launch/debug and worktree helpers.
- [README](README.md): setup and supported builds; [UI guide](docs/ui-guide.md): current behaviour.
- [ROADMAP](ROADMAP.md): feature backlog, not a command to implement every entry.
- [Memory layouts](docs/fm27/memory-layout.md): FM27; also [FM26](docs/fm26/memory-layout.md) and [FM24](docs/fm24/memory-layout.md).
