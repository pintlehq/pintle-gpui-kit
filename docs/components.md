# Components and layout

Use `pintle_gpui_kit::kit::component` for Button, Input, Tabs, Badge, Dialog, and the
rest of Kit's component library. There are no Pintle one-to-one wrappers.

Pintle adds two stateless `RenderOnce` compositions:

- `SectionHeader::new(title).description(text).action(element)` groups a title,
  supporting description, and consumer-owned action.
- `Panel::new().header(element).child(element)` groups content using semantic
  surface, foreground, border, and radius values. `padding(Pixels)` customizes
  its inset. It implements `ParentElement` for multiple children.

```rust,ignore
use pintle_gpui_kit::{components::{Panel, SectionHeader}, kit};
use kit::{prelude::*, component::button::Button};

Panel::new()
    .header(SectionHeader::new("Connections")
        .description("Application-owned connection list")
        .action(Button::new("add").label("Add").on_click(|_, _, _| {})))
    .child("Supply your application view here")
```

Replace the illustrative callback with a real application action. Keep retained
state such as `Entity<InputState>` in the owning view, not inside `render`.
Pass existing callbacks and entities through composition so Kit retains its
focus, keyboard, accessibility, and event behavior.

## Shell

`AppShell::new(content)` accepts optional `.sidebar`, `.header`, `.toolbar`, and
`.status_bar` elements. `.sidebar_width(Pixels)` defaults to 208 px. Header and
status span the window; sidebar and content fill the middle; toolbar sits above
content. Omitted regions consume no space. Negative sidebar widths clamp to zero.

The shell constrains its layout to the parent, permits children to shrink, and
clips overflowing regions. **Content and sidebar must provide their own scroll
containers**. This prevents nested scrolling policy from being imposed on
applications. Supply sensible widths for your own minimum window size.

Create a Pintle composition only when it captures shared structure or behavior.
Use semantic colors, token spacing, caller-owned state, stable element IDs, and
clear accessible labels. Test outcomes through rendered UI interactions. Docked
workspaces use the optional integration below.

## Tree and docked workspace

`NavigationNode::folder(id, label, children)` groups nodes;
`NavigationNode::resource(Document::new(key, title, context))` opens application
resources. `.icon(...)` supplies a Kit icon. `NavigationTree::new` retains an
upstream TreeState and search InputState. Subscribe to `NavigationEvent` and
forward its document and `OpenMode` to the workspace. Arrow keys navigate and
expand/collapse, Enter opens permanently, single click previews and double click
pins. Search includes matching descendants and their ancestors; clearing it
restores the expansion set from before the search.

`Workspace::open`, `select`, `keep_open`, `close`, `split` and `move_view` operate
on retained views. `groups`, `active_group`, `active_view`, `document` and `page`
provide inspection; `dock()` exposes upstream layout capabilities. Opening from
navigation targets the most recently focused group. The selected group's top
border is stronger than inactive groups. Context menus capture their tab's ID,
so commands never depend on whichever group happens to have keyboard focus.

Each group has at most one italic preview. Opening a different preview releases
its old entity; Keep Open, double click, dragging and splitting make it permanent.
Close, Close Others, Close to the Right and Close All apply to the target group.
Closing its active view chooses right, then left; an empty group collapses.
Closing the final view leaves an empty workspace, without affecting its sidebar.

Keyboard shortcuts within the workspace:

| Command | Shortcut |
| --- | --- |
| Close active tab | Cmd+W / Ctrl+W |
| Next / previous tab | Ctrl+PageDown / Ctrl+PageUp; Cmd+Alt+Right / Left |
| Keep Open | Cmd+K, Enter / Ctrl+K, Enter |
| Tab commands menu | Tab to the ellipsis button, Enter, arrow keys, Enter |

The public action types can also be bound to application-specific shortcuts.
`AppShell` remains available for simple layouts with no workspace policy.

The workspace's navigation icons accept `impl Into<kit::component::Icon>`, including
the full assets catalog and consumer-provided SVGs. `Document::icon_path(path)`
sets an optional tab icon. Register an asset source containing those paths in the
application; catalog membership alone does not embed an icon in Kit's default
`Assets` source. The Gallery uses `AllAssets` for its Lucide icons.


Workspace appearance uses full-row tree selection and keyboard focus outlines.
Document groups have rounded outer frames and Kit Folder Tabs: a 36 px bar
with curved selected tabs joining the `tab_active` content surface. The strip
provides truncated names/tooltips, preview italics, always-visible close buttons
and a Split Right control. Per-document content owns scrolling. The Gallery adds
its inspector with Kit vertical resizing; applications can compose their own lower regions.

Keep Open is bound to Cmd/Ctrl+Shift+Enter. Cmd/Ctrl+K remains available for the
application's immediate search command rather than starting a workspace chord.


