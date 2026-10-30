# FM26 Editor GUI — Implementation Plan

## Context

The fm-editor project has a mature memory-reading backend with 90+ player fields, 49+ staff fields, 14 staff roles, club sub-records, and PPM decoding all mapped. The current main.rs is a throwaway CLI demo. We need a proper GUI editor (similar to FMLE) so exploration findings persist and the tool is useful for live editing.

## Requirements Summary

- FMLE-style dark UI with dense attribute grids and colored value boxes
- Left sidebar: Load Game, Save Changes, Undo, Edit History, Human Manager, Club Search, Player Search, Staff Search, Stadium Search
- Recently-viewed entity tabs across the top for quick switching between players/staff/clubs
- Context-sensitive top section that changes per entity type (different filters/search for players vs staff vs clubs vs stadiums)
- Auto-connect option — polls for fm.exe without button press
- Disconnect detection when fm.exe closes or save reloads
- Undo/history for all writes
- Position ratings (weighted attribute calculation per position)
- PA extrapolation (projected max attributes)
- Kit rendering (later phase, not MVP)

## Entity Tab Structure

**Player tabs:** Overview, Positions & Traits, Contract, Relations
**Staff tabs:** Overview, Contract, Relations  
**Club tabs:** Teams, Relations, Staff, Vision, Income, Debts & Loans, Transfer Clauses, Affiliations, Supporters, Kits
**Stadium:** Single pane, no tabs

## Technology

- **eframe 0.31 / egui** (already in Cargo.toml)
- **std::sync::mpsc** channels for background I/O
- **std::thread** for worker owning FmProcess
- No new dependencies needed

## Architecture

### Background Worker Thread

All process I/O on a dedicated thread. UI communicates via mpsc channels.

```
UI Thread                     Worker Thread
  |-- Attach ----------------->|  FmProcess::attach()
  |<-- Connected(pid) ---------|
  |-- SearchPlayers("Kane") -->|  find_players_by_name()
  |<-- PlayerResults(vec) -----|
  |-- WriteField(addr,key,val)->| read old, write new
  |<-- WriteOk(old,new) -------|
  |-- Ping (every 2s) -------->|  read_u8(gp_base) check
  |<-- Disconnected -----------|  (if read fails)
```

### Connection Lifecycle

- **Manual**: "Load Game" sends Attach command
- **Auto-connect**: polls Attach every 3s while disconnected
- **Disconnect**: Ping every 2s; failed read_u8(gp_base) triggers Disconnected
- **Save reload**: FM26 restarts process, disconnect fires naturally
- No changes to process.rs needed

### Undo System

```rust
struct UndoEntry {
    description: String,    // "Crossing: 14 -> 18"
    addr: u64,              // entity base address + offset
    old_raw: Vec<u8>,       // bytes before edit (from cached raw)
    new_raw: Vec<u8>,       // bytes after edit
    timestamp: Instant,
}
```

Old bytes captured from cached PlayerRecord/StaffRecord raw Vec. Undo writes old_raw back via bridge. Capped at 100 entries. No redo in MVP.

### Recently-Viewed Tabs

A Vec of recently opened entities (max ~8). Each tab stores entity type + identifier + display name. Clicking a tab re-reads the entity from memory (data may have changed). Closing a tab removes it. New searches append/move-to-front.

## File Structure

```
src/
  main.rs              -- eframe::run_native() entry point
  app.rs               -- FmEditorApp struct, eframe::App impl, update() dispatch
  connection.rs         -- ProcessBridge, BridgeCmd/BridgeResult, worker thread
  undo.rs              -- UndoStack, UndoEntry
  ratings.rs           -- Position rating calculator, PA extrapolation
  ui/
    mod.rs             -- re-exports
    sidebar.rs         -- left nav panel (Load Game, Save, Undo, nav buttons)
    toolbar.rs         -- top bar: context-sensitive search/filters, recently-viewed tabs
    search.rs          -- SearchState<T>, search input + results list
    player_panel.rs    -- player tabs: Overview, Positions & Traits, Contract, Relations
    staff_panel.rs     -- staff tabs: Overview, Contract, Relations
    club_panel.rs      -- club tabs: Teams, Relations, Staff, Vision, etc.
    stadium_panel.rs   -- stadium single pane
    field_widget.rs    -- reusable: colored value box (green/yellow/red), inline editing
    status_bar.rs      -- bottom bar: connection, entity counts, save info
  // existing, unchanged:
  fields.rs, offsets.rs, process.rs, player.rs, staff.rs, club.rs
```

