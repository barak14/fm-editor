# Browsing and editing

## Navigation

Player, Staff, Human Manager and Club detail views share a persistent name/identity header. Player and staff headers show age and the club/nation information available in the attached game's schema; club headers show the club's nation where mapped (FM27). On FM27 Steam, age is calculated on the attached game's current date and updates as days pass; other builds estimate it from the birth year and the build's season year. Unknown information is labelled explicitly rather than guessed.

Player and Human Manager overviews use compact identity strips above independently stacked columns. Personal details sit beside attributes on desktop rather than above them; narrow windows reflow the columns and scroll vertically. Read-only profile values stay beside their labels and within their column, with the full value on hover when shortened. The manager's editable Identity controls are in the personal-details column, not in the header.

In Human Manager, read-only Roles follow Knowledge. Player Status follows Identity in the left personal-details column; morale is shown and edited as a numeric 1–20 rating rather than descriptive level names.

The left sidebar groups **Players**, **Staff** and **Clubs** under Browse, with **Human Manager** under Management. Connection controls have their own Game Connection block. Back/Forward/Reload remain at the top; **Undo** and **Change history** are in the editing toolbar above the main workspace, not among navigation destinations. Compact identity strips keep profile context visible without a tall banner.

When a game's version is known, the window title and sidebar heading identify it (for example, **FM27 Editor**); worktree titles retain their branch suffix. The connection block shows Connected/Waiting for save rather than repeating version and PID. The status bar retains attached-game details.

- Click a blue club name to open its club details. Where a club address is resolved through an existing schema chain, it must match an entry in the loaded club cache. Without an address, only a unique cached name is navigable. Full and short club names can differ; address-based links do not depend on their spelling.
- Click a club's ground name to open its **Stadium (read-only)** tab. This is the existing ground information associated with that club, not a separate searchable ground entity.
- **Back** and **Forward** follow one browser-style history across entity types and search pages. Search filters are retained when following related links.
- **Results** returns to the current search's results. Open entities remain in the tab strip until closed.
- Previously loaded detail snapshots remain available for read-only browsing if the game exits. Player/staff records not previously opened can still show their cached basic attributes; club details require a loaded snapshot.

Nation names are informational, not links: there is no mapped nation detail/list model yet. Club squads, club managers, training-ground pages and relations beyond the FM27 player ties described below require additional verified data support. Unsupported controls/tabs are not presented as working features.

## Search

Every search starts with **Name/UID**, followed by its supported filters and **Reset all**. Filtering is live; no Search button is required.

- A numeric query matches the exact UID.
- Name words must all match, ignoring case. Club search matches all words in either the short name or the full name ("Tottenham" finds Spurs).
- Numeric bounds are inclusive. Invalid numbers or reversed bounds show an explanation and match no records; they are not silently ignored.
- Click supported column headings to sort. Unknown ages remain last in either direction.
- When a text field or advanced search does not have keyboard focus, use Up/Down to select a result and Enter to open it. The selected/last-opened row is highlighted.
- Player **Advanced → Club ID** matches the exact current club UID (for example, Huelva is 1751 and Huelva B is 814499). It does not expand to related clubs or outgoing loans. Blank means no club restriction; invalid input matches no records. Membership is refreshed with **Reload Save**. Current club filtering uses existing FM24/FM27 Steam mappings; FM26 and Game Pass are unavailable.
- **My clubs** appears before Advanced and includes the managed club, its reserve/B and youth squads, incoming loans and owned players loaned elsewhere. It overrides the manual Club ID while preserving its text. Available on verified FM27 Steam layouts; unavailable or incomplete managed-club data disables activation. Reload Save refreshes membership. Clear All preserves the checkbox; Reset all clears both filters.
- Player **Advanced** exposes only implemented filters. **Clear All** clears the fields shown there; Cancel restores the state from before the dialog opened.
- Position filters use explicit checkboxes and an editable **minimum familiarity (1–20)** per position. A player must meet at least one selected position's minimum (OR); no selected positions means no position restriction. Defaults are 2, preserving the previous “greater than 1” filter. Unmapped/missing or invalid familiarity values never match a selected position. No colour cycling is required.
- Advanced Search is resizable and its inputs scroll independently of Clear All/Cancel/OK. Sweeper is included with the other positions; scroll to reach lower options in short windows.

