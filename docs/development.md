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
editor in the same session with `DISPLAY` from `env` but **without**
`WAYLAND_DISPLAY` (for example `env -u WAYLAND_DISPLAY DISPLAY=:2 ...`): with
both set, egui opens as a native Wayland client that `xdotool` cannot see or
click. Start long-lived editors as managed services rather than with `&`,
because a harness may hang up background children when a command returns.
Screenshots supplement the in-game comparison described below; they do not
replace checking against FM's own values. Verified with FM27 and FM24 (Steam,
Proton): render, `grim` capture, `xdotool` clicks and typing. FM26 uses the
same launch option but has not been run headless yet.

FM24 only shows a club's finances to its human manager. To get a second
club witness for a finance field, add a manager to that club in the save
(FM menu > Add New Manager, pick the club with its radio button, Confirm).

**Nobody watches the headless screen.** The user cannot see the headless
display, so they cannot confirm what happened in FM or the editor there. Take a
`shot` after each step and report what FM showed in words: the screen, the
value and the game date. Do not ask the user to check it on their own monitor.
Only one session should drive the shared display at a time.
Keep the editor off FM's display when you can: run it on a second compositor
(`FMH_DIR=/tmp/fmh-ed bash Tools/fm-headless.sh start 900x1000`) and click it
with a `zwlr_virtual_pointer_v1` client on that compositor's Wayland socket.
On 2026-11-01, an editor sharing FM27's display took keyboard and click focus;
after it closed, FM (Wine) still rendered hover but ignored `xdotool` clicks
and typing, and only restarting FM recovered it reliably. Another editor
launched on it can take focus and swallow clicks. Before typing, run
`xdotool windowfocus` on the target window, because `windowactivate` alone
does not move keyboard focus to the editor.

**Every local save is a test save.** All FM saves on this machine (the FM27
Spurs save, the FM24 "Andre Webster - Tottenham" save and any other) exist for
testing. Controlled test writes are acceptable, including writing a candidate
offset to see which on-screen value changes, and a test value that cannot be
restored is recorded as a finding rather than treated as damage. Restore what
you can and document the rest. This does not relax the write rules for real
use: editor writes still go through the acknowledged-write/undo path.


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

## Release identification diagnostics

The release comes from PE `ProductName`, not the executable's filename or its
parent directory. FM24 embeds the FM identity in `fm.exe`; FM26/FM27 embed it
in `game_plugin.dll`, while their exe can report only the Unity engine version.
`ProductVersion` and `FileVersion` are collected for diagnostics, not used as
proof of layout compatibility. Missing/non-FM metadata permits a recognised
directory-name fallback; an explicit unsupported/conflicting FM identity fails.

Every discovered `fm.exe` candidate gets a process-local detection report with:
PID; discovered module paths, bases and sizes; all available `ProductName`,
`ProductVersion` and `FileVersion` strings; resource/module-discovery failures;
the selected module and metadata/directory identification source; storefront
path/manifest hints; and the schema-selection result. Optional plugin discovery
is labelled explicitly; it never substitutes a DLL path from a different PID.
If no candidate can be identified, the error retains all candidates' reports,
not only the first failure. Skipped candidates are logged even if another game
is supported. These reports do not verify offsets or bypass the fingerprint gate.

Reports go to stderr and a persistent JSON-lines log, including in release builds:

- Windows: `%LOCALAPPDATA%\fm-editor\logs\game-detection.log`
- Linux: `${XDG_DATA_HOME:-$HOME/.local/share}/fm-editor/logs/game-detection.log`

Include that file and, if present, `game-detection.log.1` in an identification
bug report. Each entry includes the editor version, OS/architecture and Unix
timestamp. Review paths before sharing: they may include your account name.
Identical repeated reports for the same PID are suppressed (up to 64 remembered
PIDs); changed reports are recorded again. Each file is capped at 512 KiB with
one backup, and individual reports are truncated at 32 KiB with a marker.
An auxiliary `game-detection.lock` file coordinates multiple editor processes;
do not remove it while editors are running. Logging never waits for that lock.
Log-write failures or contention produce a stderr warning and are retried on the
next detection attempt, without preventing attachment.

