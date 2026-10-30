# Browsing and editing

## Navigation

Player, Staff, Human Manager and Club detail views share a persistent name/identity header. Player and staff headers show age and the club/nation information available in the attached game's schema. Unknown information is labelled explicitly rather than guessed.

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
- Player **Advanced** exposes only implemented filters. **Clear All** clears the fields shown there; Cancel restores the state from before the dialog opened.

Blank placeholder columns and non-functional filters/buttons have been removed. Column widths remain fixed in this pass; resizable/customizable tables are separate follow-up work.

## Editing and undo

The banner distinguishes **Editing live**, **Applying changes**, and **Read-only**.

- Changes apply immediately to game memory. There is no separate Save action.
- Numeric fields can be clicked to type exact values, or dragged where supported.
- Editing is disabled while disconnected/read-only or awaiting write acknowledgements. Tabs and relationship navigation remain usable.
- **Undo** (Ctrl+Z when not editing text) applies to confirmed writes only. Its tooltip names the next entity/field change.
- Failed writes produce a persistent, dismissible error, restore the unsuccessful edit's local bytes, and refresh the active view when connected. Failed undo remains available to retry.
- Undo history is cleared when changing/reconnecting saves because previous memory addresses cannot safely be reused.
- Unsupported trait fields, incomplete buffers and invalid stored ticket prices are not editable. Rendering them must not clamp and write game memory without input.

Read-only sections/tabs are labelled. Attributes use four columns on wide views and two or one on narrower views; labels use proportional text and numeric hit targets are consistent.

## Verification of this pass

Verified on Linux against the running FM27 save:

- `cargo check` and `cargo test`: 39 library tests passed, plus binary/doc-test targets.
- `Tools/build-and-launch.sh` startup/survival gate.
- Private headless Wayland screenshots and real pointer/keyboard input: Human Manager -> Tottenham Hotspur -> Stadium -> Back; Harry Kane -> Bayern -> Back, with the player search retained.
- Four-column desktop and two-column narrow player layouts.
- Human manager CA: 146 -> 147 -> Undo -> 146, confirmed in the displayed editor and with `read_entity_fields` against live game memory; PA stayed 200. The original value was restored.

No memory offsets or encodings were changed. FM24/FM26 layouts, actual process-loss UI transitions and visual refresh inside the game's own window were not live-tested in this pass; do not treat this as verification of inherited field mappings.