## Key Types

### FmEditorApp (app.rs)

```rust
struct FmEditorApp {
    active_page: Page,
    bridge: ProcessBridge,
    auto_connect: bool,

    // Search state
    player_search: SearchState<PlayerRecord>,
    staff_search: SearchState<StaffRecord>,
    club_search: SearchState<ClubRecord>,

    // Recently viewed — tabs across top
    open_tabs: Vec<EntityTab>,
    active_tab: Option<usize>,

    // Loaded entity data (refreshed when tab selected)
    selected_player: Option<PlayerRecord>,
    selected_staff: Option<StaffRecord>,
    selected_club: Option<ClubRecord>,

    undo_stack: UndoStack,
    status_msg: Option<(String, Instant)>,
    entity_counts: Option<EntityCounts>,  // players, staff, clubs
}

enum Page { HumanManager, PlayerSearch, StaffSearch, ClubSearch, StadiumSearch }

struct EntityTab {
    entity_type: EntityTabType,
    display_name: String,
    addr: u64,
    index: usize,
}

enum EntityTabType { Player, Staff, Club, Stadium }
```

### Field Widget (field_widget.rs)

FMLE-style colored value box:
- **Green** (16-20): excellent
- **Yellow-green** (12-15): good  
- **Yellow** (8-11): average
- **Orange** (4-7): poor
- **Red** (1-3): very poor

Each attribute rendered as: `Label  [colored box with value]`
Box is an inline editable DragValue with colored background.

### Player Overview Panel Layout

Matches FMLE structure:
```
+--Personal Details---+--Ability & More------+--Reputation----------+--Actions-------+
| Name, DOB, Nation   | CA [___] Value [___] | Current/Home/World   | Freeze Attrs   |
| City, Club          | PA [___]             | Snr Apps, Goals      | Heal / Inspire |
+---------------------+---------------------+----------------------+----------------+

+--Technical--+--Mental-------+--Physical-----+--Hidden-------+
| Crossing 12 | Aggression 13 | Accel    12   | Consistency 19|
| Dribbling14 | Anticip   18  | Agility  12   | Inj Prone  10 |
| ...         | ...           | ...           | ...           |
+-------------+---------------+---------------+---------------+

+--Set Pieces--+--Footedness--+--Condition----+--Personality--+
| Corners   11 | L: 16 R: 20  | Sharpness --- | Adaptab    10 |
| FreeKicks 12 |              | Fitness   --- | Ambition   18 |
| ...          |              | Fatigue   --- | ...           |
+--------------+--------------+---------------+---------------+
```

### Position Ratings (ratings.rs)

Calculate 1-100 rating per position using weighted attribute sums. E.g. for ST:
- Finishing (3x), Composure (2x), Off The Ball (2x), First Touch (1.5x), Heading (1.5x), Decisions (1x), Anticipation (1x), Pace (1x), Strength (1x), etc.

Normalized against CA to produce percentage. Displayed alongside position familiarity in the Positions & Traits tab.

### PA Extrapolation (ratings.rs)

For each attribute: `projected = min(20, current * PA / CA)`, capped at 20. Displayed as a second column in attribute grids. Edge cases: CA=0, attribute already at max.

## Implementation Phases

### Phase 1: Skeleton + Connection
**Files:** main.rs, app.rs, connection.rs, ui/mod.rs, ui/sidebar.rs, ui/toolbar.rs, ui/status_bar.rs

