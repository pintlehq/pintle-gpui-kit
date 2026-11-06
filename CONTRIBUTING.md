# Contributing

Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) for
commit messages, for example `feat: add a reusable control` or `fix: restore focus`.

Use the pinned toolchain and keep `Cargo.lock` in version control. Run the checks
in the README and the feature matrix in CI before proposing a change. Launch the
Gallery for changes to rendering, focus, layout, or theme behavior.

Reuse upstream controls first, compose them second, and add a Pintle abstraction
only for a shared requirement. Keep domain services and application state outside
the foundation. Document public API changes alongside the implementation.

For an upstream upgrade, change the single workspace Kit pin, regenerate the
lockfile, inspect changed APIs, run every feature configuration, and exercise all
Gallery pages in both modes. Do not override individual GPUI snapshot crates.

UI tests render the production views and dispatch real pointer/keyboard events.
Use Kit's test-support development feature and stable IDs. Prefer assertions on
observable results, focus, and geometry over tests that mirror builder code.

Manual smoke checklist:

- Launch with `cargo run -p pintle-gpui-gallery --locked`.
- Visit all four pages and confirm readable text/icons in both themes.
- Enter Unicode text, save, dismiss with Escape and Done, and check focus.
- Change tabs, increment/reset the counter, and revisit the page.
- Preview/reset branding and switch modes without losing state.
- Toggle shell regions and resize between 1100 × 760 and 800 × 600.
- Check keyboard traversal and scrolling; close the final window to quit.

No remote push, publication, or release tagging is part of local initialization.

Workspace changes must preserve the boundary between Kit layout ownership and
Pintle document policy. Run rendered Gallery tests for preview replacement,
keyboard/tree navigation, scoped menus, drag reorder and transfer, split resizing,
empty workspaces, and per-view state. Exercise both fixture families at 1440 × 900
and 1024 × 680 in light/dark. Keep read-only fixtures separate from library code;
do not add a database driver, Kubernetes client, persistence layer or another
docking engine to satisfy a visual example.