Column widths are currently fixed; resizable/customizable tables are separate follow-up work.

## Player comparison

Open a player profile and click **Select for compare**. Open another player's
profile, then click **Compare with selected** to open the comparison immediately.
The selected player's name stays beside the profile actions. Selecting a new
player starts a new pair; **Clear selection** removes the selection and comparison.
**Compare players** in the workspace toolbar reopens the comparison.

The two columns use player names, distinct identity accents and coloured
attribute values, with grouped technical, set-piece, mental, physical and
goalkeeping sections. Player names stay visible while scrolling. There is no
separate difference column. **Swap players** exchanges the columns. Available
age, CA/PA, wage in GBP/week and contract expiry provide context. Missing,
invalid or unmapped values are shown as unavailable, not zero. Values use each
player's own build schema; Game Pass remains unverified. Comparisons require
different players from the same build and add no new mappings.
Wage uses the explicitly typed current-contract money field where available;
legacy direct fields without an explicit monetary width are not used as fallbacks.

Comparisons are read-only snapshots, not automatically refreshed live values;
hover the snapshot information control for details. Reload profiles and select
them again after changes. Selection is disabled while a write is awaiting
acknowledgement. Comparisons clear on **Reload Save**, save replacement/unload
and reconnection; existing snapshots remain viewable after the game exits.
The window adapts and scrolls at narrow sizes. List multi-selection/right-click
comparison is not implemented.

This is attribute comparison only. Season/competition statistics, per-90 values,
league rankings and performance percentiles still require separate verified data.

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
These detailed research notes remain in this guide; the planner exposes concise
scope and estimation caveats on hover rather than repeating instructions inline.

Each slot shows its selection and the next three eligible alternatives. By
default alternatives exclude all starters; uncheck **Alternatives outside XI**
to include players starting elsewhere, explicitly labeled with their slot.
Alternatives are independent lists, not a unique substitute bench. Click any
name or shirt to open that player's profile; Back returns to the lineup. The
striped green pitch includes boundary, halfway and centre-circle markings,
penalty and goal areas, goals and penalty spots. Vector shirt icons carry the
position abbreviation; goalkeepers wear yellow and unfilled slots grey. Fit
or growth headroom and player names remain below each shirt (hover a shortened
name to read it in full). Below 620px of content width, the same slot information
is presented in stacked depth cards instead of squeezing the pitch.

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
- **Undo** (Ctrl+Z when not editing text) reverses the latest logical action with confirmed changes. Its tooltip names the entity/action and explains when Undo is unavailable. Only acknowledged writes are eligible; there is no arbitrary-row Undo or force option.
- One date-picker interaction or facility edit with a finance mirror is one action. Separate controls are not grouped just because they change in the same frame. Compound writes are not atomic: each part has its own byte guard and acknowledgement, and partial success is shown honestly.
- Undo reverses the remaining confirmed parts of that action in reverse write order. A refused or unconfirmed Undo stays retryable; parts already restored are not written again. An initially unconfirmed write is not silently made undoable. Undo cannot reverse contracts or other gameplay consequences created by FM.
- A refusal before writing is distinguished from an unconfirmed attempted write. Errors remain visible and the active record is refreshed. An unconfirmed write may already have changed memory; a refreshed display is not a claim that the old value was restored.
- Executable Undo state is invalidated when the game closes, the save is unloaded/replaced, or the editor reconnects. Audit-only reports remain available in the running editor under **Include previous closed sessions**, without write or profile-navigation authority. Nothing is persisted across editor restarts.
- The editor recognises save identity by club-object addresses. FM adding/removing records, or **Reload Save** refreshing the same save's cache, does not by itself invalidate Undo. A detected save replacement does. Before each edit or Undo, changed/unreadable bytes refuse the write; this is a read-before-write guard, not an atomic FM transaction.
- The worker also refuses queued ordinary writes while its save context is unloaded/loading, before reading or writing bytes. An attached process alone is not permission to write.
- Unsupported trait fields, incomplete buffers and invalid stored ticket prices are not editable. Rendering them must not clamp and write game memory without input.

