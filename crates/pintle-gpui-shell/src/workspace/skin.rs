use super::tab_drop::TabDropState;
use super::*;
use gpui::component::dock::{DragPanel, DropIndicator, TabGroupContext, TabGroupRenderer};
use gpui::component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    tab::{Tab, TabBar},
    tooltip::Tooltip,
};
use gpui::{Axis, Div, Empty, ScrollHandle, Stateful, px, size};
use gpui::{TestSupportExt as _, base::ElementExt as _};
use std::cell::{Cell, RefCell};

pub(super) struct WorkspaceSkin {
    pub base: Rc<DockSkin>,
    pub owner: WeakEntity<Workspace>,
}
impl DockAreaRenderer for WorkspaceSkin {
    fn split_frame(
        &self,
        node: NodeId,
        axis: Axis,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        self.base
            .split_frame(node, axis, window, cx)
            .bg(cx.theme().sidebar)
    }
    fn render_split_handle(
        &self,
        handle: &gpui::base::ResizeHandleContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        pintle_gpui_components::frame_resize_handle_appearance()(handle, window, cx)
    }
    fn tab_group_renderer(&self) -> Rc<dyn TabGroupRenderer> {
        Rc::new(WorkspaceTabs {
            base: self.base.tab_group_renderer(),
            owner: self.owner.clone(),
            scroll: ScrollHandle::default(),
            last_active: Cell::new(None),
            drop_state: Rc::new(RefCell::new(TabDropState::default())),
        })
    }
}
struct WorkspaceTabs {
    base: Rc<dyn TabGroupRenderer>,
    owner: WeakEntity<Workspace>,
    scroll: ScrollHandle,
    last_active: Cell<Option<ViewId>>,
    drop_state: Rc<RefCell<TabDropState>>,
}
fn commands(mut menu: PopupMenu, owner: WeakEntity<Workspace>, id: ViewId) -> PopupMenu {
    let pin = owner.clone();
    menu = menu.item(PopupMenuItem::new("Keep Open").on_click(move |_, _, cx| {
        let _ = pin.update(cx, |w, cx| w.keep_open(id, cx));
    }));
    for (label, direction) in [
        ("Split Right", SplitDirection::Right),
        ("Split Down", SplitDirection::Down),
    ] {
        let owner = owner.clone();
        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
            let _ = owner.update(cx, |w, cx| w.split(id, direction, window, cx));
        }));
    }
    menu = menu.separator();
    for (label, scope) in [
        ("Close", CloseScope::One),
        ("Close Others", CloseScope::Others),
        ("Close to the Right", CloseScope::ToRight),
        ("Close All", CloseScope::All),
    ] {
        let owner = owner.clone();
        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
            let _ = owner.update(cx, |w, cx| w.close(id, scope, window, cx));
        }));
    }
    menu
}
impl TabGroupRenderer for WorkspaceTabs {
    fn frame(&self, group: &TabGroupContext, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        let node = group.node();
        let owner = self.owner.clone();
        self.base
            .frame(group, window, cx)
            .border_1()
            .border_color(gpui::transparent_black())
            .rounded(px(8.))
            .relative()
            .overflow_hidden()
            .child(pintle_gpui_components::rounded_frame_overlay(
                px(8.),
                cx.theme().sidebar,
                cx.theme().border,
            ))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = owner.update(cx, |w, cx| w.activate_group(node, cx));
            })
    }
    fn content_frame(&self, g: &TabGroupContext, w: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.base.content_frame(g, w, cx).bg(cx.theme().tab_active)
    }
    fn render_active_panel(
        &self,
        panel: AnyView,
        group: &TabGroupContext,
        _: &mut Window,
        _: &mut App,
    ) -> AnyElement {
        if group.is_collapsed() {
            return Empty.into_any_element();
        }
        // Kit 0.8.1's styled dock caches the complete panel scene. Native text
        // can replay at stale positions when changing documents. Keep the view
        // retained, but draw it normally; tables still virtualize their rows.
        // Scrolling belongs to each document, not an outer dock scroll region.
        div()
            .id("tab-content")
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(panel)
            .into_any_element()
    }
    fn render_drop_indicator(
        &self,
        i: DropIndicator,
        w: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.base.render_drop_indicator(i, w, cx)
    }
    fn render_tab_bar(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let Some(workspace) = self.owner.upgrade() else {
            return Empty.into_any_element();
        };
        let active = group.active_panel().map(|p| ViewId(p.panel_id(cx)));
        if self.last_active.replace(active) != active {
            self.scroll.scroll_to_item(group.active_ix());
        }
        *self.drop_state.borrow_mut() = TabDropState {
            tabs: group
                .panels()
                .iter()
                .map(|p| (p.panel_id(cx), None))
                .collect(),
            ..Default::default()
        };
        let tabs = group
            .panels()
            .iter()
            .enumerate()
            .map(|(ix, panel)| {
                let id = ViewId(panel.panel_id(cx));
                let (document, preview) = {
                    let w = workspace.read(cx);
                    (w.document(id, cx), w.is_preview(id, cx))
                };
                let icon = document.as_ref().and_then(|d| d.icon_path.clone());
                let tooltip = document
                    .as_ref()
                    .map(|d| {
                        if d.context.is_empty() {
                            d.title.to_string()
                        } else {
                            format!("{} / {}", d.context, d.title)
                        }
                    })
                    .unwrap_or_else(|| "Document".into());
                let label = document.map(|d| d.title).unwrap_or("Document".into());
                let close_focus = window
                    .use_keyed_state(("close-tab-focus", id.0.as_u64()), cx, |_, cx| {
                        cx.focus_handle()
                    })
                    .read(cx)
                    .clone();
                let click = self.owner.clone();
                let close = self.owner.clone();

                let pin = self.owner.clone();
                let menu = self.owner.clone();
                let title = div()
                    .id(("workspace-tab-title", id.0.as_u64()))
                    .test_support()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .h_full()
                    .flex()
                    .items_center()
                    .when(preview, |d| d.italic())
                    .child(
                        div()
                            .min_w_0()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(label.clone()),
                    )
                    .context_menu(move |m, _, _| commands(m, menu.clone(), id));
                Tab::new()
                    .aria_label(label.clone())
                    .debug_selector(move || format!("workspace-tab-{}", id.0.as_u64()))
                    .tooltip(move |w, cx| Tooltip::new(tooltip.clone()).build(w, cx))
                    .min_w(px(96.))
                    .text_size(px(13.))
                    .prefix(
                        Icon::new(IconName::File)
                            .when_some(icon, |icon, path| icon.path(path))
                            .size(px(14.)),
                    )
                    .child(title)
                    .on_click(move |e, w, cx| {
                        let _ = click.update(cx, |s, cx| {
                            if e.click_count() == 2 {
                                s.keep_open(id, cx);
                            }
                            s.select(id, w, cx);
                        });
                    })
                    .suffix(
                        Button::new(("close-view", id.0.as_u64()))
                            .icon(IconName::Close)
                            .xsmall()
                            .size(px(18.))
                            .ghost()
                            .flex_shrink_0()
                            .track_focus(&close_focus)
                            .accessibility_label(format!("Close {label}"))
                            .on_click(move |_, w, cx| {
                                cx.stop_propagation();
                                let _ =
                                    close.update(cx, |s, cx| s.close(id, CloseScope::One, w, cx));
                            }),
                    )
                    .when_some(
                        group.drag_panel(ix, cx).filter(|_| group.is_draggable()),
                        |tab, drag| {
                            tab.on_drag(drag, move |drag, offset, window, cx| {
                                let _ = pin.update(cx, |s, cx| {
                                    s.keep_open(id, cx);
                                    window.focus(&s.focus, cx);
                                });
                                cx.stop_propagation();
                                drag.set_drag_offset(offset);
                                drag.set_preview_size(size(px(160.), px(36.)));
                                cx.new(|_| DragLabel(label.clone()))
                            })
                        },
                    )
            })
            .collect::<Vec<_>>();
        let drop = group.clone();
        let viewport_state = self.drop_state.clone();
        let scroll = self.scroll.clone();
        let hover_state = self.drop_state.clone();
        let drop_state = self.drop_state.clone();
        let paint_state = self.drop_state.clone();
        let node = group.node();
        let droppable = group.is_droppable() && !group.is_locked();
        let owner = self.owner.clone();
        let split = self.owner.clone();
        let active = group.active_panel().map(|p| ViewId(p.panel_id(cx)));
        div()
            .id(("workspace-tab-strip", group.node().as_u64()))
            .relative()
            .h(px(36.))
            .flex_shrink_0()
            .flex()
            .min_w_0()
            .bg(cx.theme().tab_bar)
            .child(
                div()
                    .id(("workspace-tabs", group.node().as_u64()))
                    .test_support()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .relative()
                    .child(
                        TabBar::new(("workspace-folder-tabs", group.node().as_u64()))
                            .folder()
                            .h_full()
                            .selected_index(group.active_ix())
                            .max_width(px(220.))
                            .track_scroll(&self.scroll)
                            .children(tabs),
                    )
                    // Kit tracks logical folder-tab wrappers with unscrolled
                    // bounds. Translate once into window coordinates for drops.
                    .on_prepaint(move |_, _, _| {
                        let mut state = viewport_state.borrow_mut();
                        state.viewport = Some(scroll.bounds());
                        for (ix, (_, bounds)) in state.tabs.iter_mut().enumerate() {
                            *bounds = scroll
                                .bounds_for_item(ix)
                                .map(|bounds| bounds + scroll.offset());
                        }
                    })
                    .when(droppable, |bar| {
                        bar.drag_over::<DragPanel>(move |style, drag, _, _| {
                            hover_state.borrow_mut().drag = Some(drag.clone());
                            style
                        })
                        .on_drop(move |drag: &DragPanel, window, cx| {
                            let target =
                                drop_state
                                    .borrow()
                                    .target(node, drag, window.mouse_position());
                            drop_state.borrow_mut().drag = None;
                            cx.stop_propagation();
                            if let Some(target) = target {
                                // Kit's slot is measured after source detachment.
                                drop.drop_panel(drag.clone(), Some(target.index), true, window, cx);
                            }
                        })
                    }),
            )
            .when_some(active, |bar, id| {
                bar.child(
                    Button::new("workspace-split-right")
                        .icon(IconName::PanelRight)
                        .small()
                        .size(px(28.))
                        .ghost()
                        .tooltip("Split Right")
                        .accessibility_label("Split Right")
                        .on_click(move |_, window, cx| {
                            let _ = split.update(cx, |workspace, cx| {
                                workspace.split(id, SplitDirection::Right, window, cx)
                            });
                        }),
                )
                .child(
                    Button::new("workspace-commands")
                        .icon(IconName::Ellipsis)
                        .small()
                        .size(px(32.))
                        .flex_shrink_0()
                        .ghost()
                        .accessibility_label("Tab commands")
                        .dropdown_menu(move |m, _, _| commands(m, owner.clone(), id)),
                )
            })
            .child(
                gpui::canvas(
                    |_, _, _| (),
                    move |_, _, window, cx| {
                        if !cx.has_active_drag() {
                            return;
                        }
                        let state = paint_state.borrow();
                        let Some(drag) = &state.drag else {
                            return;
                        };
                        let Some(target) = state.target(node, drag, window.mouse_position()) else {
                            return;
                        };
                        let Some(viewport) = state.viewport else {
                            return;
                        };
                        if viewport.size.width < px(3.) || viewport.size.height <= px(4.) {
                            return;
                        }
                        let x = target
                            .edge
                            .max(viewport.left())
                            .min(viewport.right() - px(3.));
                        let marker = gpui::Bounds::new(
                            gpui::point(x, viewport.top() + px(2.)),
                            size(px(3.), viewport.size.height - px(4.)),
                        );
                        window.paint_quad(gpui::fill(marker, cx.theme().ring));
                    },
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }
}
struct DragLabel(SharedString);
impl Render for DragLabel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            // Leave the insertion caret visible above the floating label.
            .mt(px(36.))
            .px_3()
            .h(px(36.))
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .child(self.0.clone())
    }
}
