# Pintle GPUI Kit

A small native UI foundation for Pintle's independent desktop developer tools:
Lightship, Gridbox, and Sendbox. Built with Rust and **GPUI Kit 0.8.1**.

Pintle supplies shared light/dark workspace themes, reusable compositions, and an optional
application shell with an optional docked workspace. Buttons, trees, virtualized
tables, inputs, tabs, badges, dialogs, rendering, focus, and
window management come from GPUI Kit.

## Start a new application

The [workspace starter](examples/starter/README.md) includes a sidebar tree,
docked tabs/splits and editable tables with atomic validation and Excel-style
copy/paste. Run the example from this repository:

```sh
cargo run --locked -p pintle-gpui-starter
cargo test --locked -p pintle-gpui-starter
```

To build an independent application, create a Rust binary project and depend on
`pintle-gpui-kit` at a verified Git commit. Follow the
[integration guide](docs/integration.md) for the manifest and native application
entry point; use the starter source as a reference for retained tables and tabs.

## Run the Gallery

On macOS 15 or later, install Xcode (including its command line tools) and Rust
through rustup. The repository pins Rust 1.99.0 with rustfmt and Clippy.

```sh
cargo run -p pintle-gpui-gallery --locked
```

The Gallery follows the supplied VS Code light/dark references: a frameless
window with a 35 px custom title bar, native macOS traffic lights, centered search,
activity bar, Explorer, rounded editor-group frames and compact folder-style document tabs.
UI chrome uses the system font; table data and Output use JetBrains Mono at 13 px.
Click the centered search or press Cmd/Ctrl+K to open a command palette with
Database, Kubernetes, Foundation and workspace commands. Search by resource name
or context, then click or press Enter to open a permanent tab. Escape clears the
query first, then closes the palette. Its 360 px height stays fixed as results
change; the activity-bar Search still focuses the
Explorer's resource filter. Cmd/Ctrl+B toggles the Explorer, and Cmd/Ctrl+J
toggles the bottom panel. **Ctrl+`** opens/focuses Terminal (or hides it when
already focused). **Details** inspects the selected row in the active view;
**Output** records real open/close/move/split/filter/sort/edit activity, capped at 500 entries.
The panel starts at 240 px, resizes, and retains its height when closed and reopened.
**Terminal** starts a real local shell on first use, with independent sessions,
New/Restart/Close Session controls, Unicode, ANSI colors, scrollback and full-screen
programs. Hiding the panel keeps its shells running. Ctrl combinations go to the
focused shell; macOS uses Cmd+C/V and Windows/Linux use Ctrl+Shift+C/V for clipboard.
See [terminal integration](docs/terminal.md).

Database fixtures include customers, orders and products; Kubernetes fixtures
include Pods, Deployments and Services. Each dataset contains 1,000 editable in-memory
rows, with filtering, sorting, column resizing and virtual scrolling. Select ranges
by dragging or Shift; Cmd/Ctrl adds disjoint ranges. Typing or F2 edits the selected
cells together; Enter/Tab commits one value to every editable selected cell,
and Escape cancels. Double-click edits a cell.
Status/type cells offer searchable closed dropdowns (Alt+Down also opens them);
choosing an option fills the selected cells. Validation rejects the entire edit
if any target is invalid; read-only cells keep their values.
Cmd/Ctrl+C copies compatible selected ranges as TSV for Excel. Cmd/Ctrl+V pastes
an Excel cell block from the active cell; validation must pass for the entire
block before any values change. Edited values are
shared across views of that resource until the application quits; view selection,
filters, sort and scroll remain independent. See [editable tables](docs/components.md#editable-tables).

Database headers have an Add/Delete/Save/Cancel toolbar. Add inserts an inline
draft and starts editing. Add can be pressed repeatedly before filling earlier
rows; both new and existing database records may keep a blank Name in the draft.
Delete stages the distinct selected rows. Save commits open editors across splits,
requires every Name in the full table (including filtered-out rows), and checkpoints
the complete draft in memory only when valid. Cancel
discards all pending changes and editors across splits. Drafts survive closing
and reopening tabs within the session; nothing is written to a database or disk.
Kubernetes headers instead offer multiple namespace checkboxes (none selected
means All) and a resource-name search. Namespace and name filters combine, stay
independent per view, and retain counts from the full source. Each Kubernetes
fixture has 500 default, 300 staging and 200 production resources.

Right-click a sidebar node to open a permanent tab or a new split, copy its
display path, or expand/collapse a folder. DB/Kubernetes resources and their
subfolders also offer session-only Rename/Delete; roots and Foundation are
protected. Rename updates all retained tabs, breadcrumbs, Details and palette
entries without replacing views. Delete confirms affected resources, tabs and
drafts, closes every affected split, and discards those drafts. Invalid editors
in unrelated resources block the entire removal. Restarting restores the fixtures.

Table context menus preserve selected ranges and offer Copy/Copy Rows, Paste
and Edit. DB menus include the same CRUD and Save/Cancel commands as the toolbar;
Kubernetes menus include Copy Name/Namespace and Filter to This Namespace.
Right-clicking empty table space offers DB draft commands or Clear Filters for
Kubernetes. Menu/Shift+F10 opens the menu from the focused tree node or cell;
arrows/Enter select commands and Escape restores focus. Text editors keep their
native text menu.

Single-click a tree resource for a preview; double-click to keep it open. Drag
tabs to reorder or move them; Cmd/Ctrl+Shift+Enter keeps a preview open. Use the tab context menu for Split Right /
Split Down and scoped close commands. Each view keeps its own state. The
Foundation tree retains the Components, Theme and simple Shell examples.
The window starts at 1440 × 900 with a 1024 × 680 minimum; the 280 px sidebar
resizes between 200 and 480 px. Close the window or use Quit to exit.
The Gallery appearance is the kit default, shared with Lightship.
`ThemeOptions::neutral()` provides an alternative neutral preset. See [theming](docs/theming.md).

## Workspace

```text
crates/
  pintle-gpui-kit/             Public facade and initialization
  pintle-gpui-theme/       Palettes, upstream configuration, layout tokens
  pintle-gpui-components/  Compositions, frames, context menus and editable tables
  pintle-gpui-shell/       AppShell, Workbench, chrome, command palette and docked Workspace
  pintle-gpui-terminal/    Optional PTY sessions and native terminal surface
