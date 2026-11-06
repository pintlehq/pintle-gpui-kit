# Architecture

The foundation is a small set of ordinary Rust libraries. The Gallery is an
application consumer, not a framework host.

```text
gallery → pintle-gpui-kit → shell → components → theme → gpui-kit
                    ↘ components/theme             ↑
Every library uses Kit directly where needed ────────┘
```

The facade exports `kit`, `theme`, `components`, `shell`, and optional `terminal` namespaces. `kit`
preserves upstream access without colliding with Pintle component names.
`init` initializes Kit and default Pintle themes once per App. `init_with_theme`
also installs caller configuration and can replace it on subsequent calls.

The theme crate owns presentation policy and translates it into upstream
`ThemeConfig`. The component crate owns stateless repeated compositions. The
shell crate arranges caller-supplied elements; it owns no routes or domain state.
The library boundaries remain independently consumable. Direct consumers of the
lower-level crates initialize Kit themselves before installing themes.

The shell dependency on components supports its public `Panel` re-export. No
crate depends on the facade from below. The shell is unrelated to GPUI Kit's
JavaScript extension host named `gpui-shell`.

Kit `open_window` owns the one Base Root per native window. Do not nest roots
inside pages or wrap the returned content in another Root. Kit's component
initialization installs window overlay behavior used by dialogs.

## Features and dependency costs

Default facade features are theme and components. Shell, terminal and assets are opt-in.
No-default-features retains Kit's runtime/base layer. Theme enables the upstream
styled component library, which is currently the owner of Theme. Pintle does not
promise that disabling compositions removes Kit's component implementation.

The lockfile may contain optional or other-platform packages that are not built
on the current target. Use `cargo tree -e features` to inspect active dependencies.
The project does not enable Fast, speech, syntax engines, WebViews, or JS extensions.

Add a crate when a capability has a coherent public contract and independent
consumers. Start a new composition as a module; avoid registries, factories,
dependency injection, or routing infrastructure without a demonstrated use case.

All foundation crates advance together at version 0.1.0. Their public Kit types
make the exact upstream release part of compatibility. Upgrade Kit and validate
downstream applications together rather than mixing GPUI versions.

## Document workspaces

`shell::workspace` is optional alongside `AppShell`, under the existing `shell`
feature. It introduces no additional dependency. `NavigationTree` uses Kit's
virtualized Tree and retained input. `Workspace` owns document metadata and view
entities. Kit's `DockArea` exclusively owns the pane tree, tab membership, split
sizes, drag hit testing, drop targets and normalization of empty groups.

`ResourceKey` identifies application data; `ViewId` identifies one retained panel
entity. A caller supplies a `ViewFactory` producing `WorkspacePage { view, focus }`.
The private `DocumentPanel` adapter implements upstream panel and focus traits.
The workspace skin delegates frames, split handles and drop indicators to
`DockSkin`. Active retained views render normally instead of using the styled
dock's whole-panel scene cache: native smoke tests on Kit 0.8.1 exposed stale
text positions after switching table documents through that cache. This avoids
an outer dock scroll region as well; document tables still virtualize rows and
own their scroll state. Its flat tab strip adds preview titles, icons, pinning,
scoped menus, accessibility tab roles and automatic reveal of keyboard-selected
tabs. Drag payloads and drop operations remain Kit's `DragPanel` / group APIs.
There is no parallel layout tree, router, database client or Kubernetes client.

Opening a resource reuses its view within the target group. A split invokes the
factory again; moving a tab preserves its entity. If a moved resource already
exists in the destination, the activated moved view wins and the destination's
old view is released. A `DockEvent` reconciliation also covers upstream content
edge drops and direct layout changes. All keys and navigation node IDs must be
unique within their application catalog.

The Gallery shares one retained `Entity<DatasetStore>` per resource. Immutable
fixture metadata and stable row keys are shared through `Rc`; edits live only in
the store's values. Every view owns a filter, visible row index vector, sort,
`EditableTableState`, and its underlying Kit TableState. Store notifications
rebuild each view's index independently. Selections use source row/column keys;
sorting preserves identity and filtering removes hidden cells from selection.
Column widths and scroll remain per-view Kit state, including across edits.