During a tab drag, a 3 px focus-colored insertion caret marks the before/after
slot under the pointer. The first/second half of a tab chooses its leading/trailing
edge; empty trailing space appends. No-op positions have no caret. The floating
label sits below the strip so it cannot obscure the marker. The overlay does not
consume layout space or mouse input. Escape cancels a drag, and content-area
split previews continue to come from Kit.


For framed opaque regions, `rounded_frame_overlay(radius, surround, border)`
covers child paint outside the curved corners and restores a one-pixel outline.
Add it to a relative, overflow-hidden container with a transparent one-pixel
border (the overlay owns the visible stroke) and supply
the actual opaque surrounding color. GPUI's overflow mask alone is rectangular.
The overlay retains ancestor clipping during nested splits, has no hitbox and
paints below Kit popovers/tooltips. The empty workspace uses the same treatment.
`frame_resize_handle_appearance()` keeps Kit resizing behavior while painting
only a short hover/drag indicator, avoiding a second full-length divider beside
an already-bordered frame.


## Editable tables

`pintle_gpui_kit::components::table` adds spreadsheet interaction over Kit DataTable.
Create `Entity<EditableTableState<D>>` once per view with `new(delegate, window,
cx)`, then render `EditableTable::new(&state)`. Implement `EditableTableDelegate`
for your application-owned data: visible row count and stable row keys, columns
with unique keys, cell values/editor kinds, synchronous commit validation, and
optional sorting. `CellEditor` is `Text`, `Dropdown(Vec<SharedString>)`, or
`ReadOnly`. Dropdown values must include all existing valid values; they cannot
accept arbitrary search text.

```rust,ignore
use pintle_gpui_kit::components::table::{EditableTable, EditableTableState};

// Construct once; MyRows implements EditableTableDelegate.
let grid = cx.new(|cx| EditableTableState::new(my_rows, window, cx));
// Render later:
EditableTable::new(&grid)
```

`CellAddress { row, column }` identifies source data, never a display offset.
Delegate keys must stay stable when values (including a displayed ID) change.
Commit updates the application model and returns `Result<(), SharedString>`;
an error leaves the draft and focus in place. Implement `commit_cells` for batch
editing: validate all `(CellAddress, value)` pairs before applying any changes,
then update/notify the shared model once. An error must leave all values intact.
Its default supports a single changed cell via `commit` and rejects larger
batches, preserving compatibility with single-cell delegates without partial
writes. The Gallery implements atomic batches. Sharing a retained data entity
between delegates propagates committed values, without sharing selection or
editor state. On a model notification call `update_delegate` to rebuild the
view's filter/sort index, or `refresh` if the visible keys already reflect it.
Do not mutate the Kit adapter directly during its render or callbacks.

`selection`, `active_cell` and `active_position` inspect retained state.
`select_cell` supports normal, extended and additive selection. Selection sets
contain no duplicates. Typing or F2 keeps the selection and opens one editor at
the active cell; committing fills every selected editable cell with that value,
including disjoint ranges. The targets are snapshotted as stable keys before
editing so sort/filter updates cannot redirect the edit. Read-only cells stay
unchanged. Each dropdown target must accept the chosen value, and application
validation must pass for the whole batch. A rejected batch keeps its editor,
selection and error message; Escape cancels without changing values. `begin_edit`,
`commit_edit` and `cancel_edit` expose the lifecycle to application commands.
`table()` exposes Kit scrolling and column geometry; `update_delegate` preserves
column widths and reconciles selection after source/index changes.

Subscribe to `EditableTableEvent::SelectionChanged`, `Committed`, `CopyFailed`
and `PasteFailed`.
`copy` preserves clipboard contents on an invalid selection. TSV includes no
headers, follows display order and quotes embedded tabs/newlines/quotes. A single
rectangle copies directly. Disjoint row ranges must share the same contiguous
columns; disjoint column ranges must share the same contiguous rows. Compatible
ranges pack together; holes or incompatible shapes display an error instead of
silently exporting a bounding rectangle with blanks.

Mouse drag and Shift extend a range. Cmd on macOS / Ctrl elsewhere adds ranges
or toggles a clicked cell. Arrow keys navigate; Shift+arrows extends. Enter and
Tab commit and move down/right; Shift reverses direction, with no edge wrapping.
Double-click/F2 edits the existing text; native typing replaces it. Alt+Down
opens dropdowns; search, Up/Down and Enter select a permitted option for all
selected editable cells. Dismissing the dropdown without choosing an option
does not broadcast the active cell's old value. Escape
cancels. Text editor shortcuts, composition and copying selected text stay with
Kit InputState. Committed-value undo/redo, formulas and fill handles
are outside this API.