examples/gallery/         Executable public API consumer
docs/                     Architecture, theming, components, integration
.github/workflows/ci.yml  macOS validation
```

## Use the library

Until a release exists, pin an actual repository commit with a Git `rev`.
`v0.1.0` is the intended first release tag, **not an existing release promised by
this repository**. See [integration](docs/integration.md) for a complete example.

```toml
[dependencies]
pintle-gpui-kit = { git = "https://github.com/pintlehq/pintle-gpui-kit", rev = "<full-commit-sha>", features = ["shell", "assets"] }
```

```rust,ignore
use pintle_gpui_kit::{components::SectionHeader, kit, shell::AppShell};

// In an application run callback, before constructing views:
pintle_gpui_kit::init(cx);
// In a render implementation:
AppShell::new(SectionHeader::new("Workspace"));
```

| Feature | Behavior |
| --- | --- |
| defaults | `theme` + `components` |
| `theme` | Pintle theme API and upstream styled component layer |
| `components` | Theme plus Pintle compositions |
| `shell` | Components plus AppShell and docked workspace |
| `terminal` | Theme plus native PTY terminal; independent of `shell` |
| `terminal-test-support` | Terminal plus deterministic process-free sessions for tests |
| `assets` | Kit icons and bundled JetBrains Mono fonts; opt in at the application |
| no defaults | Kit runtime and base layer only |

`pintle_gpui_kit::kit` exposes upstream capabilities without restricting their API.
Feature selection is additive across Cargo's dependency graph. Theme-only builds
still compile Kit's styled component crate because its Theme belongs there.
Disabling Pintle compositions cannot split that upstream crate into smaller units.

## Development and portability

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

macOS is the primary development and CI platform. Windows requires MSVC and the
Windows SDK; Linux requires GPUI's native graphics/font libraries and a graphical
session for the Gallery. Consult the [upstream installation guide](https://gpui-kit.com/docs/installation/).
No platform success is implied without a corresponding validation run.

- [Architecture](docs/architecture.md)
- [Theming and branding](docs/theming.md)
- [Component conventions](docs/components.md)
- [Application integration](docs/integration.md)
- [Contributing](CONTRIBUTING.md)
- [Validation results and limitations](docs/validation.md)

Licensed under [Apache-2.0](LICENSE). Upstream dependencies retain their licenses.

## Shared desktop workbench

`shell::workbench::Workbench` arranges caller-supplied title, activity, sidebar,
content, bottom-panel and status slots. Retain and observe a `WorkbenchState`
entity to keep region visibility and resize state across toggles. Shared chrome
helpers provide frames, title/search, activity bar, panel tabs and status styling.
`shell::command_palette::CommandPalette` accepts application-owned `CommandEntry`
IDs and presentation, and reports selected IDs without importing an app catalog.
See [workbench integration](docs/workbench.md).

The Gallery is a consumer of these APIs and owns all demo fixtures and data rules.
[Lightship](https://github.com/pintlehq/lightship) is a separate Kubernetes GUI
consumer; no Kubernetes or database client belongs in the shared kit.
