# Browsing and editing

## Navigation

Player, Staff, Human Manager and Club detail views share a persistent name/identity header. Player and staff headers show age and the club/nation information available in the attached game's schema. Unknown information is labelled explicitly rather than guessed.

The left sidebar groups **Players**, **Staff** and **Clubs** under Browse, with **Human Manager** under Management. Connection controls have their own Game Connection block. Back/Forward/Reload remain at the top; **Undo** and **Change history** are in the editing toolbar above the main workspace, not among navigation destinations. Profile identity summaries and attribute groups use bordered sections.

When a game's version is known, the window title and sidebar heading identify it (for example, **FM27 Editor**); worktree titles retain their branch suffix. The connection block shows Connected/Waiting for save rather than repeating version and PID. The status bar retains attached-game details.

- Click a blue club name to open its club details. Where a club address is resolved through an existing schema chain, it must match an entry in the loaded club cache. Without an address, only a unique cached name is navigable. Full and short club names can differ; address-based links do not depend on their spelling.
- Click a club's ground name to open its **Stadium (read-only)** tab. This is the existing ground information associated with that club, not a separate searchable ground entity.
- **Back** and **Forward** follow one browser-style history across entity types and search pages. Search filters are retained when following related links.
- **Results** returns to the current search's results. Open entities remain in the tab strip until closed.
- Previously loaded detail snapshots remain available for read-only browsing if the game exits. Player/staff records not previously opened can still show their cached basic attributes; club details require a loaded snapshot.

Nation names are informational, not links: there is no mapped nation detail/list model yet. Club squads, club managers, training-ground pages and relations also require additional verified data support. Unsupported controls/tabs are not presented as working features.

## Search

Every search starts with **Name/UID**, followed by its supported filters and **Reset all**. Filtering is live; no Search button is required.

- A numeric query matches the exact UID.
- Name words must all match, ignoring case.
- Numeric bounds are inclusive. Invalid numbers or reversed bounds show an explanation and match no records; they are not silently ignored.
- Click supported column headings to sort. Unknown ages remain last in either direction.
- When a text field or advanced search does not have keyboard focus, use Up/Down to select a result and Enter to open it. The selected/last-opened row is highlighted.
- Player **Advanced → Club ID** matches the exact current club UID (for example, Huelva is 1751 and Huelva B is 814499). It does not expand to related clubs or outgoing loans. Blank means no club restriction; invalid input matches no records. Membership is refreshed with **Reload Save**. Current club filtering uses existing FM24/FM27 Steam mappings; FM26 and Game Pass are unavailable.
- **My clubs** appears before Advanced and includes the managed club, its reserve/B and youth squads, incoming loans and owned players loaned elsewhere. It overrides the manual Club ID while preserving its text. Available on verified FM27 Steam layouts; unavailable or incomplete managed-club data disables activation. Reload Save refreshes membership. Clear All preserves the checkbox; Reset all clears both filters.
- Player **Advanced** exposes only implemented filters. **Clear All** clears the fields shown there; Cancel restores the state from before the dialog opened.
- Position filters use explicit checkboxes and an editable **minimum familiarity (1–20)** per position. A player must meet at least one selected position's minimum (OR); no selected positions means no position restriction. Defaults are 2, preserving the previous “greater than 1” filter. Unmapped/missing or invalid familiarity values never match a selected position. No colour cycling is required.
- Advanced Search is resizable and its inputs scroll independently of Clear All/Cancel/OK. Sweeper is included with the other positions; scroll to reach lower options in short windows.

Column widths are currently fixed; resizable/customizable tables are separate follow-up work.

## Best XI

The **Best XI** sidebar button opens a read-only lineup planner. It starts with
**4-1-2-2-1** (GK, back four, DM, two MCs, AML/AMR, ST); also available are
4-2-3-1 (two DMs), 4-4-2, 4-3-1-2 and 3-4-2-1 (wing-backs and wide attackers).
Managed clubs is the default scope; select an exact club by name/UID to narrow it.
Current membership includes youth and incoming loans and excludes away loans.
Unlike the player-search **My clubs** filter, contract ownership is not included.
Unavailable managed-club data requires an explicit club selection.
Current club scope uses the inherited FM24/FM27 Steam mappings. FM26 and Game
Pass are unavailable; this feature adds no new field mappings.

The solver fills as many slots as possible, then maximizes the whole XI's score
without using a player twice. Minimum familiarity defaults to 15 (accomplished),
and can be set from 1 to 20. Missing/invalid data cannot supply a player score.
Current strength uses the existing heuristic position attribute weights, scaled
by familiarity/20. This is not an FM match-engine rating. Central left/right
slots share weights; preferred foot, roles and duties are not modeled.

**Room to grow** changes the objective to total `max(PA - CA, 0)`, with positional
fit breaking ties. This is an ability headroom proxy for attribute growth, not
a predicted final attribute distribution or growth rate. Professionalism,
determination, ambition and birth year are shown as context, not multipliers.
The [FM24 development research](https://fm-arena.com/thread/20900-some-training-and-development-conventional-wisdom-for-fm24/)
links growth to age, professionalism and match exposure, with late-bloomer
exceptions. Those mechanics have not been verified in FM27; this feature does
not map or read match points or late-bloomer flags, or predict monthly growth.

Each slot shows its selection and the next three eligible alternatives. By
default alternatives exclude all starters; uncheck **Alternatives outside XI**
to include players starting elsewhere, explicitly labeled with their slot.
Alternatives are independent lists, not a unique substitute bench. Click any
name to open that player's profile; Back returns to the lineup. The pitch is
replaced by the same slot information in stacked cards on narrow windows.

Results use the current cache: **Reload Save** refreshes membership and attributes.
Injuries, fitness, suspensions and competition registration are not checked.
An incomplete squad leaves positions visibly unfilled. Snapshot results remain
read-only after the game closes; a lost connection, save change or attachment
change clears them.

## Editing and undo

The banner distinguishes **Editing live**, **Applying changes**, and **Read-only**.

- Changes apply immediately to game memory. There is no separate Save action.
- Numeric fields can be clicked to type exact values, or dragged where supported.
- Date rows also offer **Calendar**: browse months, type a year or use year controls, then choose a day. The current date is marked with brackets as well as styling. Browsing or cancelling does not write anything; choosing a day uses the same live editing and undo path as direct entry. Calendar controls are unavailable in read-only mode.
- Editing is disabled while disconnected/read-only or awaiting write acknowledgements. Tabs and relationship navigation remain usable.
- **Undo** (Ctrl+Z when not editing text) applies to confirmed writes only. Its tooltip names the next entity/field change.
- Failed writes produce a persistent, dismissible error, restore the unsuccessful edit's local bytes, and refresh the active view when connected. Failed undo remains available to retry.
- Undo history is cleared when changing/reconnecting saves because previous memory addresses cannot safely be reused.
- Unsupported trait fields, incomplete buffers and invalid stored ticket prices are not editable. Rendering them must not clamp and write game memory without input.

Read-only sections/tabs are labelled. Attributes use four columns on wide views and two or one on narrower views; labels use proportional text and numeric hit targets are consistent.

Shared profile field rows, including reputation, use a fixed-width, left-aligned label column so value boxes line up regardless of label length. Long labels truncate with their full text available on hover. Compact CA/PA pairs, position cells and date controls retain their specialized layouts.

The player **Positions & Traits** formation uses a compact, left-aligned grid with equally spaced left/centre/right columns. Sweeper is in the centre column below DC and above GK, using the same compact label/value cell as the other positions.
