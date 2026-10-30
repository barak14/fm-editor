# fm-editor — project guidance

Real-time `fm.exe` memory editor for FM24/FM26/FM27: Win32 on Windows,
`process_vm_readv`/`process_vm_writev` against Proton on Linux. Schemas are selected
per attached build (release + storefront), not globally. Game Pass support is
unverified; never assume another build's layout is compatible.

## Safety invariants

- **Schema-only offsets.** Resolve memory offsets through the attached process or
  record's schema (`fm.schemas()` / `build.schemas()`). No hardcoded layout offsets
  in readers, writers or UI. Keep different attachments/builds independent.
- **Evidence before mapping.** Confirm new or changed mappings against at least two
  entities with independently known in-game values. Inherited notes are leads, not
  verification for a new build or a changed mapping.
- **Schema + documentation together.** Update `src/fields/<version>/` and the matching
  `docs/<version>/memory-layout.md` in the same change. Record build, encoding and
  witnesses; label unverified coverage honestly.
- **Validate pointers and buffers.** Every dereference needs
  `ptr > 0x10000 && ptr < 0x7FFFFFFFFFFF` plus checked read/range handling. Missing,
  invalid or unmapped data must not panic, fabricate a value or trigger a write.
- **Writes require confirmed undo.** Use the bridge's acknowledged-write/undo path;
  never write speculatively or as a side effect of rendering/clamping. Keep failed
  writes visible and retryable; disable editing while disconnected or awaiting a
  write acknowledgement. Never reuse undo addresses across saves/reconnections.
- **Bind UI data to identity.** Async results and retained buffers must match the
  active entity's type/address. Failed loads must not expose a previous entity's
  editable data. Resolve related clubs by cached object identity, not fuzzy names.
- **Derive relationships from the save.** Resolve the managed club and related
  squads from the attached save, and rebuild membership on reload/reconnection.
  Keep current club and owning club distinct for loans. Witness UIDs belong in
  tests and documentation, never as runtime rules for a particular save.

## Code map

- `src/fields/`, `src/game_version.rs`: per-build layouts and dispatch.
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
| Field mapping | Live FM verification for the affected build against independently known in-game values; read-only mappings require no test writes |
| Write path | Live FM verification for the affected build; round-trip an edit through Undo and confirm restoration |
| Documentation only | Check paths, links and consistency; no game launch needed |

Report what actually ran and any missing live evidence. A successful launch or an
old `.launch-verified` file does not prove the current behaviour or field layout.
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