FM may cache its own displayed profiles: navigating away and back need not
refresh an externally edited value or an Undo. Do not infer memory state from a
cached FM screen. See the [FM27 history/Undo verification and cache caveat](fm27/memory-layout.md#edit-history-and-grouped-undo-verification-2026-11-01-steam).

### Change history

**Change history** opens a resizable audit window, separate from the Undo stack.
Rows show submission date/time (**UTC**), entity type/name/UID, field or action,
human-readable before/after values and a labelled status. Statuses include
**Applying**, **Applied**, **Partially applied**, **Refused**, **Unconfirmed**,
**Undoing**, **Undone** and incomplete/refused/unconfirmed Undo outcomes.
Undone actions and failed attempts stay visible rather than disappearing.

Search by entity, UID, field, value or message; filter by status or the currently
opened entity, and optionally include closed sessions. Expand **Details** for
physical changes, individual Apply/Undo attempts, acknowledgement warnings,
failure reasons and technical byte/address information. **Open profile** is
available only for the current session and is revalidated against exact cached
build/type/address/index/UID identity; names alone never select a destination.

**Undo latest** uses the same chronological command as the toolbar, not the row
being viewed. Up to 100 logical actions retain Undo authority; the in-memory
audit retains up to 500 settled reports, protecting pending operations from
eviction. Old sessions contain reports, never reusable Undo payloads. This first
version has no Redo, export, replay, history-delete action or staged Apply.
Narrow windows use stacked rows instead of squeezing a wide table.

Read-only sections/tabs are labelled. Player Overview places personal details beside
independently stacked attribute cards in four or five columns on desktop views,
with three, two or one on smaller viewports. Bounded content widths keep related values close on
large displays; short groups do not wait for neighbouring cards to finish.
Redundant inline instructions have been removed; necessary caveats remain in
labels and tooltips. Unsupported contract-type conversion has no Apply button;
its read-only status and hover explanation remain visible on the Contract tab.

In **Overview**, **Show potential attributes** above the attribute cards replaces
their values with read-only estimates at PA in the same categories. This uses the
existing extrapolation heuristic, not a guaranteed development outcome. Other
Overview fields remain editable. Turning it off restores actual attribute values
and normal editing; previewing never changes memory or adds Undo entries. The
choice is kept separately for each player/build. Invalid attributes remain
unavailable or visibly invalid, and attributes without projected growth stay as-is.

Shared profile field rows, including reputation, use a fixed-width, left-aligned label column so value boxes line up regardless of label length. Long labels truncate with their full text available on hover. Compact CA/PA pairs, position cells and date controls retain their specialized layouts.

The player **Positions & Traits** formation uses the same marked pitch and shirt
icons, with all 15 positions arranged from ST at the top to GK at the bottom.
Sweeper remains between DC and GK in the centre. On wide panels the pitch is
sized against the available profile height, with traits beside it in an
independently scrolling pane. Narrow panels stack
the pitch and traits; genuinely short views scroll rather than shrinking numeric
controls below a readable size. Position abbreviations appear on the shirts,
with familiarity values directly beneath them instead of repeated labels.
The same schema-backed editing and Undo behaviour is retained. Invalid raw
values stay visible and are not corrected by rendering. The yellow keeper shirt
is distinct from the blue outfield shirts; familiarity is always numeric.

## FM27 player data

These player rows and actions are mapped for **FM27 Steam only**. FM27 Game Pass inherits the Steam schema but remains unverified; FM24/FM26 do not show these new controls.

- **Status**, below Identity, shows editable numeric morale (1–20) and transfer value in pounds. Morale uses the attached build's mapped field; descriptive morale words are not used in the control.
- **Injuries** lists the injury name, injured-on date, and dates for returning to light and full training. **Heal** shortens the remaining durations to the elapsed time since the injury began; it never lengthens an injury. Both training durations are one undoable edit.
- **Unavailability** shows suspensions/bans, holidays and unknown codes with available dates. **Lift ban** shortens a date-based suspension to the elapsed time since it began. Holidays and unknown kinds are read-only. Heal/Lift ban require a readable list and a known current game date, and are disabled when no shortening is possible. On FM27 Steam, once the shortened ban expires during the next game day, FM deletes the suspension, and Undo can no longer restore it.
- **Identity** shows the player's languages in FM's order and level words (Basic, Good, Fluent), read-only.
- **Names** (players and staff) edits the first, last and common name. FM stores names as shared records (one "Frank" serves every Frank), so the editor never changes a name's text: **Show and change names** reads the save's existing name records once (a few seconds), and the search button beside each name lists matching existing names with how many people use them. Choosing one points the person at that record; a name nobody in the save uses cannot be created. **Clear** removes a common name. Each change is one undoable edit, and FM's search finds the new name at once; profiles FM has already shown keep the old name until a game day passes. A separate full-name string some people have (FM's "Full Name" row, e.g. "Kylian Mbappé Lottin") is not changed.
- **Nationality & ties** shows other nationalities and favoured clubs with the known reason (**As a player** / **As a supporter**, or **Reason N**). A favoured club is navigable only when its object address matches a club in the loaded cache; names alone do not create these links.
- The **Contract** tab provides **Transfer listed** and **Loan listed** checkboxes. Transfer listing records the current game date and club-listing code together with the flag; removing it clears those listing values. Loan listing changes only its own flag. Other contract bits/bytes are preserved, and an active transfer listing shows **Listed since**. Changing transfer listing requires the current game date to be known.
- **Bonuses & clauses** shows appearance fees, goal/clean-sheet/assist bonuses and unused-substitute fees as editable pound amounts. Wage-drop percentages and club-extension years are read-only. Unknown clause types retain their raw amount/value display.

