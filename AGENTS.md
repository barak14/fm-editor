# fm-editor

Real-time memory editor for Football Manager. Reads and writes `fm.exe` process memory (Win32
APIs on Windows, `process_vm_readv`/`process_vm_writev` against Proton on Linux) using a schema
selected at attach time. Supports FM24, FM26 and FM27. See [README.md](README.md) for users.

## First Principles

- **Two witnesses minimum.** One match is noise. Cross-reference at least two known-value entities.
- **Atomic field+doc edits.** Every offset lives in `src/fields/<version>/<entity>.rs` AND
  `docs/<version>/memory-layout.md`. Change both in one commit or neither.
- **Never trust inherited offsets.** Run verification yourself before acting on anything inherited.
- **All offsets through the schema.** No raw hex outside `src/fields/`. Everything goes through
  `fm.schemas()` (or a record's / the attached game's `version.schemas()`). There is no global
  active version: FM24, FM26 and FM27 can run side by side.
- **Writes need undo.** Anything calling `WriteProcessMemory` must round-trip through the undo
  stack. Never write speculatively.
- **Validate every pointer.** `ptr > 0x10000 && ptr < 0x7FFFFFFFFFFF` on every dereference.
  Handle failures — don't panic.
- **Delete dead code.** No compat shims, commented-out experiments, or half-finished refactors.

## Architecture

```
fields/               Shared types + per-version schemas (fm24/, fm26/, fm27/)
game_version.rs       Version enum + runtime dispatch via fm.schemas()
process/              Attach to fm.exe; win32.rs and linux.rs memory backends
bridge/               Worker thread → UI via mpsc (bulk cache, entity loads, writes)
player/staff/club.rs  Record-reading helpers — all offsets from fm.schemas()
ui/                   egui rendering — pages/ search layouts, *_panel.rs entity detail views
debug/                fm-debug MCP server (debug builds only) for live memory inspection
bin/                  Diagnostic binaries: scan_arrays (array RVAs), dump_manager
lib.rs / main.rs      Library crate root + thin app launcher
```

Memory layouts: [FM26](docs/fm26/memory-layout.md) · [FM24](docs/fm24/memory-layout.md) · [FM27](docs/fm27/memory-layout.md)

## Workflow

Four phases for mapping a new field. Skip one and you get drift.

1. **Research** — pick witnesses with known in-game values, find candidate offset via
   `Tools/dump-manager.sh` or `src/bin/scan_arrays.rs`. Reject if any witness doesn't match.
2. **Map** — determine encoding, add to `src/fields/<version>/<entity>.rs`, document in
   `docs/<version>/memory-layout.md`.
3. **Wire** — surface in UI panel via `field_widget::category_column` or a custom widget.
4. **Verify** — build, launch, navigate to a witness, confirm value matches in-game. Round-trip
   an edit through undo.

Non-mapping work (UI, refactors, fixes): skip the methodology, still use a branch.

## Verification Tiers

| When | Do |
|---|---|
| After editing `.rs` files | Tier 1: compile check per file |
| After changing runtime behavior | Tier 2: `bash Tools/build-and-launch.sh` |
| After mapping a field / changing a write path | Tier 3: live verification against fm.exe |

## Build

```bash
cargo build                      # compile only
bash Tools/build-and-launch.sh   # kill orphan → build → launch → verify survived
```

Requires `fm.exe` running with a save loaded. Version auto-detected from each process's install
path; with several games open, pick one in the toolbar (diagnostic scripts take `--pid <n>`).

## Tools

| Script | Purpose |
|---|---|
| `build-and-launch.sh` | Kill orphan, build, launch, verify survived |
| `create-worktree.sh <name>` | New branch + worktree under `.claude/worktrees/` |
| `rebase-worktree.sh <name>` | Validate + build + rebase onto main, prints commit list |
| `ff-merge-worktree.sh <name>` | Fast-forward merge into main + cleanup |
| `kill-fm-editor.sh` | Kill running fm-editor.exe |
| `dump-manager.sh` | Human-manager byte dump |
| `scan-arrays.sh` | Array RVA scanner |

## fm-debug MCP server

Debug builds serve an MCP server (streamable HTTP, `POST /mcp`) on `localhost:7799` while the
editor runs — `--mcp-port <n>` overrides it, and worktree launches use the port written to
`.mcp-port`. `attach_status` follows the editor's game when several are open; pass `pid` to
switch. It exposes read-only tools for the Research and Map phases: `attach_status`,
`search_entities`, `dump_struct`, `probe_range`, `probe_pointer_chain`, `compare_entities`,
`read_entity_fields` and more. [`.mcp.json`](.mcp.json) is an example client config.
