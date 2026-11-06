# Native workspace starter

A small starting application using only Pintle GPUI Kit's public APIs. It has a
searchable sidebar tree, retained preview/permanent tabs with docking and splits,
and virtualized editable tables. No Gallery code or bundled Gallery fonts are
required. The frameless native window starts at 1280 × 800, minimum 960 × 640,
with shared Pintle light/dark modes, bundled data fonts and native window controls.

Run it from the kit repository:

```sh
cargo run --locked -p pintle-gpui-starter
cargo test --locked -p pintle-gpui-starter
```

## Start your own project

Create a separate Rust binary project with `cargo new my-desktop-app`, then
follow the [integration guide](../../docs/integration.md) to add the public
`pintle-gpui-kit` facade and initialize a native window. Use this example's source
as a reference for your document factory, data model and table pages.

Pin the kit to a verified commit in `https://github.com/pintlehq/pintle-gpui-kit`.
Use Rust 1.99.0 and GPUI Kit 0.8.1; copy `rust-toolchain.toml` into your project.
For rendered tests, add the integration guide's test-support development
dependency. The first Cargo command creates the application's own `Cargo.lock`;
commit it, then use `--locked`. Keep any local dependency override untracked and
use the verified Git dependency for portable builds. Do not assume a release tag
exists.

## Adapt the application

| File | Change here |
| --- | --- |
| `src/main.rs` | Window title, menus, theme, sidebar nodes, document factory and initial tab |
| `src/model.rs` | Columns, stable record keys, dropdown choices, validation and your data service |
| `src/table_page.rs` | Per-view filter/toolbar and retained table state |
| `src/tests.rs` | Rendered application tests using the real public components |

Contacts and Team each contain 1,000 session-only records. Single-click a tree
item to preview; double-click to keep it. Tabs provide drag/reorder, close commands
and Split Right / Split Down. A split creates a fresh view over the same retained
store; selection, filter, sort and scroll stay independent. A validation guard
commits pending text before tab changes, splitting or closing a view and blocks
those actions when validation fails.

Select cells, type or use F2, and press Enter/Tab to commit. A multi-cell edit
fills all selected editable cells. Status uses a searchable Active/Inactive
dropdown (Alt+Down also opens it). Cmd/Ctrl+C copies TSV; Cmd/Ctrl+V pastes a block
starting at the active cell. Paste validates the entire block before applying
anything. IDs are read-only, names are required, and nonempty email values must
contain @. Invalid dropdowns, read-only targets and paste overflow reject the
whole block. Escape cancels a draft. Changes disappear when the app exits.

The model's `commit_cells` validates before mutation and sends one model
notification. Keep that atomic contract when replacing Store with your backend.
Retain InputState and EditableTableState outside render. Create a fresh TablePage
in the workspace factory, capture only shared models, and keep the
`before_deactivate` guard. See the kit docs for initialization, branding
and upstream `pintle_gpui_kit::kit` escape hatches.

macOS is the local validation platform. Windows/Linux require native build and
smoke verification on those systems.