- eframe app with dark theme, sidebar, toolbar, status bar
- Load Game button, auto-connect toggle
- Connection status (green/red + PID)
- Ping-based disconnect detection
- Entity count display in status bar
- **Verify:** app launches, connects, shows PID, detects fm.exe close

### Phase 2: Player Search + Read-Only Display
**Files:** ui/search.rs, ui/player_panel.rs, ui/field_widget.rs

- Search input in context-sensitive top area
- Results list with name, CA, PA, best position
- Click to open in recently-viewed tabs
- Player Overview tab with all attribute grids (read-only)
- FMLE-style colored value boxes
- Positions & Traits tab with position grid + PPM checkboxes
- **Verify:** search "Kane", see all attributes with colored boxes, switch tabs

### Phase 3: Editing + Undo
**Files:** undo.rs, updates to player_panel.rs, field_widget.rs, app.rs

- DragValue editing on colored boxes
- Writes sent through bridge
- UndoStack populated on each write
- Undo button in sidebar + Ctrl+Z
- Record re-read after write
- **Verify:** change Crossing, see it update in-game, undo it

### Phase 4: Position Ratings + PA Projection
**Files:** ratings.rs

- Weight tables for 15 positions
- Rating displayed in Positions & Traits tab
- PA projection column alongside current values in attribute grids
- **Verify:** Kane shows high ST rating, projected attributes at PA=190

### Phase 5: Staff + Club + Stadium Panels
**Files:** ui/staff_panel.rs, ui/club_panel.rs, ui/stadium_panel.rs

- Staff search + Overview/Contract/Relations tabs
- Staff roles display (from sub-record at +0x1D0)
- Club search + all 10 tabs (many read-only until sub-record writes mapped)
- Stadium single pane (attendance, nickname, capacity)
- **Verify:** find Guardiola, view roles, find Bayern, see facilities

### Phase 6: Human Manager + Polish
- Human Manager page via RVA at gp+0x4DE7A08
- Keyboard shortcuts (Ctrl+Z undo, Enter search, Esc clear)
- Edit History panel
- Status messages for writes/errors

### Phase 7 (Later): Kit Rendering
- Map kit color/pattern offsets in club struct
- Render kit preview in Club Kits tab
- Kit color editing

## Files to Modify/Create

| File | Action |
|------|--------|
| src/main.rs | **Rewrite** — eframe entry point |
| src/app.rs | **New** — FmEditorApp, eframe::App impl |
| src/connection.rs | **New** — ProcessBridge, worker thread |
| src/undo.rs | **New** — UndoStack |
| src/ratings.rs | **New** — position ratings, PA extrapolation |
| src/ui/mod.rs | **New** — module re-exports |
| src/ui/sidebar.rs | **New** — left nav panel |
| src/ui/toolbar.rs | **New** — top bar with context search + entity tabs |
| src/ui/search.rs | **New** — SearchState, search widget |
| src/ui/player_panel.rs | **New** — player detail with 4 tabs |
| src/ui/staff_panel.rs | **New** — staff detail with 3 tabs |
| src/ui/club_panel.rs | **New** — club detail with 10 tabs |
| src/ui/stadium_panel.rs | **New** — stadium single pane |
| src/ui/field_widget.rs | **New** — colored value box widget |
| src/ui/status_bar.rs | **New** — bottom status bar |

**Unchanged:** fields.rs, offsets.rs, process.rs, player.rs, staff.rs, club.rs

## Verification Plan

1. `cargo run` — window opens with dark theme, sidebar visible
2. Click Load Game with fm.exe running — Connected + PID shown
3. Close fm.exe — Disconnected within 2s, auto-connect reconnects
4. Search "Kane" — results list, click opens tab, attributes shown with colors
5. Edit Crossing 12→18 — value persists, check in-game, Undo reverts
6. Open multiple players — tabs across top, click to switch
7. Position ratings show ST highest for Kane
8. PA projection shows scaled attributes
9. Staff/Club/Stadium panels show mapped data
