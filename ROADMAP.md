# fm-editor Roadmap

High-level feature backlog. Work through these in any order.

Scope expanded on 2026-10-30 using the [FMLE26 advertised feature overview and release history](https://www.fmscout.com/a-fm-live-editor-26.html) as a capability checklist, not a UI specification. New entries describe target coverage, not verified support. Existing checkmarks are historical; this update does not re-verify mappings or certify parity. Partially implemented areas list the remaining scope. Licensing, distribution, performance claims and patch-support promises are not feature requirements.

All new memory fields require version-specific research and two witnesses, paired schema/layout documentation, and live verification. Every write, including bulk actions and recurring freezes, must pass through the acknowledged-write/undo path. FM24/FM26/FM27 coverage must be stated explicitly rather than assumed.

---

## UI / UX

### Workspace direction and remaining work

The current UI groups the sidebar, separates connection controls, places Undo/history in the editing toolbar, uses bordered identity/attribute sections and aligns shared profile fields. Staged Apply, richer change review, new entity types and the broader visual-system work below remain backlog, not implemented features.

- **Grouped left navigation** — Retain the left sidebar as a useful, familiar navigation pattern. Group destinations into Browse (players, staff, clubs and future nations/stadiums), Workspace (future shortlists/comparisons) and Tools; keep connection status in a compact, separate block. Show only implemented destinations. Differentiate through grouping, hierarchy and styling rather than removing conventional navigation.
- **Task-focused profiles** — Give identity and key facts a clear summary, then group editing into focused sections such as Attributes, Career & Contract, and Relationships. Keep tabs and dense attribute grids where useful, but develop our own section hierarchy, spacing and field presentation instead of reproducing another editor's form arrangement.
- **Change-review panel** — Show entity, field, original value, proposed value and write state in one dedicated review surface. Review, Apply and Undo belong here rather than in a left-hand stack of global actions. Use text/state icons as well as colour, not red/green field borders alone.
- **Own visual system** — Develop typography, spacing, neutral surfaces, a restrained accent palette and consistent icons together. Give tables, row selection, filter chips, section headers and editable controls a coherent identity. Differentiate through hierarchy and interaction, not merely a replacement accent colour; include light/dark themes, keyboard access and readable contrast.
- **Prototype before replacing** — Review player search → inspect → edit → apply → undo and club → finances at desktop and narrow widths, retaining grouped left navigation. An adjacent inspector is an optional layout to evaluate, not a requirement; full-width profiles and entity tabs remain valid. Agree the layout before implementation; keep the existing live-write behaviour explicit until staging is built.

### Workflow and preferences

- ✓ **UI scale slider** — Replace the 3 discrete scale settings with a slider on the main UI. Controls window content scaling, with an optional mode to scale the whole screen.
- **Navigation shortcuts on human manager screen** — Extra buttons to jump directly to the club or nation the manager belongs to.
- **Integrated advanced search** — Keep filters and results together within each entity's browsing workspace rather than a separate destination; the exact layout follows the workspace prototype.
- **Lock screens during DB load** — Prevent any screen from being selected or edited while the database is loading.
- ✓ **Reload on tab switch** — Option to refresh screen data (player/staff/club info) when switching between tabs.
- **Enhanced settings screen** — Rework the settings UI for clarity and extensibility.
- **Staged edits / explicit Apply** — Buffer proposed edits locally and write to the attached game's process memory only on Apply, with validation and undo. This is not writing a save file: saving the game to disk remains FM's responsibility. Current editing is immediate; see [the UI guide](docs/ui-guide.md).
- **Global entity search / quick switcher** — Find players, staff, clubs and, once supported, nations and stadiums from any view, with keyboard navigation.
- **Navigation continuity** — Preserve result selection, scroll position and profile section when moving between entities; build on existing Back/Forward and retained filters.
- **Theme refinement** — Carry existing Dark, Light and Dark Blue choices into the new visual system; preserve UI scaling and display preferences.
- **Session change history** — Extend confirmed-write undo with a browsable history, per-field before/after values, pending-edit discard, and clear pending/applied/failed states. Do not imply history survives reconnecting to a different save.

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

- **Nationality / international eligibility** — Edit primary and secondary nationality and which national team a player can represent. *Progress:* primary nationality name is shown read-only on FM24/FM27 (`p_nationality`); FM26 has only the nation pointer. Secondary nationality, eligibility and editing remain.
- **Injury management** — View active injuries, heal them instantly, set injury type and estimated recovery time. *Not started:* no injury, fitness, sharpness or fatigue fields are mapped.
- **Player morale** — Set morale for individual players or an entire squad. *Progress:* FM26/FM27 schemas carry a `p_morale` lead (`+0x26C`) that is not shown in the UI and is unverified (the FM27 layout lists morale as not decoded); FM24 has a lead only in its layout notes.
- **View and remove player unhappiness concerns** — Inspect active concerns (for example, wanting a new contract or more first-team football) and remove a selected concern without having to satisfy the demand. This is separate from boosting morale or changing squad status. Investigate related promises and teammates supporting a complaint separately; do not assume removing the original concern clears the squad reaction. Require per-build verification and undo for every removal. *Not started.*
- **Market value / transfer value** — View and edit what the game thinks a player is worth. *Progress:* FM26/FM27 schemas map `p_value` (`+0x234`) without recorded witnesses and nothing displays it; FM24 is unmapped. FM27 asking price (`+0x238`) is editable on the contract tab.
- **Squad numbers** — Extend existing player contract shirt-number editing to the squad workflow. *Progress:* per-player editing works on FM24/FM27; FM26 squad number is unmapped; no squad-level workflow yet.
- **Contract details** — Extend existing wage, expiry, squad-status and squad-number editing with release/bonus clauses, loyalty bonuses and transfer status; show interested clubs where verified. *Progress:* FM27 shows transfer/loan listing read-only (flag offset still unchecked against a listed player) and edits asking price; clauses, bonuses and interested clubs remain.
- **International record** — View caps and goals for national teams (read-only to start). *Progress:* read-only caps/goals shown on FM24 and FM27 (plus U21 caps/goals on FM27); FM26 is unmapped.
- **Ban / suspension removal** — Clear active bans and suspensions. *Not started.*
- **Complete player profile coverage** — Extend current attributes, CA/PA, positions, traits, personality, reputation and personal details with happiness, career plans, languages and favourite/disliked clubs and people. *Progress:* height, date of birth and appearance (ethnicity, skin tone, hair) are also covered; the listed extensions remain.
- **Registration and work permits** — Player registration, trained-in-club/nation status and work-permit eligibility; distinguish these from nationality rules and club registration limits. *Not started.*
- **Attribute freeze** — Optional player/staff attribute maintenance, with explicit scope, stop controls, undo semantics and safe detach/reconnect behaviour; never an invisible background writer. *Not started.*

## Human Managers & Staff

- **Human-manager actions** — Unsackable status, registration outside normal windows, manager stats and relationships, and moves to another club or a national team. Expose only operations whose effects and undo can be verified.
- **Staff profile coverage** — Extend current attributes, CA/PA, reputation, personal information and contracts with coaching licences, roles, scouting knowledge, tendencies, preferred formations and relationships; handle player/staff hybrid records explicitly.

## Club Data

- **Finances** — View and edit bank balance, transfer budget, and wage budget.
- **Facilities** — Training ground rating, youth facilities, stadium capacity.
- **Board expectations** — Set what the board wants from the current season.
- **Squad registration** — Manage FFP squad slots (affects European competition eligibility).
- **Affiliations** — Set up and edit feeder club relationships.
- **Club finances and income depth** — Extend budget editing with income streams, debts/loans, ticket and season-ticket data, and transfer clauses.
- **Ownership and staffing** — Ownership type/status, chairperson and benefactor settings, takeover controls and extra staff allowances.
- **Club relationships and identity** — Favourite/rival club and person relationships, supporters, attendance and identity details; kit editing is tracked under Entities.
- **Club vision and tactics** — Extend board expectations with club vision and tactical attributes/preferred formations.

## Entities

- **Stadium support** — Search and edit independent stadium records, including stands, capacities, pitch type/dimensions, condition and facilities; extend the existing read-only club ground view.
- **Team kit editing** — Change kit colours, types and styles, with previews, for clubs and national teams.

## Nations & National Teams

- **Nation profiles and rules** — Nationality waiting periods, dual-nationality rules, reputation, FIFA ranking points, FA settings and youth rating.
- **Nation environment and relationships** — Attendance, languages and rival nations; national kits are tracked under Entities.
- **Men's and women's national teams** — Expose and distinguish both sets of data where supported, including national-squad selection for bulk actions. Do not assume the same record layout.

## Search

- **Advanced staff search** — Expanded search for staff with filterable attributes.
- **Advanced club search** — Expanded search for clubs with filterable attributes.
- **Advanced player search coverage** — Extend existing live Name/UID and attribute filters with verified profile/contract criteria and useful sortable result data.
- **Nation and stadium search** — Live filtering and advanced filters for the new entity types; use the same interaction rules as existing player/staff/club searches.

## Shortlists & Transfers

- **Shortlist export** — Build shortlists of players and export them for import into FM24/FM26.
- **Shortlist loading** — Load existing FM shortlist files into the editor.
- **View future transfers** — Button/screen to browse scheduled future transfers.
- **View player retirements** — Button/screen to browse players due to retire.
- **Contract and transfer operations** — Move players, staff and human managers to clubs; arrange player loans, sign free agents, cancel loans/future transfers and release on a free. Verify linked records and reversible effects, not just a club-pointer change; manager national-team moves are tracked above.
- **Retirement controls** — Inspect and edit retirement plans/status, including international retirement where verified.

## Competition & World State

- **Match result editing** — Change the score of a completed match. For researchers and the morally flexible.
- **League table manipulation** — Add/remove points and adjust goal difference for any club.
- **Media predictions** — Set or change pre-season expected league finish.
- **Advance season date** — Skip to a specific date in the game calendar.

## Power User / Bulk Actions

- **Bulk edit** — Select search results or a club/national squad, preview affected entities and apply selected field changes. Report partial failures individually and support undo for confirmed writes.
- **Comparison view** — Open two players side by side for direct attribute comparison.
- **Player history** — View career stats, clubs, and loan history (read-only).
- **Hidden attributes in search columns** — Expose consistency, important matches, etc. as filterable/sortable columns in the player list.
- **Clone player profile** — Copy selected attributes, CA/PA, personality, hidden attributes, positions and traits to one or many players. Preview the copied fields; do not clone identity or contracts implicitly.
- **Reusable player templates** — Save named sets of attributes, positions and traits, then apply them across players and saves; validate against the target version's supported fields.
- **Instant player and squad actions** — Heal injuries, restore fitness, remove bans, boost morale and inspire players; maximize team cohesion and tactical familiarity. Support individual, club-squad and national-squad scope where appropriate, using the same preview/undo safeguards as bulk editing.

## Image / Face Support

- ✓ **Player face image in editor** — Show the player's face pack PNG in the player panel when a player is selected. UID is already mapped (`p_uid` at `+0x0294`). Needs: `egui_extras` image loader, a configurable face pack directory in settings, and a silhouette fallback when no image exists. Low effort.
- **Regen face packs (NewGAN, NG_Regens)** — Assign faces from a regen pack to generated players and staff, for FM24, FM26 and FM27 (Windows and Proton). Read each regen's UID, nationality, ethnicity/skin tone and DOB from memory, pick a face from the matching pack category, and write the UID→face `config.xml` in the pack's own folder (never edit the user's other graphics configs). Needs:
  - **Per-version folder layout** — Packs ship faces in per-region subfolders (e.g. European, South American). FM24 loaded subfolder faces from one pack-root `config.xml` with relative paths (NewGAN: `from="Central European/GER833"`). FM26 was reported to have trouble with this; the working FM26 install (NG_Regens, Dec 2025) wrote every mapping twice — a pack-root config with `Sub/File` paths *and* a `config.xml` in each region folder with bare, category-unique file names (`PP5CentralEurope3231`) — while an earlier root-level config with bare NewGAN names (`IRN34`, no files beside it) appears to be a failed attempt. Which form FM26 actually honours is unconfirmed: test one face per form on FM26 (root path, per-folder bare name) before choosing; check FM27 the same way. Every form maps to `graphics/pictures/person/r-<UID>/portrait`.
  - **Category mapping per pack** — Map FM's ethnicity, skin tone and nationality to each pack's categories as data, not code.
  - **Regen detection** — Research how to tell generated people from real ones (UID range, missing real face, a record flag).
  - **Recycled UIDs** — FM reuses UIDs (aggressively in FM26). Store UID + DOB + nationality per assignment and reassign when they change.
  - **Rescan workflow** — New youth intakes may only appear in memory after the save is reloaded; guide the user through reload → rescan → reload skin.
  - **Field coverage** — FM27 has UID, nationality, ethnicity and skin tone mapped; FM26 lacks ethnicity/skin tone; FM24 lacks nationality and appearance (FM27€0 leads exist).

## Release & Distribution

Release work is complete (2026-10-30); see [the packaging guide](docs/releases.md) and [native packaging](docs/native-packages.md).

- ✓ **Windows/Linux CI** — Native x86-64 workflow runs locked compile/tests and release builds, plus disconnected startup checks. Hosted job results are the evidence for each platform, not a Linux build alone.
- ✓ **Portable test archives** — Windows ZIP and Linux tar.gz packaging includes the editor, tracked documentation, matching application source and SHA-256 checksums. The Linux baseline is Ubuntu 22.04; no system permissions are changed automatically.
- ✓ **Native Windows/Arch packages** — A per-user, unsigned Windows installer and pinned Arch `fm-editor-bin` recipe wrap existing portable release binaries. See [native packaging](docs/native-packages.md).
- ✓ **GitHub release workflow** — Manual test builds as Actions artifacts, then a version-tagged release checked against Cargo's version with archives/checksums attached.
- ✓ **Distribution and source compliance** — Bundled dependency notices and corresponding source for the released version; local settings, MCP configuration, debug binaries and private research kept out of release archives.
- ✓ **Packaged-app smoke tests** — Extract/install and launch the packages on Windows/Linux, then check attachment, one edit and Undo against supported builds. State untested coverage, especially Game Pass, in release notes.
- **Automatic updates and code signing** — Future work.

## Export

- **Export to JSON or SQLite** — Dump the loaded database (or a selection) to JSON or a SQLite file.

---

*Last updated: 2026-10-30*
*Historical completion markers retained; expanded scope is not a new completion audit. Player Data progress notes reflect a code review on 2026-10-30, not live verification.*
