# Development procedures

Read [AGENTS.md](../AGENTS.md) for project invariants. This document contains
on-demand procedures, not extra planning/delegation rules for a particular model.
For current functionality use [the UI guide](ui-guide.md) and source code, not old
implementation plans.

## Checks and launch

```bash
cargo check
cargo test
bash Tools/build-and-launch.sh
```

Run from the worktree being changed. The launch wrapper kills only that branch's
editor, builds it, starts it and checks that it survives startup. It preserves the
main editor when launching a feature branch. Runtime workflows still need testing
after startup; the script is not a UI or memory-layout test.

A loaded game is needed for live editing tests, not for compile/unit tests or
read-only disconnected UI checks. Never change system permissions, disable build
fingerprint checks or assume a Steam schema works on Game Pass to make a test pass.
See [Linux setup](../README.md#linux--proton) for the available memory-access options.

Worktree launches use `.mcp-port` and a branch-specific executable
`fme-<branch>` (`.exe` on Windows). Logs are `/tmp/fme-<branch>.log`; main uses
`/tmp/fm-editor.log`. A build failure and a startup panic are different failures:
read the compiler output or the corresponding launch log before changing code.

## Headless FM session (Linux)

`Tools/fm-headless.sh` runs FM on a headless labwc compositor with XWayland on
the GPU, so live checks can screenshot and drive the game without using the
desktop. It needs `labwc`, `wlr-randr`, `grim` and `xdotool`.

Set this Steam launch option once per FM version (use the main worktree's path):

```
env $(bash /path/to/fm-editor/Tools/fm-headless.sh launch-env 2>/dev/null) %command%
```

`launch-env` prints `DISPLAY` and a frame cap (`DXVK_FRAME_RATE`,
`VKD3D_FRAME_RATE`) only while the compositor is alive, and nothing otherwise,
so FM opens normally on the desktop whenever the harness is stopped. Measured
on the FM27 3D main menu (7900 XTX shared with the desktop): FM added about 22
percentage points of GPU load with a 60 Hz output and about 10 at 15 Hz, so
the output rate alone does not throttle it. The DXVK cap is there to do that;
its effect has not been measured yet.

```bash
bash Tools/fm-headless.sh start           # optional WxH, default 1920x1080
bash Tools/fm-headless.sh launch fm27     # fm24 | fm26 | fm27 | <appid>
bash Tools/fm-headless.sh status          # shows whether FM is on it
bash Tools/fm-headless.sh shot            # grim capture; prints the PNG path
(eval "$(bash Tools/fm-headless.sh env)"; xdotool mousemove 400 280 click 1)
bash Tools/fm-headless.sh stop            # refuses while FM is on it
```

`FMH_FPS` (default 30, `0` uncapped) and `FMH_REFRESH` (output Hz, default 30)
are read by `start`, and the cap applies to the next FM launch. Run `env`
clients in a subshell as above: exporting the headless `DISPLAY` into a
long-lived shell makes later commands open there instead of on the desktop.

The compositor is detached from the shell that starts it, because FM loses its
display and will likely crash if the compositor stops. Save and quit FM before
`stop`; `stop --force` skips that check. To drive write/undo checks, start the
editor with the `env` output too, so its window is in the same session.
Screenshots supplement the in-game comparison described below; they do not
replace checking against FM's own values. Verified with FM27 (Steam, Proton)
only: render, `grim` capture, `xdotool` clicks and typing. FM24 and FM26 use the
same launch option but have not been run headless yet.


## Field research and verification

This procedure is for new/changed mappings. UI-only changes to existing fields do
not need fresh offset research, but changes to writes still require a live edit/undo
check. Verification is specific to the attached build, not a project-wide stamp.

1. **Identify the build and field.** Inspect its existing schema and matching
   `docs/<version>/memory-layout.md`. Check the attached game, storefront and module
   fingerprint before interpreting bytes. Unverified schemas remain unverified.
2. **Choose at least two witnesses before scanning.** Record UIDs and values from
   the game's own UI, preferably distinct values. Record the build/save context,
   UIDs and observed values in the matching memory-layout document; historical
   values are not evidence for the current save/build.
3. **Read candidates.** Use fm-debug or the diagnostics below. Follow validated
   pointer chains and cross-check every candidate against every witness. Reject a
   candidate if any witness disagrees; plausible bytes alone are not evidence.
4. **Establish representation.** Confirm width, encoding, sentinel values, flags
   and sub-record ownership against the bytes and in-game values. Follow the
   existing schema types; do not guess an encoding or copy offsets from another
   build. Keep layout constants in `src/fields/`, not UI/readers/writers.
5. **Map and document atomically.** Update the appropriate per-build schema and
   `docs/<version>/memory-layout.md` together, including witnesses and verification
   scope. A missing doc update is not an accepted shortcut for a small mapping.
6. **Wire and test.** Use existing record readers/widgets and the bridge's
   acknowledged-write/undo path. Add regression tests for encoding/ranges, missing
   data and rendering without edits where applicable. Run compile/tests and launch.
7. **Verify live.** Navigate to the witnesses and compare displayed values with FM.
   For editable fields, record the original, make a controlled edit, confirm it in
   FM, Undo and confirm restoration. FM caches an entity's displayed data once it
   has shown it in a session, and navigating away and back does not refresh it.
   Edit before FM first displays the witness, or let one game day pass (Calendar >
   Go On Holiday) to refresh it; confirm restoration in memory. Restore test changes and
   state exactly which builds/workflows were verified. If FM is unavailable, report
   the missing gate rather than calling the mapping verified.

Schema paths are per version/build under `src/fields/<version>/`; there are no flat
`src/fields/player.rs` or `docs/memory-layout.md` files to update.

## Read-only memory tools

Debug editor builds serve streamable HTTP MCP at `http://localhost:7799/mcp`.
`--mcp-port <n>` overrides the port. Worktree wrappers use the port in `.mcp-port`.
A disconnected MCP client may simply mean that editor instance is not running or
that the client targets a different port; check the actual configuration first.

- Call `attach_status` before inspecting memory. It follows the editor's attachment;
  specify `pid` when intentionally switching games.
- Discover the current tool schemas rather than copying stale call signatures.
  Tools include `get_known_schema`, `search_entities`, `dump_struct`, `probe_range`,
  `probe_pointer_chain`, `compare_entities` and `read_entity_fields`.
- These tools are read-only. They do not replace GUI write/undo verification.
- Tool availability depends on the current harness/client. If a worker lacks MCP
  access, keep live inspection in the session that has it; do not assume a universal
  SSE limitation or change authentication settings to work around missing tools.

Fallback diagnostics (with `--pid <n>` when several games are running):

```bash
bash Tools/dump-manager.sh
bash Tools/scan-arrays.sh
```

`.mcp.json` is a tracked client example. Worktree creation patches its port locally;
those generated port changes are not feature changes and should not be committed.
`.rusty/mcp.toml` and `.claude/settings.local.json` are local client configuration,
not shared project policy. Leave permissions and credentials alone unless the task
specifically calls for changing them.

## Worktree helpers

```bash
bash Tools/create-worktree.sh <name>
# Implement and verify in .claude/worktrees/<name>/
bash Tools/rebase-worktree.sh <name>
# Review the commits; merge only when the user has accepted them:
bash Tools/ff-merge-worktree.sh <name>
```

These are optional convenience scripts, not a model-specific ceremony. Read their
preconditions before use: the rebase helper expects committed changes and a
`.launch-verified` file, and builds before/after rebasing. It enforces that launch
gate even for docs-only work, although documentation does not need a game launch;
do not fabricate the sentinel. A plain Git branch/worktree is sufficient for docs.

The merge helper fast-forwards main, rebuilds there, then removes the feature
worktree and branch. Do not use it while the user still needs the preview or while
unrelated work is present. Resolve mechanical conflicts without discarding either
side; ask when a conflict needs a product/data decision. Do not force-clean or
remove worktrees as incidental housekeeping.

Unregistered directories are reported and left untouched by the creation helper.
If safe removal fails after a merge, the merge helper retains the worktree and
branch and returns exit code 6 (merge/build succeeded, cleanup incomplete). Inspect
the remaining files before retrying removal; it never falls back to force deletion.

## Recurring regression risks

- Confirm request/entity identity before replacing a panel buffer, and again before
  offering it for editing. A failed load must not reveal a previous entity as the
  new one. Reconcile confirmed/failed bytes in retained snapshots too.
- Related clubs can have different full/short names (for example Spurs/Tottenham).
  Prefer the schema-resolved address matched to the cache; never fuzzy-match an
  editable destination.
- Keep fixed label/value columns where alignment matters. Selection and numeric
  thresholds must be understandable without colour; test scrolling/footer access
  at small sizes. Conventional sidebars/tabs are fine—do not reproduce another
  product's entire composition or replace useful navigation just to look different.
- Render-only tests should prove that missing, invalid or out-of-range data cannot
  emit edits. Preserve packed date flags when changing a date.

## Tasks and historical material

Use the current harness's task list for the requested work. Keep lasting feature
ideas in [ROADMAP.md](../ROADMAP.md), and create issues only when requested. Old
completion markers are historical, not evidence about today's code. Do not turn a
backlog into an instruction to implement or verify everything in one session.

Superseded implementation plans and session verification logs are recoverable from
Git history rather than kept as current instructions. Promote a repeated,
project-specific failure into an invariant or regression test when useful; there is
no requirement to append a new instruction after every correction.
