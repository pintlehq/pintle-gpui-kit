//! A small application consuming only Pintle GPUI Kit's public APIs.
mod model;
mod table_page;
#[cfg(test)]
mod tests;

use gpui::component::{
    ActiveTheme, Sizable, TitleBar,
    button::{Button, ButtonVariants},
};
use gpui::{
    App, AppContext, Bounds, Context, Entity, IntoElement, Render, Subscription, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use pintle_gpui_kit::kit as gpui_kit; // Kit's action derive resolves this alias.
use pintle_gpui_kit::{
    kit as gpui,
    shell::{
        workbench::{self as chrome, WorkbenchState},
        workspace::{
            NavigationEvent, NavigationNode, NavigationTree, OpenMode, Workspace, WorkspacePage,
        },
    },
    theme,
};
use std::rc::Rc;
use table_page::TablePage;

gpui::actions!(starter, [Quit]);

struct Starter {
    tree: Entity<NavigationTree>,
    workspace: Entity<Workspace>,
    layout: Entity<WorkbenchState>,
    _layout: Subscription,
    _navigation: Subscription,
}
impl Starter {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let datasets = Rc::new(model::datasets(cx));
        let sources = datasets.clone();
        let workspace = cx.new(|cx| {
            Workspace::new(
                Rc::new(move |document, window, cx| {
                    let source = sources
                        .iter()
                        .find(|source| source.document.key == document.key)
                        .expect("registered resource");
                    // Fresh view entities, shared data: moving a tab retains its state;
                    // splitting creates independent selection/filter/scroll state.
                    let page = cx.new(|cx| {
                        TablePage::new(document.clone(), source.store.clone(), window, cx)
                    });
                    let grid = page.read(cx).grid.clone();
                    WorkspacePage::new(page, cx).before_deactivate(move |window, cx| {
                        grid.update(cx, |grid, cx| grid.commit_edit(window, cx))
                    })
                }),
                window,
                cx,
            )
        });
        let tree = cx.new(|cx| {
            NavigationTree::new(
                vec![NavigationNode::folder(
                    "people",
                    "People",
                    datasets
                        .iter()
                        .map(|data| NavigationNode::resource(data.document.clone()))
                        .collect(),
                )],
                window,
                cx,
            )
        });
        let navigation = cx.subscribe_in(
            &tree,
            window,
            |this, _, event: &NavigationEvent, window, cx| {
                this.workspace.update(cx, |workspace, cx| {
                    workspace.open(event.document.clone(), event.mode, window, cx);
                });
            },
        );
        workspace.update(cx, |workspace, cx| {
            workspace.open(
                datasets[0].document.clone(),
                OpenMode::Permanent,
                window,
                cx,
            );
        });
        let layout = cx.new(|cx| WorkbenchState::new(cx));
        let layout_subscription = cx.observe(&layout, |_, _, cx| cx.notify());
        Self {
            layout,
            _layout: layout_subscription,
            tree,
            workspace,
            _navigation: navigation,
        }
    }
}
impl Render for Starter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        chrome::Workbench::new(&self.layout, self.workspace.clone())
            .sidebar(chrome::sidebar(
                "starter-sidebar",
                "Explorer",
                self.tree.clone(),
                cx,
            ))
            .title(
                chrome::title_bar(cx).child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .px_3()
                        .child("My workspace")
                        .child(
                            Button::new("toggle-theme")
                                .ghost()
                                .small()
                                .label("Toggle theme")
                                .accessibility_label("Toggle light or dark theme")
                                .on_click(|_, _, cx| {
                                    let mode = if cx.theme().mode.is_dark() {
                                        theme::ThemeMode::Light
                                    } else {
                                        theme::ThemeMode::Dark
                                    };
                                    theme::set_mode(mode, cx);
                                }),
                        ),
                ),
            )
            .status(chrome::status_bar(
                "Demo data",
                "Changes stay in memory · Cmd/Ctrl+C / V to copy / paste cells",
                cx,
            ))
    }
}
fn initialize(cx: &mut App) {
    pintle_gpui_kit::init(cx);
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([gpui::KeyBinding::new(
        if cfg!(target_os = "macos") {
            "cmd-q"
        } else {
            "ctrl-q"
        },
        Quit,
        None,
    )]);
    cx.set_menus(vec![gpui::Menu {
        name: "My workspace".into(),
        items: vec![gpui::MenuItem::action("Quit", Quit)],
        disabled: false,
    }]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}
fn window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1280.), px(800.)),
            cx,
        ))),
        window_min_size: Some(size(px(960.), px(640.))),
        ..TitleBar::window_options()
    }
}
fn main() {
    gpui::application()
        .with_assets(gpui::assets::AllAssets)
        .run(|cx| {
            initialize(cx);
            gpui::open_window(window_options(cx), cx, |window, cx| {
                window.set_window_title("My workspace");
                cx.new(|cx| Starter::new(window, cx))
            })
            .expect("open starter window");
            cx.activate(true);
        });
}