To inspect an installed module without starting FM, attaching or reading game
memory, run from the worktree (the path can be anywhere; no install name is needed):

```bash
FM_VERSION_TEST_MODULE='/path/to/fm.exe' cargo test --lib identify_installed_module_from_metadata -- --ignored --nocapture
# For FM26/FM27, point FM_VERSION_TEST_MODULE at game_plugin.dll instead.
```

This check requires FM product metadata and tests identification again using a
neutral path. It does not exercise the fallback or verify offsets. On 2026-11-09
the local Steam FM24 EXE reported `Football Manager 2024`, `24.4.2+2081827`;
the local Steam FM27 plugin reported `Football Manager 27`, `27.0.0+2493426`.
FM27's EXE reported Unity `6000.3.18f1-custom (96cdb67fb884)`, not an FM release.
Epic file metadata and layouts remain untested. Storefront hints still depend
on paths/manifests; renamed/moved Epic installs are not reliably distinguished
from Steam. Do not infer storefront or field compatibility from ProductName.

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

### Scripted research (`Tools/fm-research.py`)

A command-line client for the same read-only fm-debug tools, plus helpers that
read FM's own screens on the headless display. It uses `.mcp-port` from the
current directory (otherwise 7799; override with `--port`) and the attached
build's schema files. It never writes game memory.

```bash
R="python3 Tools/fm-research.py"
$R leads player --from fm27                 # unmapped FM27 fields with shifted leads
$R check player contract+0x3c:date "James Maddison=2023-06-28" "Heung-Min Son=2021-07-23"
$R scan player date "James Maddison=2023-06-28" --in contract --size 0x70
$R survey player contract+0x4f:u8           # value distribution over the array
$R where player contract+0x4f:u8 'v & 2'    # entities matching a condition
$R check player "0x2e0>0x0:fmstr" "uid:19325045"   # follow a pointer, decode a string
$R ui-search "Harry Kane" --open 0          # FM search box, click the result row
$R ui-menu History Milestones --then Landmarks --crop 150,180,1300,800   # navigate + OCR
```

Locations are an entity offset (`0x1d4`), a schema sub-record (`contract+0x3c`) or
a pointer chain (`0x2e0>0x0`). Encodings: `u8 u16 i16 u32 i32 u64 ptr date
fmstr bits8`. Witness specs are a name, `uid:N` or `idx:N` (from `where`),
optionally `=expected`; `=-` expects a missing or zero value. `check` exits
non-zero unless every expected value matches.

The UI commands drive FM24's layout on the 1920x1080 headless display using
`xdotool`, `grim`, ImageMagick and `tesseract`. OCR narrows the search and
reads tables, but read values off the screenshot before recording them as
evidence (it misreads digits, e.g. `£BBK`). FM confirms Transfer Status
changes only on **Confirm**; cancel any FM panel still showing a test value
before leaving it. A typical loop: `leads` for candidates, FM screens for
witness values, `check`/`scan` across at least two witnesses, `survey` for
plausibility over the whole array, then the editor's write/Undo check.

Fallback diagnostics (with `--pid <n>` when several games are running):

```bash
bash Tools/dump-manager.sh
bash Tools/scan-arrays.sh
bash Tools/scan-arrays.sh --date 184/2023   # static DateDayYear words for a day/year
```

`--date DAY/YEAR` (repeatable) lists the module's static words that decode to
that day of the year, ignoring FM's flag bits, instead of scanning for arrays.
To locate a game clock, scan for today's date, let FM advance (calendar >
**Go On Holiday**, return after 1 day, avoids the inbox), scan for the next day
and keep the RVA that moved. Repeat once more before mapping it.

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