`components::table` adapts Kit's virtualized TableDelegate. Pintle supplies
selection sets and cell interactions, disabling conflicting upstream row/cell
selection. Only the active editor owns an InputState; dropdowns use a controlled
Kit Popover. Native text/composition enters through EntityInputHandler and is
forwarded to Kit's text engine, rather than synthesizing characters from keys.
There is no spreadsheet rendering engine or replacement scroll implementation.

`WorkspacePage::before_deactivate` supplies an optional synchronous validation
hook. Workspace selection, opening, splitting and close commands call it before
changing documents. Pointer entry into another document also validates other
pages. Returning false keeps the old page and focuses its error. Moving a view
retains its editor entity and draft rather than invoking this lifecycle hook.
Database required-name validation belongs to Gallery's Save checkpoint, so incomplete
new and existing draft rows can pass the cell-commit and navigation guards.
Kubernetes and Starter keep their strict per-cell validation. Save first commits
all editors for its table, then validates every source row and reveals the first
missing Name with an application-supplied table error. Dropdown validation and
atomic batch application remain the shared adapter's existing contract.
No layout, view state or edited data is persisted across launches.


## Gallery workbench regions

The shared shell owns activity/title/status chrome and retained workbench layout.
Gallery supplies its catalog, actions and Details/Output/Terminal content. Kit
resizable groups place the Explorer beside the editor and the inspector below
it. Sidebar and panel resize state remains in retained entities while hidden.
The panel starts at 240 px, ranges from 120 px to half the work area, and leaves
at least 240 px for the editor container. Split layouts remain DockArea-owned.

Details resolves the current Workspace view and reads its active cell and shared
row values directly by stable row key, so insert/delete/revert notifications cannot
make it index a stale view projection. Editable-table events notify the inspector;
weak references prevent a cycle between views and workspace.
Workspace observations record layout changes, and table observations record filter
and sort changes; commit events record edits. Output stores at most 500 entries and has independent scrolling
and Clear. Layout observations commit at application event boundaries, matching
native input events; render functions do not generate activity entries.

Details and activity recording are Gallery application features. Workbench chrome,
command presentation, Workspace, AppShell and view factories are reusable APIs.
The optional page validation hook remains available. Terminal processes
are isolated in the opt-in terminal crate; Workspace and AppShell own no processes
or domain backend clients.


The tab renderer retains per-frame tab bounds and typed drag-hover state. One
geometry resolver supplies both the painted insertion caret and the Kit drop
slot, adjusting for source detachment within a group. A non-scrolling wrapper
measures the viewport because a scrolling element's prepaint bounds include its
own scroll offset. The caret is a hitbox-free canvas overlay; there is no separate
layout or drag engine. Explicit tab slots also prevent an old content split
preview from affecting a subsequent tab-bar drop.

## Native terminals

`pintle-gpui-terminal` depends on theme and Kit, independently of shell/components.
`portable-pty` 0.9.0 supplies Unix PTYs and Windows ConPTY;
`alacritty_terminal` 0.26.0 supplies the streaming parser, grid, selections and history.
Pintle owns the GPUI renderer and input adapter, not an ANSI parser.

A retained `TerminalSession` owns a backend; a retained `TerminalView` observes it.
Blocking spawn/write/resize/wait operations run on a coordinator thread. A reader
parses 8 KiB chunks into a shared model, and a separate cancellation worker can
interrupt blocked input. Input uses a bounded 32-item queue (at most 1 MiB per
item); rejection returns an error before queuing any bytes. Protocol replies apply
backpressure on their worker rather than being dropped. Output is parsed directly
into bounded scrollback, without an unbounded transcript queue. Notifications use
a single-slot channel and are coalesced at 16 ms intervals. Only the visible grid
is shaped and painted; hidden sessions continue parsing without mounting a view.

Unix reads poll with cancellation and drain available output after exit. Closing
signals the foreground and shell process groups before releasing PTY descriptors;
Windows closes ConPTY. Child handles are reaped on workers, including setup errors.
App quit asynchronously awaits backend completion. Restart replaces the backend
and emulator; the view retains its identity. OSC 52 clipboard requests are disabled;
clipboard access is through explicit view copy/paste actions.

Gallery alone owns the session picker and panel policy. Its first Terminal visit
creates one shell, and closing the last session leaves an empty state. Hidden
panels retain their session entities. Terminal key context excludes Gallery Ctrl
bindings (including Quit on Windows/Linux); macOS Cmd bindings remain available.
Arrow navigation between inspector tabs is limited to their header focus handles.
