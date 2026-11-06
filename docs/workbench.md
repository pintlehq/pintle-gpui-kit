# Application-owned content, shared desktop UI

Use `pintle_gpui_kit::{kit, shell, theme}`. Initialize the facade before constructing
views; `kit::open_window` owns the single root for the native window. Enable
`shell` and `assets` for the full Pintle desktop appearance. Terminal is separate.

Create `Entity<shell::workbench::WorkbenchState>` with `WorkbenchState::new(cx)`
and observe it from the app view to request redraws. Pass it and your main content
to `Workbench::new`, then supply `.title`, `.activity`, `.sidebar`, `.panel` and
`.status` elements as needed. Omitted slots consume no space. Use the shared
`title_bar`, `title_content`, `search_button`, `activity_bar`, `sidebar`, `frame`,
`panel_tabs`, `panel_tab` and `status_bar` helpers for consistent chrome.

Visibility setters notify observers and restore a supplied content focus handle
when hiding a region. With a docked Workspace use `active_focus_handle(cx)`.
Resize entities remain alive while their regions are hidden. Keep application
panel-tab selection and content entities retained as well. `next_panel_tab`
implements roving header-key navigation; invoke it only while a tab header has
focus so content editors keep their arrow keys.

`CommandPalette::new(entries, window, cx)` retains Kit's command input.
Each `CommandEntry::new(id, group, label)` can supply an icon and search keywords.
IDs must be unique within the application. `set_entries` replaces catalog
presentation. `render` reports the original stable ID after filtering and closes
the dialog before the application callback runs. `open` mounts the modal and
focuses its input once; supply a renderer reading the current palette through a
weak app entity. Action-backed entries optionally dispatch a Kit action instead;
attach its handler to the modal renderer because it has a separate action ancestry.

See Gallery for a changing catalog, application validation and optional terminal
sessions; see Starter for an independent editable-table consumer. Lightship uses
these APIs for offline Kubernetes demo pages. No kit module owns cluster clients,
resource mutation rules, application routes or persistence.

## Resource browsers

`NavigationTree::new` retains the original 22px compact rows and expanded folders.
Use `NavigationTree::with_options(nodes, NavigationTreeOptions { .. }, window, cx)`
for a different row height, optional font family, initial expanded-ID set, and
`NavigationDecoration` callback. The callback receives a stable node ID and App;
it returns caller-owned trailing content. Searching still expands matching
ancestors temporarily and restores the retained expansion when cleared.

`components::resource_table::ResourceTable::new(&table_state)` presents an upstream
`TableState<D: TableDelegate>` with 36px rows, 32px headers, monospace text and a
rounded frame without striping. `ResourceListHeader::new(title, status, toolbar)`
accepts arbitrary elements in each slot. The caller owns data, column definitions,
selection, filtering, sorting and actions; upstream owns virtualization, focus,
scrolling and column resizing. This composition does not change `EditableTable`.

Lightship demonstrates cluster-specific IDs and read-only resource tables. The kit
has no knowledge of Kubernetes types, fixtures, namespace rules or connections.

For bulk controls, compose `FloatingActionBar::new(summary, actions, dismiss)` and
pass it to `ResourceTable::floating_actions(...)`. All three slots are arbitrary
caller-owned elements; the kit does not own selection, command IDs or mutations.
The 48px bar overlays the table, centered 12px above its bottom edge. Showing or
hiding it does not resize the viewport, reserve space or change scroll offsets.
Rows scroll underneath it, including the final rows; dismiss the bar to reveal
covered content. Only the bar blocks pointer input, while transparent space beside
it remains interactive. The actions stay fixed during horizontal and vertical
scrolling. Omit the optional slot to retain the ordinary table layout. Consumers
should collapse long action lists into an upstream dropdown in narrow panes.
