# Changelog

## Unreleased

### Usability

- Standardize player, staff, human-manager and club identity headers, section presentation and attribute controls; adapt attribute layouts to narrower windows.
- Add related club-name links using existing schema chains/cached identities, and a ground-name shortcut to the club Stadium tab.
- Make Back/Forward history span entity types and search pages while retaining search filters.
- Standardize Name/UID-first search toolbars, reset actions, inclusive filter labels, numeric validation, result selection and keyboard navigation; add club sorting.
- Remove non-functional controls, unfinished tabs and empty placeholder result columns; label read-only sections explicitly.
- Document current workflows and remaining relationship support in [the UI guide](docs/ui-guide.md).

### Safety

- Select schemas per build (release + storefront) instead of per release. PC Game Pass installs are recognised by their GDK manifest or `XboxGames` / `WindowsApps` location and get their own schema; FM27 Game Pass starts as an unverified copy of the Steam schema with an uncalibrated hash, so it can attach for bring-up on Windows. The toolbar names the build ("FM27 Game Pass"), and fm-debug `attach_status` reports the storefront and the module path.
- An unreadable game module (e.g. under `WindowsApps`) now fails the version gate with the file error instead of a misleading hash mismatch; the gate reads only the first 64 KB of the module.
- `Tools/scan-arrays.sh` scans every writable, non-executable section from the module's PE section table instead of a fixed FM26 window, so it reaches the FM27 arrays and any build's relocated ones.

- Support several Football Manager versions open at once: the toolbar offers one attach button per running `fm.exe` with its version and PID, and the status bar names the attached game. The editor, the fm-debug MCP server and the diagnostic binaries each read through the schema of the process they attached to; the process-global "active version" is gone, so one attachment can no longer switch another's offsets.
- fm-debug `attach_status` reports every running game, follows the game the editor is attached to, and takes an optional `pid` to switch; `get_known_schema` now reports the attached game's schema. `Tools/dump-manager.sh` and `Tools/scan-arrays.sh` accept `--pid <n>`.

- Disable memory editing and Undo when read-only or awaiting acknowledgement; record undo entries only after successful writes.
- Correlate write responses with request IDs, report detached writes as failures, preserve failed undo for retry, reconcile local buffers and keep write errors visible.
- Clear stale-save undo addresses on reconnect; retain read-only detail snapshots and reject unrelated asynchronous detail responses.
- Reject unmapped/out-of-bounds trait writes, avoid invalid ticket-price clamping on render, and safely handle missing DOB/gender data.

### Editing

- Player contracts are editable: wage, expiry, squad status (FM's selectable statuses only) and squad number, plus asking price; transfer listing stays read-only.
- Player height and appearance (ethnicity, skin tone, hair colour and length, FM27) are editable.
- Staff and human-manager contract wage and expiry, and club reputation, are editable.
- Dates keep FM's flag bits above the day-of-year when edited and display correctly when flags are set.

### Project

- Add a README and license the project under GPL-3.0-or-later.
- Remove one-off research scripts, the snapshot-analysis toolchain and unused diagnostic binaries; keep `scan_arrays` and `dump_manager`.
- Keep local agent and editor configuration out of the repository; `.mcp.json` is now a cross-platform example for the fm-debug MCP server.
- Make the MCP server crates ordinary dependencies (Cargo cannot select dependencies on `debug_assertions`); the server itself remains debug-only.
