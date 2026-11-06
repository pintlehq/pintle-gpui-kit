# External application integration

For a complete working application, see `examples/starter` and its README.
Run `cargo run --locked -p pintle-gpui-starter` from this repository.
For an independent consumer, start with `cargo new my-pintle-app`, copy the
repository's `rust-toolchain.toml`, and add the dependency below. Use the starter
source as a reference for document tabs, retained views and editable tables.
Commit the application's own lockfile after resolving dependencies.

Use the Git repository and pin a real commit. Do not use filesystem paths outside
the application's own workspace for portable/distributed builds:

```toml
[package]
name = "my-pintle-app"
version = "0.1.0"
edition = "2024"
rust-version = "1.99"

[dependencies]
pintle-gpui-kit = { git = "https://github.com/pintlehq/pintle-gpui-kit", rev = "<full-commit-sha>", features = ["shell", "assets"] }
```

After maintainers publish the intended first tag, replace `rev` with
`tag = "v0.1.0"`. That tag is not created by initialization. crates.io publication
is a future option; the internal dependencies already carry matching versions.

## Minimal native application

```rust,no_run
use pintle_gpui_kit::{components::SectionHeader, kit as gpui, shell::AppShell};
use gpui::{AppContext, Context, IntoElement, Render, Window, WindowOptions};

struct Workspace;

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        AppShell::new(SectionHeader::new("My workspace")
            .description("Application content goes here"))
    }
}

fn main() {
    gpui::application().with_assets(gpui::assets::Assets).run(|cx| {
        pintle_gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() { cx.quit(); }
        }).detach();
        gpui::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| Workspace)
        }).expect("open application window");
        cx.activate(true);
    });
}
```

Create the application and register assets; initialize the foundation; construct
views inside Kit's window builder. Kit supplies the root wrapper. Use
`init_with_theme` for branding, as described in [theming](theming.md).
Applications own quit menus, commands, preference persistence, services, and state.

For theme-only usage set `default-features = false, features = ["theme"]`.
For compositions without the shell use the defaults. Add `assets` only when using
the embedded icon set; a consumer can register its own `AssetSource` instead.
Access additional controls through `gpui::component`, e.g.
`gpui::component::input::{Input, InputState}`.

For custom `IntoElement` derives, import the facade's Kit namespace as `gpui`.
Kit's action derive additionally needs `use pintle_gpui_kit::kit as gpui_kit;` in the
module containing `gpui::actions!`. These aliases require no direct GPUI dependency.

## Compatibility and tests

Pintle GPUI Kit 0.1.0 targets exactly Kit 0.8.1 and its GPUI 0.3.10 snapshot. Commit your
application's own `Cargo.lock`; a library lockfile is not inherited by consumers.
Do not mix public GPUI types from other snapshots or enable an alternative backend
without validating the full dependency graph.

UI tests may add `gpui-kit = { version = "=0.8.1", default-features = false,
features = ["test-support", "component"] }` under `[dev-dependencies]` for the
documented `#[gpui_kit::test]` macro. This is the same Kit release and enables
observation only for development targets. Production Gallery imports go entirely
through the foundation facade.

Upgrade foundation crates together and revalidate Kit integration, feature
combinations, theme persistence across mode changes, dialogs, input, and resizing.

## Retained document workspace

The factory must create a **fresh view entity** on each call. Share application
data entities or services in its captures, never a single table-view entity. A page's focus
handle must be tracked by a rendered element in that page. The workspace uses it
when selecting tabs and when identifying the last focused group.

```rust,no_run
use std::rc::Rc;
use pintle_gpui_kit::{kit as gpui, shell::{AppShell, workspace::*}};
use gpui::{prelude::*, App, AppContext, Context, Entity, FocusHandle, Focusable,
           IntoElement, Render, SharedString, Subscription, Window, div, px};

struct DocumentView { title: SharedString, focus: FocusHandle }
impl Focusable for DocumentView {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus.clone() }
}
impl Render for DocumentView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().track_focus(&self.focus).child(self.title.clone())
    }
}
struct Host {
    tree: Entity<NavigationTree>, workspace: Entity<Workspace>,
    _navigation: Subscription,
}
impl Host {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let document = Document::new("customers", "Customers", "Local / public");
        let workspace = cx.new(|cx| Workspace::new(Rc::new(|document, _, cx| {
            let view = cx.new(|cx| DocumentView {
                title: document.title.clone(), focus: cx.focus_handle(),
            });
            WorkspacePage::new(view, cx)
        }), window, cx));
        workspace.update(cx, |workspace, cx| {
            workspace.open(document.clone(), OpenMode::Permanent, window, cx);
        });
        let tree = cx.new(|cx| NavigationTree::new(vec![
            NavigationNode::folder("local", "Local", vec![
                NavigationNode::resource(document),
            ]),
        ], window, cx));
        let subscription = cx.subscribe_in(&tree, window,
            |this, _, event: &NavigationEvent, window, cx| {
                this.workspace.update(cx, |workspace, cx| {
                    workspace.open(event.document.clone(), event.mode, window, cx);
                });
            });
        Self { tree, workspace, _navigation: subscription }
    }
}
impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        AppShell::new(self.workspace.clone())
            .sidebar(self.tree.clone()).sidebar_width(px(280.))
    }
}
```

Initialize Pintle and construct `Host` in the same `open_window` callback as the
minimal example. The Gallery's `workbench.rs` shows a resizable sidebar using
Kit's `h_resizable` and `resizable_panel` with bounds. Its `data.rs` shows a real
EditableTableDelegate, stable cell keys, retained filter input and a shared
in-memory store. See the editable-table section in `components.md` for editor,
selection, clipboard and page-validation integration.
Supply application header/sidebar branding and asset sources normally; workspace
metadata is plain text and foundation owns no product identity.

Prefer the workspace methods for document lifecycle. `dock()` is an escape hatch
for upstream layout inspection and resizing; bypassing the workspace to register
unmanaged panels bypasses its resource and preview contracts. Floating windows,
cross-native-window moves, saved layouts, SQL execution and Kubernetes networking
are intentionally outside this layer.

## Embedded terminal

Enable `terminal` to use native terminal sessions without the application shell.
See [terminal integration](terminal.md) for launch options, lifecycle, input,
clipboard bindings and process-free UI tests. `shell` and `assets` remain separate
opt-ins, and the Starter is unchanged.