Unloaded lists say **Not loaded** and unreadable lists are explicitly unavailable/read-only, not misrepresented as empty. All editable controls remain visible but disabled while disconnected or awaiting an acknowledgement. List-item values update only after the write is acknowledged; the player is then re-read to refresh list membership, even if another entity is now active. Confirmed changes support **Undo**, just like other player edits.

## FM27 staff data

Staff profiles (FM27 Steam) also show, read-only: role ratings, languages, the coaching licence and the preferred in- and out-of-possession formations. Formations FM was not seen to name show as **Formation #N**. The **Names** section works as for players.

## Experimental FM27 Part Time offer policy trial

The FM27 Club view includes **Contract policy (trial)**. It can prepare a
read-only eligibility/before-and-after preview for the selected club. Editing
is disabled by default. A trial requires an explicit `--enable-policy-trial`
launch, the verified Windows Steam build, a human manager at that club, an
eligible Non-Contract-only policy, and confirmation of a separate test save
with an untouched backup. Unsupported records show a refusal, not an editable
default. No club name or UID is hardcoded into eligibility.

An explicit **Apply club policy trial** adds a Part Time policy input for future
offers. FM still chooses/validates terms through normal negotiation. This does
not convert an existing contract, select wages/dates or remove board limits.
The Player Contract tab continues to show unsupported type conversion; wages
are labelled in GBP/week and bonuses in GBP/event.

The initial live trial exposed an Undo-guard bug, now covered by a regression
and corrected in code. The user has since confirmed Undo after opening/changing
an offer, reapplication and save/reload. Read-only checks confirmed the policy
after reload and a day advance. An accepted fixed-term Part Time contract also
matches the read-only current-contract checks. **Post-contract reload and salary-
payment tests remain outstanding.** Follow the [evidence and test-copy workflow](fm27/contract-type-investigation.md)
before use. Undo concerns the byte edit and cannot reverse accepted contracts
or gameplay consequences after time advances.
