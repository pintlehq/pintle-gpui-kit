use super::*;
use crate::data::ResourceView;
use crate::inspector::Inspector;
use gpui::assets::IconName;
use gpui::component::menu::{DropdownMenu, PopupMenuItem};
use gpui::{AnyElement, FocusHandle, Focusable};
use pintle_gpui_kit::shell::workbench::{self as chrome, WorkbenchState};
use pintle_gpui_kit::shell::workspace::{
    Document, NavigationEvent, NavigationTree, OpenMode, ViewId, Workspace, WorkspacePage,
};
use std::rc::Rc;

mod command_palette;
mod context_menu;
use command_palette::CommandPalette;

type LayoutEntry = (ViewId, gpui::component::dock::NodeId, SharedString);
pub struct Workbench {
    pub workspace: Entity<Workspace>,
    pub catalog: Entity<crate::catalog::Catalog>,
    rename: Option<context_menu::RenameState>,
    pub navigation: Entity<NavigationTree>,
    pub inspector: Entity<Inspector>,
    pub layout: Entity<WorkbenchState>,
    command_palette: CommandPalette,
    last_layout: Vec<LayoutEntry>,
    last_active: Option<ViewId>,
    _subscriptions: Vec<gpui::Subscription>,
}
impl Workbench {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let catalog = cx.new(crate::catalog::Catalog::new);
        let nodes = catalog.read(cx).nodes.clone();
        let command_palette = CommandPalette::new(&nodes, window, cx);
        let initial = catalog
            .read(cx)
            .node("db/local/shop/customers")
            .unwrap()
            .document
            .clone()
            .unwrap();
        let layout = cx.new(|cx| WorkbenchState::new(cx));
        let inspector = cx.new(|cx| Inspector::new(layout.clone(), cx));
        let activity = inspector.downgrade();
        let factory_catalog = catalog.clone();
        let workspace = cx.new(|cx| {
            Workspace::new(
                Rc::new(move |document, window, cx| {
                    let source = factory_catalog.read(cx).sources.get(&document.key).cloned();
                    if let Some((data, store)) = source {
                        let view = cx.new(|cx| ResourceView::new(data, store.clone(), window, cx));
                        view.update(cx, |view, cx| {
                            view.update_metadata(document.clone(), window, cx);
                            view.connect_catalog(&factory_catalog, window, cx);
                            view.connect_context_menu(cx);
                            view.connect_inspector(activity.clone(), cx);
                        });
                        let guard = view.clone();
                        let source = store;
                        WorkspacePage::new(view, cx).before_deactivate(move |window, cx| {
                            if source.read(cx).discarding_with_pointer(window, cx) {
                                return true;
                            }
                            guard.update(cx, |view, cx| {
                                view.editable
                                    .update(cx, |state, cx| state.commit_edit(window, cx))
                            })
                        })
                    } else {
                        let Some(page) = Page::ALL
                            .into_iter()
                            .find(|p| p.id() == document.key.0.as_ref())
                        else {
                            return WorkspacePage::new(
                                cx.new(|cx| context_menu::UnavailableResource {
                                    focus: cx.focus_handle(),
                                }),
                                cx,
                            );
                        };
                        let view = cx.new(|cx| {
                            let mut view = Gallery::new(window, cx);
                            view.page = page;
                            view.embedded = true;
                            view
                        });
                        WorkspacePage::new(view, cx)
                    }
                }),
                window,
                cx,
            )
        });
        inspector.update(cx, |inspector, _| {
            inspector.workspace = Some(workspace.downgrade())
        });
        workspace.update(cx, |w, cx| {
            w.open(initial, OpenMode::Permanent, window, cx);
        });
        let navigation = cx.new(|cx| NavigationTree::new(nodes, window, cx));
        let navigation_sub = cx.subscribe_in(
            &navigation,
            window,
            |this, _, event: &NavigationEvent, window, cx| {
                let document = this
                    .catalog
                    .read(cx)
                    .node(&event.document.key.0)
                    .and_then(|node| node.document.clone());
                if let Some(document) = document {
                    this.workspace.update(cx, |w, cx| {
                        w.open(document, event.mode, window, cx);
                    });
                }
            },
        );
        let workspace_sub = cx.observe(&workspace, |this, _, cx| this.synchronize(cx));
        let layout_sub = cx.observe(&layout, |_, _, cx| cx.notify());
        let catalog_sub = cx.observe_in(&catalog, window, |this, _, window, cx| {
            this.synchronize_catalog(window, cx)
        });
        let mut this = Self {
            workspace,
            catalog,
            rename: None,
            navigation,
            inspector,
            layout,
            command_palette,
            last_layout: vec![],
            last_active: None,
            _subscriptions: vec![navigation_sub, workspace_sub, catalog_sub, layout_sub],
        };
        this.connect_navigation_menu(cx);
        this.synchronize(cx);
        this
    }
    fn synchronize(&mut self, cx: &mut Context<Self>) {
        let w = self.workspace.read(cx);
        let groups = w.groups(cx);
        let active = groups
            .iter()
            .find(|group| Some(group.id) == w.active_group())
            .or(groups.first())
            .and_then(|group| group.active);
        let layout = groups
            .into_iter()
            .flat_map(|group| {
                group
                    .views
                    .into_iter()
                    .filter_map(|id| w.document(id, cx).map(|d| (id, group.id, d.title)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        if layout == self.last_layout && active == self.last_active {
            return;
        }
        let layout_changed = layout != self.last_layout;
        self.last_active = active;
        if !layout_changed {
            self.inspector
                .update(cx, |inspector, cx| inspector.refresh_details(cx));
            return;
        }
        let mut messages = vec![];
        for (id, group, title) in &layout {
            match self.last_layout.iter().find(|(old, _, _)| old == id) {
                None => messages.push(format!(
                    "{} {title}",
                    if self.last_layout.iter().any(|(_, _, t)| t == title) {
                        "Split"
                    } else {
                        "Open"
                    }
                )),
                Some((_, old_group, _)) if old_group != group => {
                    messages.push(format!("Move {title} to group {}", group.as_u64()))
                }
                Some((_, _, old_title)) if old_title != title => {
                    messages.push(format!("Rename {old_title} to {title}"))
                }
                _ => {}
            }
        }
        for (id, _, title) in &self.last_layout {
            if !layout.iter().any(|(new, _, _)| new == id) {
                messages.push(format!("Close {title}"));
            }
        }
        if layout.len() == self.last_layout.len()
            && layout != self.last_layout
            && messages.is_empty()
        {
            messages.push("Reorder tabs".to_owned());
        }
        self.last_layout = layout;
        self.inspector.update(cx, |inspector, cx| {
            for message in messages {
                inspector.record(message, cx);
            }
            inspector.refresh_details(cx);
        });
        if layout_changed {
            cx.notify();
        }
    }
    fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.workspace.read(cx).active_focus_handle(cx);
        self.layout.update(cx, |layout, cx| {
            layout.set_sidebar_visible(!layout.sidebar_visible, &focus, window, cx)
        });
    }
    fn search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.workspace.read(cx).active_focus_handle(cx);
        self.layout.update(cx, |layout, cx| {
            layout.set_sidebar_visible(true, &focus, window, cx)
        });
        window.focus(&self.navigation.read(cx).search.focus_handle(cx), cx);
    }
    fn activity_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        chrome::activity_bar()
            .child(
                Button::new("activity-explorer")
                    .icon(IconName::Files)
                    .ghost()
                    .size(px(36.))
                    .selected(self.layout.read(cx).sidebar_visible)
                    .tooltip("Explorer (⌘B / Ctrl+B)")
                    .accessibility_label("Explorer")
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_sidebar(window, cx))),
            )
            .child(
                Button::new("activity-search")
                    .icon(IconName::Search)
                    .ghost()
                    .size(px(36.))
                    .tooltip("Search resources")
                    .accessibility_label("Search resources")
                    .on_click(cx.listener(|this, _, window, cx| this.search(window, cx))),
            )
            .child(div().flex_1())
            .child(
                Button::new("toggle-appearance")
                    .icon(if cx.theme().mode.is_dark() {
                        IconName::Sun
                    } else {
                        IconName::Moon
                    })
                    .ghost()
                    .size(px(36.))
                    .accessibility_label("Toggle appearance")
                    .tooltip("Toggle light / dark")
                    .on_click(|_, _, cx| {
                        theme::set_mode(
                            if cx.theme().mode.is_dark() {
                                theme::ThemeMode::Light
                            } else {
                                theme::ThemeMode::Dark
                            },
                            cx,
                        );
                    }),
            )
            .child(
                Button::new("appearance-settings")
                    .icon(IconName::Settings)
                    .ghost()
                    .size(px(36.))
                    .accessibility_label("Appearance settings")
                    .tooltip("Appearance settings")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.workspace.update(cx, |w, cx| {
                            w.open(
                                Document::new(Page::Theme.id(), "Theme", "Foundation"),
                                OpenMode::Permanent,
                                window,
                                cx,
                            );
                        });
                    })),
            )
            .into_any_element()
    }
    fn title_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let workspace = self.workspace.clone();
        let open_tabs = workspace
            .read(cx)
            .groups(cx)
            .into_iter()
            .flat_map(|g| g.views)
            .filter_map(|id| workspace.read(cx).document(id, cx).map(|d| (id, d)))
            .collect::<Vec<_>>();
        chrome::title_bar(cx)
            .child(chrome::title_content(
                chrome::search_button("Pintle workspace", cx).on_click(
                    cx.listener(|this, _, window, cx| this.open_command_palette(window, cx)),
                ),
                div()
                    .flex()
                    .items_center()
                    .child(
                        Button::new("open-tabs")
                            .icon(IconName::ClockArrowLeft)
                            .ghost()
                            .xsmall()
                            .size(px(24.))
                            .accessibility_label("Open tabs")
                            .tooltip("Open tabs")
                            .dropdown_menu(move |mut menu, _, _| {
                                for (id, document) in &open_tabs {
                                    let id = *id;
                                    let workspace = workspace.clone();
                                    menu =
                                        menu.item(
                                            PopupMenuItem::new(format!(
                                                "{} · {}",
                                                document.title, document.context
                                            ))
                                            .on_click(move |_, window, cx| {
                                                workspace
                                                    .update(cx, |w, cx| w.select(id, window, cx))
                                            }),
                                        );
                                }
                                menu
                            }),
                    )
                    .child(
                        Button::new("toggle-panel")
                            .icon(IconName::PanelBottom)
                            .ghost()
                            .xsmall()
                            .size(px(24.))
                            .accessibility_label("Toggle panel")
                            .tooltip("Toggle panel (⌘J / Ctrl+J)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.inspector.update(cx, |i, cx| i.toggle(window, cx))
                            })),
                    )
                    .child(div().w(px(8.))),
            ))
            .into_any_element()
    }
}
impl Focusable for Workbench {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.workspace.focus_handle(cx)
    }
}
impl Render for Workbench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = chrome::Workbench::new(&self.layout, self.workspace.clone())
            .title(self.title_bar(cx))
            .activity(self.activity_bar(cx))
            .sidebar(chrome::sidebar(
                "explorer-frame",
                "Explorer",
                self.navigation.clone(),
                cx,
            ))
            .panel(self.inspector.clone())
            .status(chrome::status_bar(
                "Pintle · Demo workspace",
                format!(
                    "In-memory edits   ·   {} resources   ·   UTF-8",
                    self.catalog.read(cx).sources.len()
                ),
                cx,
            ));
        div()
            .size_full()
            .on_action(cx.listener(|this, _: &OpenCommandPalette, window, cx| {
                this.open_command_palette(window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.inspector
                    .update(cx, |i, cx| i.toggle_terminal(window, cx))
            }))
            .on_action(cx.listener(|this, _: &NewTerminal, window, cx| {
                this.inspector
                    .update(cx, |i, cx| i.new_terminal(window, cx))
            }))
            .on_action(cx.listener(|this, _: &ToggleInspector, window, cx| {
                this.inspector.update(cx, |i, cx| i.toggle(window, cx))
            }))
            .on_action(
                cx.listener(|this, _: &ToggleSidebar, window, cx| this.toggle_sidebar(window, cx)),
            )
            .child(shell)
    }
}
