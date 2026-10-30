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

- Disable memory editing and Undo when read-only or awaiting acknowledgement; record undo entries only after successful writes.
- Correlate write responses with request IDs, report detached writes as failures, preserve failed undo for retry, reconcile local buffers and keep write errors visible.
- Clear stale-save undo addresses on reconnect; retain read-only detail snapshots and reject unrelated asynchronous detail responses.
- Reject unmapped/out-of-bounds trait writes, avoid invalid ticket-price clamping on render, and safely handle missing DOB/gender data.

### Project

- Add a README and license the project under GPL-3.0-or-later.
- Remove one-off research scripts, the snapshot-analysis toolchain and unused diagnostic binaries; keep `scan_arrays` and `dump_manager`.
- Keep local agent and editor configuration out of the repository; `.mcp.json` is now a cross-platform example for the fm-debug MCP server.
- Make the MCP server crates ordinary dependencies (Cargo cannot select dependencies on `debug_assertions`); the server itself remains debug-only.
