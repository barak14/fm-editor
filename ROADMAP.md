# fm-editor Roadmap

High-level feature backlog. Work through these in any order.

---

## UI / UX

- ✓ **UI scale slider** — Replace the 3 discrete scale settings with a slider on the main UI. Controls window content scaling, with an optional mode to scale the whole screen.
- **Navigation shortcuts on human manager screen** — Extra buttons to jump directly to the club or nation the manager belongs to.
- **Integrated advanced search** — Embed advanced search results below the sidebar items (human / club / staff) rather than as a separate screen.
- **Lock screens during DB load** — Prevent any screen from being selected or edited while the database is loading.
- ✓ **Reload on tab switch** — Option to refresh screen data (player/staff/club info) when switching between tabs.
- **Enhanced settings screen** — Rework the settings UI for clarity and extensibility.
- **Don't persist edits until save** — Buffer all edits in memory; only write to the save file when the user explicitly presses Save.
- **Global player search box** — A persistent search box in the top-right that filters players across all views without needing to switch to a search tab.

## List Views / Columns

- **Reload future transfers on open** — Option to re-fetch future transfer data each time that screen is opened.
- **Multi-select players and staff** — Allow multi-selection in list views for bulk actions.
- **Right-click context menu** — Context menu on players, staff, and clubs in the main list view to run actions (e.g. edit, add to shortlist, navigate to club/nation) without opening the detail panel.
- **CA/PA rating columns for players** — Add Current Ability and Potential Ability as visible columns in the player list.
- **Resizeable columns with auto-size** — All list view columns should be resizeable, with a double-click or button to auto-fit column width to content.
- ✓ **Fixed column widths** — Prevent long names or values from pushing columns out of alignment; enforce a max width with truncation/tooltip.
- **Customisable columns per view** — Let the user choose which columns are shown and in what order for each list view (players, staff, clubs).

## Localisation

- **UI language** — Allow the editor interface to be displayed in different languages.
- **Currency format** — Let the user choose their preferred currency and display format for wages, transfer values, and finances (e.g. £, €, $, with regional formatting).
- ✓ **Wage period display** — Show wages as weekly, monthly, or yearly — user preference. FM stores weekly internally; the editor converts for display.

## Data Entry / FM Binary

- ✓ **FM binary version dialog** — If the FM path is non-standard, prompt the user for the binary version. Alternatively, decode the version automatically from the DLL if not already done.

## Player Data

- **Nationality / international eligibility** — Edit primary and secondary nationality and which national team a player can represent.
- **Injury management** — View active injuries, heal them instantly, set injury type and estimated recovery time.
- **Player morale** — Set morale for individual players or an entire squad.
- **Market value / transfer value** — View and edit what the game thinks a player is worth.
- **Squad numbers** — Assign or change shirt numbers.
- **Contract details** — Expiry date, release clause, contract clauses (wage is mapped; the rest isn't yet).
- **International record** — View caps and goals for national teams (read-only to start).
- **Ban / suspension removal** — Clear active bans and suspensions.

## Club Data

- **Finances** — View and edit bank balance, transfer budget, and wage budget.
- **Facilities** — Training ground rating, youth facilities, stadium capacity.
- **Board expectations** — Set what the board wants from the current season.
- **Squad registration** — Manage FFP squad slots (affects European competition eligibility).
- **Affiliations** — Set up and edit feeder club relationships.

## Entities

- **Stadium support** — View and edit stadium records.
- **Team kit editing** — Change kit colours, types, and styles for clubs.

## Search

- **Advanced staff search** — Expanded search for staff with filterable attributes.
- **Advanced club search** — Expanded search for clubs with filterable attributes.

## Shortlists & Transfers

- **Shortlist export** — Build shortlists of players and export them for import into FM24/FM26.
- **Shortlist loading** — Load existing FM shortlist files into the editor.
- **View future transfers** — Button/screen to browse scheduled future transfers.
- **View player retirements** — Button/screen to browse players due to retire.

## Competition & World State

- **Match result editing** — Change the score of a completed match. For researchers and the morally flexible.
- **League table manipulation** — Add/remove points and adjust goal difference for any club.
- **Media predictions** — Set or change pre-season expected league finish.
- **Advance season date** — Skip to a specific date in the game calendar.

## Power User / Bulk Actions

- **Bulk edit** — Apply an attribute change across a whole squad at once (e.g. boost morale for everyone).
- **Comparison view** — Open two players side by side for direct attribute comparison.
- **Player history** — View career stats, clubs, and loan history (read-only).
- **Hidden attributes in search columns** — Expose consistency, important matches, etc. as filterable/sortable columns in the player list.
- **Paste player template** — Copy an attribute profile from one player and apply it to another.

## Image / Face Support

- ✓ **Player face image in editor** — Show the player's face pack PNG in the player panel when a player is selected. UID is already mapped (`p_uid` at `+0x0294`). Needs: `egui_extras` image loader, a configurable face pack directory in settings, and a silhouette fallback when no image exists. Low effort.
- **Full FM NewGAN support** — Support NewGAN face packs for both FM24 and FM26.

## Export

- **Export to JSON or SQLite** — Dump the loaded database (or a selection) to JSON or a SQLite file.

---

*Last updated: 2026-04-14*
*6 items completed (66c64e9, b734d78)* 