Cmd/Ctrl+V pastes a rectangular Excel-style TSV block starting at the active
cell, using the current displayed row/column order. The old selection does not
repeat the block. On success, the pasted cells become selected and the active
cell stays at the starting destination (unless filtering removes it). Quoted
tabs/newlines, doubled quotes, Unicode, empty cells and CRLF/LF/CR row separators
are supported. A final row terminator does not add a blank destination row.
`parse_tsv` exposes the parser; `paste_text(text, window, cx)` exposes the same
paste operation to application commands.

Parsing, bounds, read-only and dropdown checks precede the atomic
`commit_cells` call. The delegate must validate the entire batch before writing.
Invalid values, ragged input or a block extending beyond the visible table reject
the entire paste, preserve the previous selection and clipboard, and display an
error. Paste does not add rows, coerce dropdown values or skip read-only targets.
While editing, Cmd/Ctrl+V pastes into Kit Input normally without committing the
grid or interpreting the editor's text as a cell block.

A workspace consumer can prevent invalid drafts from being discarded:

```rust,ignore
let guard = resource_view.clone();
WorkspacePage::new(resource_view, cx).before_deactivate(move |window, cx| {
    guard.update(cx, |view, cx| {
        view.editable.update(cx, |grid, cx| grid.commit_edit(window, cx))
    })
})
```

Use the same commit check before application-owned filter changes. The Gallery
shows this wiring, shared store notifications, status/type dropdown metadata,
and Details/Output integration. It does not write to a database or cluster.

Its database toolbar owns session drafts and Save/Cancel checkpoints in the
Gallery store. Internal row identities remain stable through insertion, deletion,
filtering and sorting, independently of editable display IDs. Toolbar actions
commit editors across views before mutating the draft, preserving partial input
when Add creates another row. Database Name fields may remain empty or whitespace
in new or existing draft records, including batch edits and paste. Required-name
validation happens at Save across all source rows, including filtered-out rows;
a failure reveals the first missing Name and retains every pending change without
advancing the checkpoint. Cancel discards editors and restores the last checkpoint.
Kubernetes and other strict delegates still validate each cell commit. The shared
adapter validates unchanged requested values too, since a delegate may reject an
initially empty required field.
Only actual value changes emit edit activity; delegates should skip no-op data
notifications.
`EditableTableState::set_error(Option<SharedString>, cx)` displays or clears an
application validation error without changing the delegate, editor, selection or
focus. Gallery uses it after revealing a required field rejected by Save; normal
selection and successful commit clear the error through the existing lifecycle.


Kubernetes views use a controlled Kit Popover with namespace checkboxes and a
name-only search input. Counts come from the shared source, while selected
namespaces, query, sorting and selection belong to each retained view. An empty
namespace selection includes all namespaces; unavailable selections remain in
the menu with a zero count so they can be cleared.

### Application context menus and catalog metadata

`NavigationTree::set_context_menu` configures an application builder over a
stable node ID and Kit `PopupMenu`; menus are disabled by default. `set_nodes` replaces catalog metadata while
retaining expansion and surviving selection; a deleted selection falls back to
its nearest surviving ancestor. `toggle_node`, `is_expanded` and `focus` support
application folder commands. Existing consumers need no menu configuration.

`EditableTableState::set_context_menu` receives a stable `CellAddress` or `None`
for background space. Menu builders execute after pointer dispatch releases
entity leases. Right-clicking selected cells preserves the range; another cell
becomes selected only when no editor is pending. Applications defer selection
changes until their action validates an existing editor. `set_selection`
reconciles stable keys; `paste_clipboard` uses the same atomic TSV checks as the
keyboard shortcut. Editor input handles its native text menu. Header menus are
not provided by this hook.

The shared `ContextMenuState` hosts Kit's accessible PopupMenu with window-edge
placement and focus restoration. Tree/grid hooks support Menu and Shift+F10.
`context_menu_contains_pointer` identifies an actual rendered menu hitbox, so an
application can let its menu commands reach their own validation/Cancel handlers
instead of being intercepted by a page-deactivation guard. Keyboard tab commands
continue to validate. Gallery snapshots menu targets and display order once on
open; sorting/filtering cannot retarget commands to a different source row.

`Workspace::update_document_metadata` updates all tabs for a resource key without
replacing pages; applications update their content metadata from the same catalog.
`open_in_split` opens directly beside the active group without a temporary source
tab. `close_resources` preflights all required page guards before removing any
views: `ClosePolicy::Validate` checks every page, while `Discard` skips only the
target resources' guards. Application-owned draft disposal follows successful
removal; unrelated validation failures leave the layout unchanged. Ordinary
open/select/close methods retain their existing behavior.

The Gallery owns its mutable session catalog. It preserves immutable resource
keys and fixture editor rules while renaming labels/context, and refreshes the
navigation and cached palette only on catalog changes. Its confirmed subtree
deletion removes source stores and palette entries, cancels affected editors,
and closes all matching views. No persistence or live backend operations are added.
