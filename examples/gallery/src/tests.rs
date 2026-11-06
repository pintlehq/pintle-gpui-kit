use super::{Gallery, Page};
use gpui::InputEvent as _;
use gpui::component::{ActiveTheme, WindowExt};
use gpui::test::{TestQueryExt, TestWindowExt};
use gpui::{AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions, px, size};
use pintle_gpui_kit::{kit as gpui, theme};

mod context_menus;
mod performance;
mod resources;

/// Native Kit tabs use positional IDs; document titles retain their view IDs.
fn workspace_tab(
    window: &gpui::Window,
    id: pintle_gpui_kit::shell::workspace::ViewId,
) -> gpui::test::ElementSnapshot {
    let title = window.find(("workspace-tab-title", id.0.as_u64()));
    window
        .find_all_by_role(gpui::Role::Tab)
        .into_iter()
        .find(|tab| title.path().starts_with(tab.path()))
        .expect("document title belongs to a native folder tab")
}

fn open(cx: &mut TestAppContext) -> (gpui::AnyWindowHandle, Entity<Gallery>) {
    cx.update(|cx| {
        super::initialize(cx);
        theme::set_mode(theme::ThemeMode::Light, cx);
        gpui::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(1100.), px(760.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Gallery::new(window, cx)),
        )
        .unwrap()
    })
}

#[gpui_kit::test]
fn navigation_input_save_and_dialog_focus(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("page-title").label(), Some("Overview"));
        window.click("nav-components", cx);
        window.click("display-name", cx);
        window.input("Ada José", cx);
        window.press("backspace", cx);
        assert_eq!(window.find("display-name").value(), Some("Ada Jos"));
        assert_eq!(window.find("display-name").focused(), Some(true));
        window.press("tab", cx);
        assert_eq!(window.find("save-name").focused(), Some(true));
        window.press("shift-tab", cx);
        assert_eq!(window.find("display-name").focused(), Some(true));
        // Open the dialog while the input owns focus; Kit restores that focus.
        let focus_before = window.focused(cx);
        window.click("save-name", cx);
        assert!(window.has_active_dialog(cx));
        assert_eq!(view.read(cx).saved_name.as_deref(), Some("Ada Jos"));
        window.press("escape", cx);
        assert!(!window.has_active_dialog(cx));
        assert_eq!(window.focused(cx), focus_before);
        window.click("nav-overview", cx);
        window.click("nav-components", cx);
        assert_eq!(window.find("display-name").value(), Some("Ada Jos"));
        assert_eq!(window.find("save-status").label(), Some("Saved: Ada Jos"));
        window.click("save-name", cx);
        window.click("dismiss-save", cx);
        assert!(!window.has_active_dialog(cx));
    })
    .unwrap();
}

#[gpui_kit::test]
fn counter_tabs_and_theme_keep_application_state(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav-components", cx);
        window.click("increment", cx);
        window.click("increment", cx);
        assert_eq!(view.read(cx).count, 2);
        window.within("demo-tabs").click(1usize, cx);
        assert_eq!(window.find("tab-content").label(), Some("Details content"));
        window.click("mode-dark", cx);
        assert_eq!(theme::Theme::global(cx).mode, theme::ThemeMode::Dark);
        assert_eq!(view.read(cx).count, 2);
        assert_eq!(view.read(cx).tab, 1);
        window.click("reset-count", cx);
        assert_eq!(view.read(cx).count, 0);
        assert_eq!(window.find("reset-count").disabled(), Some(true));
        window.click("nav-theme", cx);
        window.click("toggle-brand", cx);
        let branded = theme::Theme::global(cx).primary;
        window.click("mode-light", cx);
        window.click("mode-dark", cx);
        assert_eq!(theme::Theme::global(cx).primary, branded);
        window.click("toggle-brand", cx);
        assert_eq!(theme::Theme::global(cx).primary, gpui::rgb(0x0078d4).into());
    })
    .unwrap();
}

#[gpui_kit::test]
fn optional_shell_regions_reclaim_layout_space(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav-shell", cx);
        assert_eq!(view.read(cx).page, Page::Shell);
        let original_width = window.find("shell-content").bounds().size.width;
        for (toggle, region) in [
            ("toggle-sidebar", "shell-sidebar"),
            ("toggle-header", "shell-header"),
            ("toggle-toolbar", "shell-toolbar"),
            ("toggle-status", "shell-status"),
        ] {
            assert!(window.try_find(region).is_some());
            window.click(toggle, cx);
            assert!(window.try_find(region).is_none());
        }
        assert!(window.find("shell-content").bounds().size.width > original_width);
    })
    .unwrap();
    cx.simulate_window_resize(handle, size(px(800.), px(600.)));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("shell-content").bounds().size.width > px(0.));
        assert!(window.find("mode-dark").visible());
        assert!(window.find("toggle-sidebar").visible());
        window.click("toggle-sidebar", cx);
        assert!(window.find("shell-sidebar").visible());
    })
    .unwrap();
}

use super::{
    data::{Dataset, ResourceView},
    workbench::Workbench,
};
use pintle_gpui_kit::shell::workspace::{CloseScope, OpenMode, SplitDirection, ViewId};
fn workbench(cx: &mut TestAppContext) -> (gpui::AnyWindowHandle, Entity<Workbench>) {
    cx.update(|cx| {
        super::initialize(cx);
        theme::set_mode(theme::ThemeMode::Light, cx);
        gpui::open_window(super::appearance::window_options(cx), cx, |window, cx| {
            cx.new(|cx| Workbench::new(window, cx))
        })
        .unwrap()
    })
}
fn resource(
    workspace: &Entity<pintle_gpui_kit::shell::workspace::Workspace>,
    id: ViewId,
    cx: &gpui::App,
) -> Entity<ResourceView> {
    workspace
        .read(cx)
        .page(id, cx)
        .unwrap()
        .downcast::<ResourceView>()
        .unwrap()
}

#[gpui_kit::test]
fn workspace_preview_split_move_and_close(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let workspace = view.read(cx).workspace.clone();
        let datasets = Dataset::fixtures();
        assert_eq!(workspace.read(cx).groups(cx).len(), 1);
        let initial = workspace.read(cx).active_view(cx).unwrap();
        assert!(!workspace.read(cx).is_preview(initial, cx));
        let preview = workspace.update(cx, |w, cx| {
            w.open(datasets[1].document.clone(), OpenMode::Preview, window, cx)
        });
        let replacement = workspace.update(cx, |w, cx| {
            w.open(datasets[2].document.clone(), OpenMode::Preview, window, cx)
        });
        assert!(workspace.read(cx).document(preview, cx).is_none());
        assert_eq!(workspace.read(cx).groups(cx)[0].views.len(), 2);
        let same = workspace.update(cx, |w, cx| {
            w.open(
                datasets[2].document.clone(),
                OpenMode::Permanent,
                window,
                cx,
            )
        });
        assert_eq!(same, replacement);
        assert!(!workspace.read(cx).is_preview(same, cx));
        let split = workspace
            .update(cx, |w, cx| {
                w.split(initial, SplitDirection::Right, window, cx)
            })
            .unwrap();
        assert_eq!(workspace.read(cx).groups(cx).len(), 2);
        let original = resource(&workspace, initial, cx);
        let duplicate = resource(&workspace, split, cx);
        assert_ne!(original.entity_id(), duplicate.entity_id());
        workspace.update(cx, |w, cx| w.select(initial, window, cx));
        window.render_frame(cx);
        window
            .within(("resource-view", original.entity_id()))
            .click("row-filter", cx);
        window.input("สมชาย", cx);
        assert_eq!(original.read(cx).filter.read(cx).value().as_ref(), "สมชาย");
        assert_eq!(
            duplicate.read(cx).table.read(cx).delegate().indices.len(),
            1000
        );
        let group = workspace
            .read(cx)
            .groups(cx)
            .iter()
            .find(|g| g.views.contains(&initial))
            .unwrap()
            .id;
        workspace.update(cx, |w, cx| w.move_view(split, group, Some(0), window, cx));
        assert_eq!(workspace.read(cx).groups(cx).len(), 1);
        assert_eq!(
            resource(&workspace, split, cx).entity_id(),
            duplicate.entity_id()
        );
        assert!(
            workspace.read(cx).document(split, cx).is_some(),
            "before ToRight"
        );
        workspace.update(cx, |w, cx| w.close(split, CloseScope::ToRight, window, cx));
        assert_eq!(workspace.read(cx).groups(cx)[0].views, vec![split]);
        assert!(
            workspace.read(cx).document(split, cx).is_some(),
            "split registry before close all"
        );
        workspace.update(cx, |w, cx| w.close(split, CloseScope::All, window, cx));
        assert!(workspace.read(cx).groups(cx).is_empty());
        window.render_frame(cx);
        workspace.update(cx, |w, cx| {
            w.open(datasets[3].document.clone(), OpenMode::Preview, window, cx);
        });
        window.render_frame(cx);
        assert_eq!(workspace.read(cx).groups(cx).len(), 1);
    })
    .unwrap();
}

#[gpui_kit::test]
fn tree_search_keyboard_and_preview_interactions(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    let workspace = cx.update(|cx| view.read(cx).workspace.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav:db/local/shop/orders", cx);
    })
    .unwrap();
    let orders = cx.update(|cx| workspace.read(cx).active_view(cx).unwrap());
    cx.update(|cx| assert!(workspace.read(cx).is_preview(orders, cx)));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.double_click("nav:k8s/development/pods", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let w = workspace.read(cx);
        assert!(w.document(orders, cx).is_none());
        let id = w.active_view(cx).unwrap();
        assert!(!w.is_preview(id, cx));
        assert_eq!(w.document(id, cx).unwrap().title.as_ref(), "Pods");
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav:local", cx);
        window.click("resource-search", cx);
        window.input("customers", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("nav:db/local/shop/customers").visible());
        assert!(window.try_find("nav:db/local/shop/orders").is_none());
        window.click("resource-search", cx);
        window.press("cmd-a", cx);
        window.press("backspace", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("nav:db/local/shop/customers").is_none());
        window.click("nav:local", cx);
        window.press("left", cx);
        window.press("right", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let w = workspace.read(cx);
        let id = w.active_view(cx).unwrap();
        assert_eq!(w.document(id, cx).unwrap().title.as_ref(), "orders");
        assert!(!w.is_preview(id, cx));
    });
}

#[gpui_kit::test]
fn tables_keep_isolated_filter_sort_selection_scroll_through_moves(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    let workspace = cx.update(|cx| view.read(cx).workspace.clone());
    let (first, other, original, duplicate) = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let first = workspace.read(cx).active_view(cx).unwrap();
            let other = workspace
                .update(cx, |w, cx| w.split(first, SplitDirection::Down, window, cx))
                .unwrap();
            let original = resource(&workspace, first, cx);
            let duplicate = resource(&workspace, other, cx);
            workspace.update(cx, |w, cx| w.select(first, window, cx));
            window.render_frame(cx);
            window
                .within(("resource-view", original.entity_id()))
                .click("row-filter", cx);
            window.input("สมชาย", cx);
            (first, other, original, duplicate)
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let table = original.read(cx).table.clone();
        assert_eq!(table.read(cx).delegate().indices.len(), 250);
        assert_eq!(
            duplicate.read(cx).table.read(cx).delegate().indices.len(),
            1000
        );
        let scope = ("resource-view", original.entity_id());
        window.within(scope).click(("icon-sort", 0usize), cx);
        assert_eq!(
            table.read(cx).delegate().sort,
            Some((0, gpui::component::table::ColumnSort::Descending))
        );
        table.update(cx, |t, cx| t.scroll_to_row(100, cx));
        window.render_frame(cx);
        window.within(scope).click("table-cell-100:1", cx);
        let before = table.read(cx).visible_range().rows().clone();
        assert!(before.start > 0);
        assert_eq!(
            original.read(cx).editable.read(cx).active_position(cx),
            Some((100, 1))
        );
        let target = workspace
            .read(cx)
            .groups(cx)
            .into_iter()
            .find(|g| g.views.contains(&other))
            .unwrap()
            .id;
        workspace.update(cx, |w, cx| w.move_view(first, target, None, window, cx));
        window.render_frame(cx);
        assert_eq!(
            resource(&workspace, first, cx).entity_id(),
            original.entity_id()
        );
        assert_eq!(
            table.read(cx).delegate().sort,
            Some((0, gpui::component::table::ColumnSort::Descending))
        );
        assert_eq!(
            original.read(cx).editable.read(cx).active_position(cx),
            Some((100, 1))
        );
        assert_eq!(table.read(cx).visible_range().rows().start, before.start);
        assert_eq!(
            window.within(scope).find("row-filter").value(),
            Some("สมชาย")
        );
        assert_eq!(duplicate.read(cx).editable.read(cx).active_cell(), None);
    })
    .unwrap();
}

#[gpui_kit::test]
fn rendered_tab_drag_reorder_move_and_keyboard_close(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    let workspace = cx.update(|cx| view.read(cx).workspace.clone());
    let (first, second, third) = cx
        .update_window(handle, |_, window, cx| {
            let first = workspace.read(cx).active_view(cx).unwrap();
            let fixtures = Dataset::fixtures();
            let second = workspace.update(cx, |w, cx| {
                w.open(
                    fixtures[1].document.clone(),
                    OpenMode::Permanent,
                    window,
                    cx,
                )
            });
            let third = workspace.update(cx, |w, cx| {
                w.open(fixtures[2].document.clone(), OpenMode::Preview, window, cx)
            });
            window.render_frame(cx);
            let from = window
                .find(("workspace-tab-title", third.0.as_u64()))
                .bounds()
                .center();
            let target = workspace_tab(window, first).bounds();
            let before = gpui::point(target.left() + px(5.), target.center().y);
            begin_drag(window, from, before, cx);
            finish_drag(window, before, cx);
            (first, second, third)
        })
        .unwrap();
    cx.update(|cx| {
        let w = workspace.read(cx);
        assert_eq!(w.groups(cx)[0].views, vec![third, first, second]);
        assert!(!w.is_preview(third, cx));
    });
    let split = cx
        .update_window(handle, |_, window, cx| {
            let split = workspace
                .update(cx, |w, cx| {
                    w.split(second, SplitDirection::Right, window, cx)
                })
                .unwrap();
            window.render_frame(cx);
            window.drag_to(
                ("workspace-tab-title", third.0.as_u64()),
                ("workspace-tab-title", split.0.as_u64()),
                cx,
            );
            split
        })
        .unwrap();
    cx.update(|cx| {
        let groups = workspace.read(cx).groups(cx);
        let target = groups.iter().find(|g| g.views.contains(&split)).unwrap();
        assert!(target.views.contains(&third));
        assert_eq!(target.views.len(), 2);
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        workspace.update(cx, |w, cx| w.select(third, window, cx));
        window.render_frame(cx);
        window.press("cmd-w", cx);
    })
    .unwrap();
    cx.update(|cx| assert!(workspace.read(cx).document(third, cx).is_none()));
}

#[gpui_kit::test]
fn inactive_tab_context_menu_targets_its_own_group(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    let workspace = cx.update(|cx| view.read(cx).workspace.clone());
    let (first, second, split) = cx
        .update_window(handle, |_, window, cx| {
            let first = workspace.read(cx).active_view(cx).unwrap();
            let fixtures = Dataset::fixtures();
            let second = workspace.update(cx, |w, cx| {
                w.open(
                    fixtures[1].document.clone(),
                    OpenMode::Permanent,
                    window,
                    cx,
                )
            });
            let split = workspace
                .update(cx, |w, cx| {
                    w.split(second, SplitDirection::Down, window, cx)
                })
                .unwrap();
            window.render_frame(cx);
            window.right_click(("workspace-tab-title", first.0.as_u64()), cx);
            (first, second, split)
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for _ in 0..5 {
            window.press("down", cx);
        }
        window.press("enter", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let w = workspace.read(cx);
        assert!(w.document(first, cx).is_some());
        assert!(w.document(second, cx).is_none());
        assert!(w.document(split, cx).is_some());
        assert_eq!(w.groups(cx).len(), 2);
    });
}

#[gpui_kit::test]
fn tables_at_both_sizes_and_modes_and_sidebar_resize(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    let workspace = cx.update(|cx| view.read(cx).workspace.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let edge = window
            .within("workspace-sidebar")
            .find_all(("resizable-handle", 0usize))
            .into_iter()
            .min_by_key(|handle| handle.bounds().origin.x.as_f32() as i32)
            .unwrap()
            .bounds()
            .center();
        let width = window.find("resource-search").bounds().size.width;
        window.drag(edge, edge + gpui::point(px(80.), px(0.)), cx);
        assert!(
            (window.find("resource-search").bounds().size.width - width - px(80.)).abs() < px(3.)
        );
        let edge = window
            .within("workspace-sidebar")
            .find_all(("resizable-handle", 0usize))
            .into_iter()
            .min_by_key(|handle| handle.bounds().origin.x.as_f32() as i32)
            .unwrap()
            .bounds()
            .center();
        window.drag(edge, edge + gpui::point(px(700.), px(0.)), cx);
        assert!(window.find("resource-search").bounds().size.width <= px(480.));
    })
    .unwrap();
    for (width, height) in [(1440., 900.), (1024., 680.)] {
        cx.simulate_window_resize(handle, size(px(width), px(height)));
        for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
            for fixture in Dataset::fixtures() {
                cx.update_window(handle, |_, window, cx| {
                    assert_eq!(window.viewport_size(), size(px(width), px(height)));
                    theme::set_mode(mode, cx);
                    let id = workspace.update(cx, |w, cx| {
                        w.open(fixture.document.clone(), OpenMode::Preview, window, cx)
                    });
                    window.render_frame(cx);
                    let resource = resource(&workspace, id, cx);
                    let range = resource.read(cx).table.read(cx).visible_range().rows();
                    assert!(range.end - range.start < 60, "table virtualizes 1000 rows");
                    let scope = window.within(("resource-view", resource.entity_id()));
                    let filter = scope.find("row-filter");
                    let table = scope.find("table-inner");
                    assert!(filter.visible());
                    let editor_height = window
                        .find(("resource-view", resource.entity_id()))
                        .bounds()
                        .size
                        .height;
                    assert!(editor_height >= px(200.));
                    assert!(
                        (table.bounds().size.height - (editor_height - px(54.))).abs() < px(3.)
                    );
                    assert!(window.find("inspector-panel").bounds().size.height >= px(120.));
                    // At minimum width with a 480 px sidebar, the Lightship content
                    // insets reserve 40 px plus borders; the table still has ~490 px.
                    assert!(table.bounds().size.width > px(470.));
                    assert!(
                        table.bounds().bottom() <= window.find("inspector-panel").bounds().top()
                    );
                    assert!(filter.bounds().right() <= px(width));
                })
                .unwrap();
            }
        }
    }
}

#[gpui_kit::test]
fn tab_pinning_keyboard_cycle_adjacent_close_and_split_resize(cx: &mut TestAppContext) {
    use gpui::component::dock::{DockPlacement, PaneRef};
    let (handle, view) = workbench(cx);
    let workspace = cx.update(|cx| view.read(cx).workspace.clone());
    let (first, second, third) = cx
        .update_window(handle, |_, window, cx| {
            let first = workspace.read(cx).active_view(cx).unwrap();
            let data = Dataset::fixtures();
            let second = workspace.update(cx, |w, cx| {
                w.open(data[1].document.clone(), OpenMode::Permanent, window, cx)
            });
            let third = workspace.update(cx, |w, cx| {
                w.open(data[2].document.clone(), OpenMode::Preview, window, cx)
            });
            window.render_frame(cx);
            window.double_click(("workspace-tab-title", third.0.as_u64()), cx);
            assert!(!workspace.read(cx).is_preview(third, cx));
            window.press("ctrl-pageup", cx);
            assert_eq!(workspace.read(cx).active_view(cx), Some(second));
            window.press("cmd-w", cx);
            assert_eq!(workspace.read(cx).active_view(cx), Some(third));
            window.press("cmd-w", cx);
            assert_eq!(workspace.read(cx).active_view(cx), Some(first));
            (first, second, third)
        })
        .unwrap();
    cx.update(|cx| {
        assert!(workspace.read(cx).document(second, cx).is_none());
        assert!(workspace.read(cx).document(third, cx).is_none());
    });
    cx.update_window(handle, |_, window, cx| {
        let original = resource(&workspace, first, cx);
        workspace.update(cx, |w, cx| {
            w.split(first, SplitDirection::Down, window, cx);
        });
        window.render_frame(cx);
        let mut split = None;
        workspace
            .read(cx)
            .dock()
            .read(cx)
            .layout(DockPlacement::Center)
            .unwrap()
            .root()
            .walk(&mut |node| {
                if let PaneRef::Split { children, .. } = node.kind()
                    && children.len() == 2
                {
                    split = Some(node.id());
                }
            });
        let split = split.unwrap();
        let handle = window
            .within(("dock-split", split.as_u64()))
            .find(("resizable-handle", 0usize))
            .bounds()
            .center();
        let before = window
            .within(("resource-view", original.entity_id()))
            .find("table-inner")
            .bounds()
            .size
            .height;
        window.drag(handle, handle + gpui::point(px(0.), px(70.)), cx);
        let after = window
            .within(("resource-view", original.entity_id()))
            .find("table-inner")
            .bounds()
            .size
            .height;
        assert!((after - before - px(70.)).abs() < px(4.));
        // Resize a real header edge through native pointer dispatch.
        let header = window
            .within(("resource-view", original.entity_id()))
            .find(("col-header", 0usize))
            .bounds();
        let edge = gpui::point(header.right() - px(1.), header.center().y);
        let table = original.read(cx).table.clone();
        let width = table.read(cx).col_width(0).unwrap();
        window.drag(edge, edge + gpui::point(px(55.), px(0.)), cx);
        assert!(table.read(cx).col_width(0).unwrap() > width + px(40.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn frameless_titlebar_search_theme_and_open_tabs(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    cx.update(|cx| {
        let options = super::appearance::window_options(cx);
        assert!(options.app_owns_titlebar_drag);
        assert!(options.titlebar.as_ref().unwrap().appears_transparent);
        assert!(options.titlebar.as_ref().unwrap().title.is_none());
        assert_eq!(options.window_min_size, Some(size(px(1024.), px(680.))));
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(cx.theme().font_family.as_ref(), ".SystemUIFont");
        assert_eq!(cx.theme().background, gpui::rgb(0xffffff).into());
        assert_eq!(cx.theme().sidebar, gpui::rgb(0xfafafd).into());
        window.click("command-search", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert_eq!(
            window
                .find_by_label("Search resources and commands...")
                .focused(),
            Some(true)
        );
        window.input("pods", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.press("enter", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        let workspace = view.read(cx).workspace.clone();
        let active = workspace.read(cx).active_view(cx).unwrap();
        assert_eq!(
            workspace
                .read(cx)
                .document(active, cx)
                .unwrap()
                .key
                .0
                .as_ref(),
            "k8s/development/pods"
        );
        assert!(!workspace.read(cx).is_preview(active, cx));
        window.press("cmd-k", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window
                .find_by_label("Search resources and commands...")
                .value(),
            Some("")
        );
        window.press("escape", cx);
        window.click("toggle-appearance", cx);
        assert_eq!(cx.theme().mode, theme::ThemeMode::Dark);
        assert_eq!(cx.theme().background, gpui::rgb(0x121314).into());
        assert_eq!(cx.theme().sidebar, gpui::rgb(0x191a1b).into());
        assert_eq!(cx.theme().font_family.as_ref(), ".SystemUIFont");
        window.click("toggle-appearance", cx);
        window.click("appearance-settings", cx);
        let workspace = view.read(cx).workspace.clone();
        let active = workspace.read(cx).active_view(cx).unwrap();
        assert_eq!(
            workspace
                .read(cx)
                .document(active, cx)
                .unwrap()
                .key
                .0
                .as_ref(),
            Page::Theme.id()
        );
        window.click("open-tabs", cx);
        // The top-right history control lists actual open views, including previews.
        assert!(
            window
                .find_by_label("customers · Local / shop / public")
                .visible()
        );
        window.press("escape", cx);
    })
    .unwrap();
    for dimensions in [size(px(1024.), px(680.)), size(px(1440.), px(900.))] {
        cx.simulate_window_resize(handle, dimensions);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.viewport_size(), dimensions);
            let search = window.find("command-search").bounds();
            assert!(
                (search.center().x - dimensions.width / 2.).abs() < px(3.),
                "search={search:?}, window={dimensions:?}"
            );
            assert!(window.find("toggle-appearance").visible());
            assert!(window.find("appearance-settings").visible());
        })
        .unwrap();
    }
}

// Separate UI turns let Kit deliver deferred Input/Command callbacks and modal
// focus changes before the next native gesture, just as the application does.
fn palette_step(
    cx: &mut TestAppContext,
    handle: gpui::AnyWindowHandle,
    step: impl FnOnce(&mut gpui::Window, &mut gpui::App),
) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        step(window, cx);
    })
    .unwrap();
}

const PALETTE_INPUT: &str = "Search resources and commands...";

#[gpui_kit::test]
fn command_palette_lifecycle_focus_and_theme_matrix(cx: &mut TestAppContext) {
    let (handle, _) = workbench(cx);
    for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
        cx.update(|cx| theme::set_mode(mode, cx));
        for dimensions in [size(px(1024.), px(680.)), size(px(1440.), px(900.))] {
            cx.simulate_window_resize(handle, dimensions);
            let mut previous_focus = None;
            palette_step(cx, handle, |window, cx| {
                window.click("resource-search", cx);
                window.press("cmd-a", cx);
                window.input("shop", cx);
                previous_focus = window.focused(cx);
                window.click("command-search", cx);
            });
            palette_step(cx, handle, |window, cx| {
                assert!(window.has_active_dialog(cx));
                assert_eq!(window.find_by_label(PALETTE_INPUT).focused(), Some(true));
                let palette = window.find("workspace-command-palette").bounds();
                assert!(palette.size.width <= px(560.) && palette.size.width >= px(550.));
                assert!((palette.center().x - dimensions.width / 2.).abs() < px(2.));
                assert!(
                    (palette.center().y - dimensions.height / 2.).abs() < px(2.),
                    "palette={palette:?}, viewport={dimensions:?}"
                );
                // Includes the capped list, search field and keyboard footer.
                assert!(palette.size.height <= px(380.));
                assert!(window.find("command-hints").visible());
                window.press("cmd-k", cx);
                window.press("ctrl-k", cx);
                assert_eq!(window.find_all("workspace-command-palette").len(), 1);
                window.input("no-such-resource-xyz", cx);
            });
            palette_step(cx, handle, |window, cx| {
                assert!(window.find("command-empty").visible());
                window.press("enter", cx);
                assert!(window.has_active_dialog(cx));
                window.press("escape", cx);
            });
            palette_step(cx, handle, |window, cx| {
                assert!(window.has_active_dialog(cx));
                assert_eq!(window.find_by_label(PALETTE_INPUT).value(), Some(""));
                assert_eq!(window.find("resource-search").value(), Some("shop"));
                window.press("escape", cx);
            });
            palette_step(cx, handle, |window, cx| {
                assert!(!window.has_active_dialog(cx));
                assert_eq!(window.focused(cx), previous_focus);
                window.press("ctrl-k", cx);
            });
            palette_step(cx, handle, |window, cx| {
                assert_eq!(window.find_by_label(PALETTE_INPUT).focused(), Some(true));
                window.input("pods", cx);
            });
            palette_step(cx, handle, |window, cx| {
                // Below the title bar, well outside the centered surface.
                window.click_at("dialog", gpui::point(px(8.), px(60.)), cx);
            });
            palette_step(cx, handle, |window, cx| {
                assert!(!window.has_active_dialog(cx));
                assert_eq!(window.focused(cx), previous_focus);
                window.press("cmd-k", cx);
            });
            palette_step(cx, handle, |window, cx| {
                assert_eq!(window.find_by_label(PALETTE_INPUT).value(), Some(""));
                window.press("escape", cx);
            });
        }
    }
}

#[gpui_kit::test]
fn command_palette_filtered_resources_keyboard_pointer_and_retained_tabs(cx: &mut TestAppContext) {
    use gpui::component::IndexPath;
    let (handle, workbench) = workbench(cx);
    let (workspace, first, original) = cx.update(|cx| {
        let workspace = workbench.read(cx).workspace.clone();
        let first = workspace.read(cx).active_view(cx).unwrap();
        let original = resource(&workspace, first, cx);
        (workspace, first, original)
    });
    palette_step(cx, handle, |window, cx| {
        window.click("editable-cell:2:1", cx);
        window.press("cmd-k", cx);
    });
    palette_step(cx, handle, |window, cx| window.input("PUBLIC", cx));
    palette_step(cx, handle, |window, cx| {
        assert_eq!(window.find_all_by_role(gpui::Role::ListBoxOption).len(), 3);
        window.press("up", cx); // Wrap from customers to products.
        window.press("down", cx); // customers
        window.press("down", cx); // orders
        assert_eq!(workspace.read(cx).active_view(cx), Some(first));
        window.press("enter", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(!window.has_active_dialog(cx));
        let active = workspace.read(cx).active_view(cx).unwrap();
        assert_eq!(
            workspace
                .read(cx)
                .document(active, cx)
                .unwrap()
                .title
                .as_ref(),
            "orders"
        );
        assert!(!workspace.read(cx).is_preview(active, cx));
        window.click("command-search", cx);
    });
    palette_step(cx, handle, |window, cx| {
        window.input("k8s/development/pods", cx)
    });
    palette_step(cx, handle, |window, cx| {
        window
            .within("command")
            .click(IndexPath::new(0).section(1), cx);
    });
    palette_step(cx, handle, |window, cx| {
        let active = workspace.read(cx).active_view(cx).unwrap();
        assert_eq!(
            workspace
                .read(cx)
                .document(active, cx)
                .unwrap()
                .title
                .as_ref(),
            "Pods"
        );
        assert!(!workspace.read(cx).is_preview(active, cx));
        window.press("cmd-k", cx);
    });
    palette_step(cx, handle, |window, cx| window.input("overview", cx));
    palette_step(cx, handle, |window, cx| window.press("enter", cx));
    palette_step(cx, handle, |window, cx| {
        let active = workspace.read(cx).active_view(cx).unwrap();
        assert_eq!(
            workspace
                .read(cx)
                .document(active, cx)
                .unwrap()
                .key
                .0
                .as_ref(),
            Page::Overview.id()
        );
        window.press("cmd-k", cx);
    });
    palette_step(cx, handle, |window, cx| window.input("customers", cx));
    palette_step(cx, handle, |window, cx| window.press("enter", cx));
    palette_step(cx, handle, |window, cx| {
        assert_eq!(workspace.read(cx).active_view(cx), Some(first));
        assert_eq!(
            resource(&workspace, first, cx).entity_id(),
            original.entity_id()
        );
        assert_eq!(
            original.read(cx).editable.read(cx).active_position(cx),
            Some((2, 1))
        );
        // The restored retained view, rather than the dismissed modal, owns input.
        assert!(gpui::Focusable::focus_handle(&original, cx).contains_focused(window, cx));
    });
}

#[gpui_kit::test]
fn command_palette_actions_close_once_and_work_with_hidden_sidebar(cx: &mut TestAppContext) {
    use gpui::component::IndexPath;
    let (handle, workbench) = workbench(cx);
    for (query, row) in [("Toggle Sidebar", 0), ("Toggle Inspector", 1)] {
        for pointer in [false, true] {
            let (sidebar_before, inspector_before) = cx.update(|cx| {
                let view = workbench.read(cx);
                (
                    view.layout.read(cx).sidebar_visible,
                    view.inspector.read(cx).layout.read(cx).panel_visible,
                )
            });
            palette_step(cx, handle, |window, cx| window.press("cmd-k", cx));
            palette_step(cx, handle, |window, cx| window.input(query, cx));
            palette_step(cx, handle, |window, cx| {
                if pointer {
                    window
                        .within("command")
                        .click(IndexPath::new(row).section(3), cx);
                } else {
                    window.press("enter", cx);
                }
            });
            palette_step(cx, handle, |window, cx| {
                assert!(!window.has_active_dialog(cx));
                let view = workbench.read(cx);
                assert_eq!(
                    view.layout.read(cx).sidebar_visible,
                    if row == 0 {
                        !sidebar_before
                    } else {
                        sidebar_before
                    }
                );
                assert_eq!(
                    view.inspector.read(cx).layout.read(cx).panel_visible,
                    if row == 1 {
                        !inspector_before
                    } else {
                        inspector_before
                    }
                );
            });
        }
    }
    palette_step(cx, handle, |window, cx| {
        window.press("cmd-b", cx);
        assert!(!workbench.read(cx).layout.read(cx).sidebar_visible);
        window.press("ctrl-k", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(!workbench.read(cx).layout.read(cx).sidebar_visible);
        assert_eq!(window.find_by_label(PALETTE_INPUT).focused(), Some(true));
        window.press("escape", cx);
        window.click("activity-search", cx);
        assert!(workbench.read(cx).layout.read(cx).sidebar_visible);
        assert_eq!(window.find("resource-search").focused(), Some(true));
    });
}

#[gpui_kit::test]
fn command_palette_resource_navigation_preserves_edit_validation(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let (workspace, first, original) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.update(cx, |w, cx| {
                w.open(
                    Dataset::fixtures()[3].document.clone(),
                    OpenMode::Permanent,
                    window,
                    cx,
                )
            });
            let original = resource(&workspace, first, cx);
            (workspace, first, original)
        })
        .unwrap();
    let grid = cx.update(|cx| original.read(cx).editable.clone());
    palette_step(cx, handle, |window, cx| {
        window.click("editable-cell:0:0", cx);
        window.press("f2", cx);
        window.press("cmd-a", cx);
        window.press("backspace", cx);
        window.press("cmd-k", cx);
    });
    palette_step(cx, handle, |window, cx| window.input("orders", cx));
    palette_step(cx, handle, |window, cx| window.press("enter", cx));
    palette_step(cx, handle, |window, cx| {
        assert!(!window.has_active_dialog(cx));
        assert_eq!(workspace.read(cx).active_view(cx), Some(first));
        assert!(grid.read(cx).is_editing());
        assert_eq!(grid.read(cx).error().as_deref(), Some("Name is required"));
        assert_eq!(window.find("cell-editor").value(), Some(""));
        assert_eq!(window.find("cell-editor").focused(), Some(true));
        window.input("Palette saved", cx);
        window.press("cmd-k", cx);
    });
    palette_step(cx, handle, |window, cx| window.input("orders", cx));
    palette_step(cx, handle, |window, cx| window.press("enter", cx));
    palette_step(cx, handle, |_, cx| {
        assert_ne!(workspace.read(cx).active_view(cx), Some(first));
        assert!(!grid.read(cx).is_editing());
        assert_eq!(
            original
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .store
                .read(cx)
                .values[0][0]
                .as_ref(),
            "Palette saved"
        );
    });
}

#[test]
fn gallery_icon_source_contains_non_default_icons() {
    use gpui::{
        AssetSource,
        assets::{AllAssets, IconName},
    };
    for icon in [
        IconName::Database,
        IconName::Table,
        IconName::Box,
        IconName::Network,
        IconName::ClockArrowLeft,
        IconName::FunnelX,
        IconName::Sun,
        IconName::Moon,
        IconName::Settings,
        IconName::Search,
    ] {
        assert!(
            AllAssets.load(&icon.path()).unwrap().is_some(),
            "missing {icon:?}"
        );
    }
}

#[gpui_kit::test]
fn overflowing_tabs_reveal_keyboard_selection(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    cx.simulate_window_resize(handle, size(px(1024.), px(680.)));
    cx.update_window(handle, |_, window, cx| {
        let workspace = view.read(cx).workspace.clone();
        for fixture in Dataset::fixtures() {
            workspace.update(cx, |w, cx| {
                w.open(fixture.document.clone(), OpenMode::Permanent, window, cx);
            });
        }
        for page in Page::ALL {
            workspace.update(cx, |w, cx| {
                w.open(
                    pintle_gpui_kit::shell::workspace::Document::new(
                        page.id(),
                        page.title(),
                        "Foundation",
                    ),
                    OpenMode::Permanent,
                    window,
                    cx,
                );
            });
        }
        window.render_frame(cx);
        let group = workspace.read(cx).groups(cx).remove(0);
        for id in [group.views[0], *group.views.last().unwrap()] {
            workspace.update(cx, |w, cx| w.select(id, window, cx));
            window.render_frame(cx);
            let bar = window.find(("workspace-tabs", group.id.as_u64())).bounds();
            let tab = workspace_tab(window, id).bounds();
            assert!(tab.left() >= bar.left() && tab.right() <= bar.right());
        }
        window.press("ctrl-pagedown", cx);
        assert_eq!(workspace.read(cx).active_view(cx), Some(group.views[0]));
        let bar = window.find(("workspace-tabs", group.id.as_u64())).bounds();
        let tab = workspace_tab(window, group.views[0]).bounds();
        assert!(tab.left() >= bar.left() && tab.right() <= bar.right());
    })
    .unwrap();
}

#[gpui_kit::test]
fn vscode_activity_details_output_and_retained_panel(cx: &mut TestAppContext) {
    use super::inspector::InspectorTab;
    let (handle, view) = workbench(cx);
    let (workspace, inspector) = cx.update(|cx| {
        (
            view.read(cx).workspace.clone(),
            view.read(cx).inspector.clone(),
        )
    });
    let first = cx.update(|cx| workspace.read(cx).active_view(cx).unwrap());
    let (original, initial_height) = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("activity-bar").bounds().size.width, px(48.));
            assert!(window.find("inspector-empty").visible());
            let initial_height = window.find("inspector-panel").bounds().size.height;
            assert!((initial_height - px(240.)).abs() < px(2.));
            let original = resource(&workspace, first, cx);
            window
                .within(("resource-view", original.entity_id()))
                .click("table-cell-0:1", cx);
            assert_eq!(inspector.read(cx).details(cx)[1].1.as_ref(), "สมชาย 1");
            assert!(window.find(("detail-field", 4usize)).visible());
            (original, initial_height)
        })
        .unwrap();
    let second = cx
        .update_window(handle, |_, window, cx| {
            workspace.update(cx, |w, cx| {
                w.open(
                    Dataset::fixtures()[3].document.clone(),
                    OpenMode::Permanent,
                    window,
                    cx,
                )
            })
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(inspector.read(cx).details(cx).is_empty());
        let pods = resource(&workspace, second, cx);
        window
            .within(("resource-view", pods.entity_id()))
            .click("table-cell-1:0", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        assert_eq!(inspector.read(cx).details(cx)[0].1.as_ref(), "api-0002");
        workspace.update(cx, |w, cx| w.select(first, window, cx));
        window.render_frame(cx);
        assert_eq!(inspector.read(cx).details(cx)[1].1.as_ref(), "สมชาย 1");
        window
            .within(("resource-view", original.entity_id()))
            .click("row-filter", cx);
        window.input("José", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        assert!(inspector.read(cx).details(cx).is_empty());
        window
            .within(("resource-view", original.entity_id()))
            .click(("icon-sort", 0usize), cx);
        window.click("inspector-output", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        assert_eq!(inspector.read(cx).tab, InspectorTab::Output);
        assert!(
            inspector
                .read(cx)
                .entries
                .iter()
                .any(|line| line.starts_with("Filter customers:"))
        );
        assert!(
            inspector
                .read(cx)
                .entries
                .iter()
                .any(|line| line.starts_with("Sort customers:"))
        );
        assert!(window.find("inspector-output-content").visible());
        window.click("clear-output", cx);
        assert!(inspector.read(cx).entries.is_empty());
        window.press("left", cx);
        assert_eq!(inspector.read(cx).tab, InspectorTab::Details);
        let divider = window
            .within("workspace-inspector")
            .find(("resizable-handle", 0usize))
            .bounds()
            .center();
        window.drag(divider, divider + gpui::point(px(0.), px(-70.)), cx);
        let resized = window.find("inspector-panel").bounds().size.height;
        assert!((resized - initial_height - px(70.)).abs() < px(3.));
        window.click("close-inspector", cx);
        assert!(window.try_find("inspector-panel").is_none());
        window.press("cmd-j", cx);
        assert!((window.find("inspector-panel").bounds().size.height - resized).abs() < px(2.));
        window.click("activity-explorer", cx);
        assert!(window.try_find("explorer-frame").is_none());
        window.click("activity-search", cx);
        assert!(window.find("explorer-frame").visible());
        assert_eq!(window.find("resource-search").focused(), Some(true));
        window.press("cmd-b", cx);
        assert!(window.try_find("explorer-frame").is_none());
        window.press("cmd-k", cx);
        assert!(window.has_active_dialog(cx));
        assert!(window.try_find("explorer-frame").is_none());
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window
                .find_by_label("Search resources and commands...")
                .focused(),
            Some(true)
        );
        window.press("escape", cx);
    })
    .unwrap();
    cx.simulate_window_resize(handle, size(px(1024.), px(680.)));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let height = window.find("inspector-panel").bounds().size.height;
        assert!(height >= px(120.) && height <= px((680. - 35. - 22. - 6.) / 2.));
        let original = resource(&workspace, first, cx);
        assert!(
            window
                .find(("resource-view", original.entity_id()))
                .bounds()
                .size
                .height
                >= px(200.)
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn output_is_bounded_and_workspace_commands_are_recorded(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    let (workspace, inspector) = cx.update(|cx| {
        (
            view.read(cx).workspace.clone(),
            view.read(cx).inspector.clone(),
        )
    });
    let (first, split) = cx
        .update_window(handle, |_, window, cx| {
            let first = workspace.read(cx).active_view(cx).unwrap();
            let split = workspace
                .update(cx, |w, cx| {
                    w.split(first, SplitDirection::Right, window, cx)
                })
                .unwrap();
            (first, split)
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        let target = workspace
            .read(cx)
            .groups(cx)
            .into_iter()
            .find(|g| g.views.contains(&split))
            .unwrap()
            .id;
        workspace.update(cx, |w, cx| w.move_view(first, target, None, window, cx));
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        workspace.update(cx, |w, cx| w.close(first, CloseScope::All, window, cx));
    })
    .unwrap();
    cx.update(|cx| {
        let entries = &inspector.read(cx).entries;
        for prefix in [
            "Open customers",
            "Split customers",
            "Move customers",
            "Close customers",
        ] {
            assert!(
                entries.iter().any(|line| line.starts_with(prefix)),
                "missing {prefix}: {entries:?}"
            );
        }
        assert!(inspector.read(cx).details(cx).is_empty());
        inspector.update(cx, |i, cx| {
            for n in 0..510 {
                i.record(format!("event {n}"), cx);
            }
        });
        assert_eq!(inspector.read(cx).entries.len(), 500);
        assert_eq!(inspector.read(cx).entries.front().unwrap(), "event 10");
        assert_eq!(inspector.read(cx).entries.back().unwrap(), "event 509");
    });
}

#[gpui_kit::test]
fn tree_focus_and_folder_tab_frames(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav:foundation", cx);
        let row = window.find("nav:foundation").bounds();
        assert_eq!(row.size.height,px(22.));
        let scale = window.scale_factor();
        let y = row.top().as_f32() * scale;
        let quads = window.painted_quads();
        assert!(quads.iter().any(|q| (q.bounds.top().as_f32() - y).abs() < 1.
            && (q.bounds.size.width.as_f32() - row.size.width.as_f32() * scale).abs() < 1.
            && q.border_color == cx.theme().ring
            && q.border_widths.left.as_f32() == scale));
        // Selection is a full row, never the former floating 2 px stripe.
        assert!(!quads.iter().any(|q| (q.bounds.size.width.as_f32() - 2. * scale).abs() < 0.1
            && q.bounds.top().as_f32() >= y && q.bounds.bottom().as_f32() <= y + 22. * scale
            && q.bounds.size.height.as_f32() > 16. * scale));
        let workspace = view.read(cx).workspace.clone();
        let first = workspace.read(cx).active_view(cx).unwrap();
        let long = workspace.update(cx, |w,cx| w.open(
            pintle_gpui_kit::shell::workspace::Document::new("very-long-document", "A very long resource title that must be truncated without moving the close button", "Foundation"),
            OpenMode::Permanent,window,cx));
        window.render_frame(cx);
        for id in [first, long] {
            let bounds = workspace_tab(window, id).bounds();
            assert_eq!(bounds.size.height,px(32.));
            assert!(bounds.size.width >= px(96.) && bounds.size.width <= px(220.));
            let before = window.find(("close-view",id.0.as_u64())).bounds();
            window.hover(("workspace-tab-title",id.0.as_u64()),cx);
            assert_eq!(before,window.find(("close-view",id.0.as_u64())).bounds());
        }
        window.click("workspace-split-right",cx);
        assert_eq!(workspace.read(cx).groups(cx).len(),2);
    }).unwrap();
}

#[gpui_kit::test]
fn folder_tabs_join_content_and_keep_all_close_buttons_in_both_modes(cx: &mut TestAppContext) {
    for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
        for (width, height) in [(1440., 900.), (1024., 680.)] {
            let (handle, view) = workbench(cx);
            cx.simulate_window_resize(handle, size(px(width), px(height)));
            cx.update_window(handle, |_, window, cx| {
                theme::set_mode(mode, cx);
                let workspace = view.read(cx).workspace.clone();
                let first = workspace.read(cx).active_view(cx).unwrap();
                let fixtures = Dataset::fixtures();
                let middle = workspace.update(cx, |w, cx| {
                    w.open(fixtures[1].document.clone(), OpenMode::Preview, window, cx)
                });
                workspace.update(cx, |w, cx| w.keep_open(middle, cx));
                let last = workspace.update(cx, |w, cx| {
                    w.open(
                        fixtures[2].document.clone(),
                        OpenMode::Permanent,
                        window,
                        cx,
                    )
                });
                window.render_frame(cx);
                let group = workspace.read(cx).groups(cx)[0].id;
                for selected in [first, middle, last] {
                    window.click(("workspace-tab-title", selected.0.as_u64()), cx);
                    assert_eq!(workspace.read(cx).active_view(cx), Some(selected));
                    let bar = window.find(("workspace-tabs", group.as_u64())).bounds();
                    assert_eq!(bar.size.height, px(36.));
                    for id in [first, middle, last] {
                        let tab = workspace_tab(window, id);
                        assert_eq!(tab.selected(), Some(id == selected));
                        assert_eq!(tab.bounds().size.height, px(32.));
                        assert_eq!(tab.bounds().top(), bar.top() + px(4.));
                        assert_eq!(tab.bounds().bottom(), bar.bottom());
                        let close = window.find(("close-view", id.0.as_u64()));
                        assert!(close.visible());
                        assert_eq!(close.bounds().size, size(px(18.), px(18.)));
                        assert!(tab.bounds().contains(&close.bounds().center()));
                    }
                    // The padding directly below the tab bar uses the selected
                    // tab's fill, without a divider crossing their shared edge.
                    let scale = window.scale_factor();
                    let seam = gpui::point(bar.center().x, bar.bottom() + px(1.)).scale(scale);
                    assert!(
                        window
                            .painted_quads()
                            .iter()
                            .any(|quad| quad.bounds.contains(&seam)
                                && quad.background == cx.theme().tab_active.into())
                    );
                    assert!(
                        !window
                            .painted_quads()
                            .iter()
                            .any(|quad| quad.bounds.top() >= bar.top().scale(scale)
                                && quad.bounds.bottom() <= bar.bottom().scale(scale)
                                && quad.background == cx.theme().ring.into())
                    );
                }
                // Close an inactive tab directly, without selecting it first.
                window.click(("close-view", middle.0.as_u64()), cx);
                assert!(workspace.read(cx).document(middle, cx).is_none());
                assert_eq!(workspace.read(cx).active_view(cx), Some(last));
            })
            .unwrap();
        }
    }
}

fn drag_pointer(
    window: &mut gpui::Window,
    position: gpui::Point<gpui::Pixels>,
    cx: &mut gpui::App,
) {
    window.dispatch_event(
        gpui::MouseMoveEvent {
            position,
            pressed_button: Some(gpui::MouseButton::Left),
            modifiers: window.modifiers(),
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}
fn begin_drag(
    window: &mut gpui::Window,
    from: gpui::Point<gpui::Pixels>,
    to: gpui::Point<gpui::Pixels>,
    cx: &mut gpui::App,
) {
    window.dispatch_event(
        gpui::MouseMoveEvent {
            position: from,
            pressed_button: None,
            modifiers: window.modifiers(),
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        gpui::MouseDownEvent {
            position: from,
            button: gpui::MouseButton::Left,
            click_count: 1,
            modifiers: window.modifiers(),
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    drag_pointer(window, from + gpui::point(px(8.), px(0.)), cx);
    drag_pointer(window, to, cx);
    assert!(cx.has_active_drag());
}
fn finish_drag(window: &mut gpui::Window, at: gpui::Point<gpui::Pixels>, cx: &mut gpui::App) {
    window.dispatch_event(
        gpui::MouseUpEvent {
            position: at,
            button: gpui::MouseButton::Left,
            click_count: 1,
            modifiers: window.modifiers(),
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}
fn insertion_markers(window: &gpui::Window) -> Vec<gpui::Bounds<gpui::ScaledPixels>> {
    let scale = window.scale_factor();
    window
        .painted_quads()
        .into_iter()
        .filter(|q| {
            (q.bounds.size.width.as_f32() - 3. * scale).abs() < 0.01
                && (q.bounds.size.height.as_f32() - 32. * scale).abs() < 0.01
        })
        .map(|q| q.bounds)
        .collect()
}
#[gpui_kit::test]
fn tab_insertion_marker_matches_before_after_append_and_cancel(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    let workspace = cx.update(|cx| view.read(cx).workspace.clone());
    let (first, second, third) = cx
        .update_window(handle, |_, window, cx| {
            let fixtures = Dataset::fixtures();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace.update(cx, |w, cx| {
                w.open(
                    fixtures[1].document.clone(),
                    OpenMode::Permanent,
                    window,
                    cx,
                )
            });
            let third = workspace.update(cx, |w, cx| {
                w.open(fixtures[2].document.clone(), OpenMode::Preview, window, cx)
            });
            (first, second, third)
        })
        .unwrap();
    for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
        cx.update_window(handle, |_, window, cx| {
            theme::set_mode(mode, cx);
            workspace.update(cx, |w, cx| w.select(third, window, cx));
            window.render_frame(cx);
            let a = workspace_tab(window, first).bounds();
            let source = window
                .find(("workspace-tab-title", third.0.as_u64()))
                .bounds()
                .center();
            let before = gpui::point(a.left() + px(5.), a.center().y);
            begin_drag(window, source, before, cx);
            let markers = insertion_markers(window);
            assert_eq!(markers.len(), 1, "one marker in {mode:?}");
            assert!(
                (markers[0].left().as_f32() - a.left().as_f32() * window.scale_factor()).abs()
                    <= window.scale_factor() + 0.01
            );
            assert!(
                window
                    .painted_quads()
                    .iter()
                    .any(|q| q.bounds == markers[0] && q.background == cx.theme().ring.into())
            );
            assert!(!workspace.read(cx).is_preview(third, cx));
            assert_eq!(workspace_tab(window, first).bounds(), a);
            finish_drag(window, before, cx);
            assert!(insertion_markers(window).is_empty());
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(
                workspace.read(cx).groups(cx)[0].views,
                vec![third, first, second]
            )
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let b = workspace_tab(window, second).bounds();
            let after = gpui::point(b.right() - px(5.), b.center().y);
            let source = window
                .find(("workspace-tab-title", third.0.as_u64()))
                .bounds()
                .center();
            begin_drag(window, source, after, cx);
            assert_eq!(insertion_markers(window).len(), 1);
            assert!(
                (insertion_markers(window)[0].left().as_f32()
                    - b.right().as_f32() * window.scale_factor())
                .abs()
                    <= window.scale_factor() + 0.01
            );
            finish_drag(window, after, cx);
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(
                workspace.read(cx).groups(cx)[0].views,
                vec![first, second, third]
            )
        });
    }
    for (source, target, after, expected) in [
        (first, second, true, vec![second, first, third]),
        (third, second, true, vec![second, third, first]),
        (first, second, false, vec![first, second, third]),
    ] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let from = window
                .find(("workspace-tab-title", source.0.as_u64()))
                .bounds()
                .center();
            let bounds = workspace_tab(window, target).bounds();
            let to = gpui::point(
                if after {
                    bounds.right() - px(4.)
                } else {
                    bounds.left() + px(4.)
                },
                bounds.center().y,
            );
            begin_drag(window, from, to, cx);
            assert_eq!(insertion_markers(window).len(), 1);
            finish_drag(window, to, cx);
        })
        .unwrap();
        cx.update(|cx| assert_eq!(workspace.read(cx).groups(cx)[0].views, expected));
    }
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let group = workspace.read(cx).groups(cx)[0].id;
        let a = workspace_tab(window, first).bounds();
        window.click("resource-search", cx);
        begin_drag(
            window,
            a.center(),
            gpui::point(a.right() - px(4.), a.center().y),
            cx,
        );
        assert!(insertion_markers(window).is_empty());
        let end = workspace_tab(window, third).bounds().right();
        let bar = window.find(("workspace-tabs", group.as_u64())).bounds();
        let tail = gpui::point(end + px(12.), bar.center().y);
        drag_pointer(window, tail, cx);
        assert_eq!(insertion_markers(window).len(), 1);
        let commands = window.find("workspace-commands").bounds().center();
        drag_pointer(window, commands, cx);
        assert!(insertion_markers(window).is_empty());
        drag_pointer(window, tail, cx);
        window.press("escape", cx);
        assert!(!cx.has_active_drag());
        assert!(insertion_markers(window).is_empty());
        finish_drag(window, tail, cx);
        begin_drag(window, a.center(), tail, cx);
        finish_drag(window, tail, cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            workspace.read(cx).groups(cx)[0].views,
            vec![second, third, first]
        )
    });
}

#[gpui_kit::test]
fn tab_insertion_cross_group_scrolled_strip_retains_the_view(cx: &mut TestAppContext) {
    let (handle, view) = workbench(cx);
    cx.simulate_window_resize(handle, size(px(1024.), px(680.)));
    let (workspace, resource, source, last) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = view.read(cx).workspace.clone();
            let fixtures = Dataset::fixtures();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let source = workspace.update(cx, |w, cx| {
                w.open(
                    fixtures[1].document.clone(),
                    OpenMode::Permanent,
                    window,
                    cx,
                )
            });
            let resource = resource(&workspace, source, cx);
            resource
                .read(cx)
                .filter
                .clone()
                .update(cx, |input, cx| input.set_value("EUR", window, cx));
            workspace
                .update(cx, |w, cx| {
                    w.split(first, SplitDirection::Right, window, cx)
                })
                .unwrap();
            for dataset in fixtures.iter().skip(2) {
                workspace.update(cx, |w, cx| {
                    w.open(dataset.document.clone(), OpenMode::Permanent, window, cx);
                });
            }
            let mut last = first;
            for page in Page::ALL {
                last = workspace.update(cx, |w, cx| {
                    w.open(
                        pintle_gpui_kit::shell::workspace::Document::new(
                            page.id(),
                            page.title(),
                            "Foundation",
                        ),
                        OpenMode::Permanent,
                        window,
                        cx,
                    )
                });
            }
            window.render_frame(cx);
            (workspace, resource, source, last)
        })
        .unwrap();
    let target_id = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let target = workspace
                .read(cx)
                .groups(cx)
                .into_iter()
                .find(|g| g.views.contains(&last))
                .unwrap();
            let viewport = window.find(("workspace-tabs", target.id.as_u64())).bounds();
            window.dispatch_event(
                gpui::MouseMoveEvent {
                    position: viewport.center(),
                    pressed_button: None,
                    modifiers: Default::default(),
                }
                .to_platform_input(),
                cx,
            );
            window.dispatch_event(
                gpui::ScrollEvent {
                    position: viewport.center(),
                    delta: gpui::ScrollDelta::Pixels(gpui::point(px(-10000.), px(0.))),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            let last_bounds = workspace_tab(window, last).bounds();
            let viewport = window.find(("workspace-tabs", target.id.as_u64())).bounds();
            assert!(
                last_bounds.left() >= viewport.left() && last_bounds.right() <= viewport.right()
            );
            // Selecting the source leaves the destination group's independent tab scroll intact.
            workspace.update(cx, |w, cx| w.select(source, window, cx));
            window.render_frame(cx);
            let from = window
                .find(("workspace-tab-title", source.0.as_u64()))
                .bounds()
                .center();
            let before = gpui::point(last_bounds.left() + px(4.), last_bounds.center().y);
            begin_drag(window, from, before, cx);
            let markers = insertion_markers(window);
            assert_eq!(markers.len(), 1);
            assert!(
                (markers[0].left().as_f32() - last_bounds.left().as_f32() * window.scale_factor())
                    .abs()
                    < 1.
            );
            assert!(
                window
                    .painted_quads()
                    .iter()
                    .any(|q| q.bounds == markers[0] && q.background == cx.theme().ring.into())
            );
            finish_drag(window, before, cx);
            assert!(insertion_markers(window).is_empty());
            target.id
        })
        .unwrap();
    cx.update(|cx| {
        let group = workspace
            .read(cx)
            .groups(cx)
            .into_iter()
            .find(|g| g.id == target_id)
            .unwrap();
        assert_eq!(&group.views[group.views.len() - 2..], &[source, last]);
        assert_eq!(
            workspace.read(cx).page(source, cx).unwrap().entity_id(),
            resource.entity_id()
        );
        assert_eq!(resource.read(cx).filter.read(cx).value().as_ref(), "EUR");
    });
}

#[gpui_kit::test]
fn rounded_frames_finish_with_their_outline_and_have_no_idle_divider_lines(
    cx: &mut TestAppContext,
) {
    let (handle, _) = workbench(cx);
    for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
        cx.update_window(handle, |_, window, cx| {
            theme::set_mode(mode, cx);
            window.render_frame(cx);
            let scale = window.scale_factor();
            let quads = window.painted_quads();
            for id in ["explorer-frame", "tab-panel", "inspector-panel"] {
                let frame = window.find(id).bounds();
                let outline = quads
                    .iter()
                    .rev()
                    .find(|q| {
                        (q.bounds.left().as_f32() - frame.left().as_f32() * scale).abs() < 0.1
                            && (q.bounds.top().as_f32() - frame.top().as_f32() * scale).abs() < 0.1
                            && (q.bounds.size.width.as_f32() - frame.size.width.as_f32() * scale)
                                .abs()
                                < 0.1
                            && (q.bounds.size.height.as_f32() - frame.size.height.as_f32() * scale)
                                .abs()
                                < 0.1
                            && q.border_color == cx.theme().border
                    })
                    .unwrap_or_else(|| panic!("missing final rounded outline for {id}: {frame:?}"));
                assert_eq!(outline.corner_radii.top_left.as_f32(), 8. * scale);
                assert_eq!(outline.corner_radii.bottom_right.as_f32(), 8. * scale);
                assert_eq!(outline.border_widths.left.as_f32(), scale);
            }
            let sidebar = window.find("explorer-frame").bounds();
            // The resize band must not add a square full-height stroke beside
            // a region that already has its own curved outline.
            assert!(!quads.iter().any(|q| q.bounds.size.width.as_f32() <= scale
                && q.bounds.size.height.as_f32() > sidebar.size.height.as_f32() * scale * 0.9
                && q.bounds.left().as_f32() >= sidebar.right().as_f32() * scale
                && q.bounds.left().as_f32() <= (sidebar.right().as_f32() + 6.) * scale));
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn rounded_overlay_respects_ancestor_clipping(cx: &mut TestAppContext) {
    struct CroppedFrame {
        left: f32,
        top: f32,
    }
    impl gpui::Render for CroppedFrame {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            use gpui::{ParentElement, Styled};
            gpui::div()
                .relative()
                .w(px(120.))
                .h(px(80.))
                .overflow_hidden()
                .child(
                    gpui::div()
                        .absolute()
                        .left(px(self.left))
                        .top(px(self.top))
                        .w(px(200.))
                        .h(px(140.))
                        .flex_shrink_0()
                        .relative()
                        .border_1()
                        .border_color(gpui::transparent_black())
                        .rounded(px(8.))
                        .overflow_hidden()
                        .child(pintle_gpui_kit::components::rounded_frame_overlay(
                            px(8.),
                            gpui::rgb(0xfafafd).into(),
                            gpui::rgb(0x123456).into(),
                        )),
                )
        }
    }
    for (left, top) in [(0., 0.), (-40., -20.)] {
        let handle = cx.open_window(size(px(400.), px(300.)), |_, _| CroppedFrame { left, top });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let outlines: Vec<_> = window
                .painted_quads()
                .into_iter()
                .filter(|q| q.border_color == gpui::rgb(0x123456).into())
                .collect();
            if left < 0. {
                // All four border strips are outside the crop; only the
                // frame's interior is visible, so no outline may escape.
                assert!(outlines.is_empty());
                return;
            }
            let outline = &outlines[0];
            let scale = window.scale_factor();
            assert!(outline.content_mask.bounds.left().as_f32() >= 0.);
            assert!(outline.content_mask.bounds.top().as_f32() >= 0.);
            assert!(outline.content_mask.bounds.right().as_f32() <= 120. * scale);
            assert!(outline.content_mask.bounds.bottom().as_f32() <= 80. * scale);
        })
        .unwrap();
    }
}

fn assert_workspace_outlines(window: &gpui::Window, cx: &gpui::App) {
    let scale = window.scale_factor();
    let quads = window.painted_quads();
    for id in [
        "explorer-frame",
        "tab-panel",
        "inspector-panel",
        "empty-workspace",
    ] {
        for element in window.find_all(id) {
            let frame = element.bounds();
            let outlines: Vec<_> = quads
                .iter()
                .filter(|q| {
                    (q.bounds.left().as_f32() - frame.left().as_f32() * scale).abs() < 0.1
                        && (q.bounds.top().as_f32() - frame.top().as_f32() * scale).abs() < 0.1
                        && (q.bounds.size.width.as_f32() - frame.size.width.as_f32() * scale).abs()
                            < 0.1
                        && (q.bounds.size.height.as_f32() - frame.size.height.as_f32() * scale)
                            .abs()
                            < 0.1
                        && q.border_color == cx.theme().border
                        && q.border_widths.left.as_f32() > 0.
                })
                .collect();
            // GPUI optimizes one transparent outline into up to four disjoint
            // paint strips. A second stroke would overlap those masks.
            assert!(
                !outlines.is_empty() && outlines.len() <= 4,
                "one outline for {id}: {frame:?}"
            );
            for (ix, a) in outlines.iter().enumerate() {
                for b in &outlines[ix + 1..] {
                    assert!(
                        a.content_mask
                            .bounds
                            .intersect(&b.content_mask.bounds)
                            .is_empty(),
                        "duplicate outline for {id}"
                    );
                }
            }
            let q = outlines[0];
            for radius in [
                q.corner_radii.top_left,
                q.corner_radii.top_right,
                q.corner_radii.bottom_left,
                q.corner_radii.bottom_right,
            ] {
                assert_eq!(radius.as_f32(), 8. * scale, "{id}");
            }
            assert_eq!(q.border_widths.left.as_f32(), scale);
            // Deferred paint must stay in the region that owns the frame.
            assert!(q.content_mask.bounds.left().as_f32() >= frame.left().as_f32() * scale - 0.1);
            assert!(q.content_mask.bounds.top().as_f32() >= frame.top().as_f32() * scale - 0.1);
            assert!(q.content_mask.bounds.right().as_f32() <= frame.right().as_f32() * scale + 0.1);
            assert!(
                q.content_mask.bounds.bottom().as_f32() <= frame.bottom().as_f32() * scale + 0.1
            );
        }
    }
}

// Check painting before releasing the pointer, not just the settled layout.
fn resize_and_check_frames(
    window: &mut gpui::Window,
    from: gpui::Point<gpui::Pixels>,
    delta: gpui::Point<gpui::Pixels>,
    cx: &mut gpui::App,
) {
    window.dispatch_event(
        gpui::MouseDownEvent {
            position: from,
            button: gpui::MouseButton::Left,
            click_count: 1,
            modifiers: window.modifiers(),
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    for fraction in [0.25, 0.5, 1.] {
        drag_pointer(window, from + delta * fraction, cx);
        assert_workspace_outlines(window, cx);
    }
    finish_drag(window, from + delta, cx);
    assert_workspace_outlines(window, cx);
}

#[gpui_kit::test]
fn rounded_frame_matrix_during_nested_splits_resizing_and_panel_changes(cx: &mut TestAppContext) {
    use gpui::component::dock::{DockPlacement, PaneRef};
    for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
        for (width, height) in [(1440., 900.), (1024., 680.)] {
            let (handle, view) = workbench(cx);
            cx.simulate_window_resize(handle, size(px(width), px(height)));
            cx.update_window(handle, |_, window, cx| {
                theme::set_mode(mode, cx);
                let workspace = view.read(cx).workspace.clone();
                let first = workspace.read(cx).active_view(cx).unwrap();
                let fixtures = Dataset::fixtures();
                // An internal active tab has two rounded corners of its own.
                let pods = workspace.update(cx, |w, cx| {
                    w.open(
                        fixtures[3].document.clone(),
                        OpenMode::Permanent,
                        window,
                        cx,
                    )
                });
                workspace.update(cx, |w, cx| {
                    w.open(
                        fixtures[1].document.clone(),
                        OpenMode::Permanent,
                        window,
                        cx,
                    );
                    w.select(pods, window, cx);
                });
                window.render_frame(cx);
                let tab = workspace_tab(window, pods).bounds();
                let bar = window
                    .find((
                        "workspace-tabs",
                        workspace.read(cx).groups(cx)[0].id.as_u64(),
                    ))
                    .bounds();
                assert_eq!(tab.top(), bar.top() + px(4.));
                assert_eq!(tab.bottom(), bar.bottom());
                assert!(
                    window
                        .find_all(("workspace-tab-indicator", pods.0.as_u64()))
                        .is_empty()
                );
                assert_workspace_outlines(window, cx);
                let right = workspace
                    .update(cx, |w, cx| w.split(pods, SplitDirection::Right, window, cx))
                    .unwrap();
                workspace.update(cx, |w, cx| {
                    w.select(first, window, cx);
                    w.split(first, SplitDirection::Down, window, cx);
                });
                window.render_frame(cx);
                assert_eq!(window.find_all("tab-panel").len(), 3);
                assert_workspace_outlines(window, cx);
                // Both limits, with nested panes and the inspector still open.
                for dx in [700., -900.] {
                    let edge = window
                        .within("workspace-sidebar")
                        .find_all(("resizable-handle", 0usize))
                        .into_iter()
                        .min_by_key(|h| h.bounds().origin.x.as_f32() as i32)
                        .unwrap()
                        .bounds()
                        .center();
                    resize_and_check_frames(window, edge, gpui::point(px(dx), px(0.)), cx);
                    let sidebar_width = window.find("explorer-frame").bounds().size.width;
                    assert!((px(200.)..=px(480.)).contains(&sidebar_width));
                }
                for dy in [-900., 900.] {
                    let edge = window
                        .within("workspace-inspector")
                        .find_all(("resizable-handle", 0usize))
                        .into_iter()
                        .max_by_key(|h| h.bounds().center().y.as_f32() as i32)
                        .unwrap()
                        .bounds()
                        .center();
                    resize_and_check_frames(window, edge, gpui::point(px(0.), px(dy)), cx);
                    let panel_height = window.find("inspector-panel").bounds().size.height;
                    assert!(
                        panel_height >= px(120.) && panel_height <= px((height - 63.) / 2. + 1.)
                    );
                }
                let mut splits = Vec::new();
                workspace
                    .read(cx)
                    .dock()
                    .read(cx)
                    .layout(DockPlacement::Center)
                    .unwrap()
                    .root()
                    .walk(&mut |node| {
                        if let PaneRef::Split { axis, .. } = node.kind() {
                            splits.push((node.id(), axis));
                        }
                    });
                assert_eq!(splits.len(), 2);
                for (node, axis) in splits {
                    let edge = window
                        .within(("dock-split", node.as_u64()))
                        .find_all(("resizable-handle", 0usize))
                        .into_iter()
                        .max_by_key(|h| {
                            if axis == gpui::Axis::Horizontal {
                                h.bounds().size.height.as_f32() as i32
                            } else {
                                h.bounds().size.width.as_f32() as i32
                            }
                        })
                        .unwrap()
                        .bounds()
                        .center();
                    let delta = if axis == gpui::Axis::Horizontal {
                        gpui::point(px(45.), px(0.))
                    } else {
                        gpui::point(px(0.), px(35.))
                    };
                    resize_and_check_frames(window, edge, delta, cx);
                }
                // Paint the last virtualized rows against the bottom corners.
                for group in workspace.read(cx).groups(cx) {
                    let page = resource(&workspace, group.active.unwrap(), cx);
                    page.read(cx)
                        .table
                        .clone()
                        .update(cx, |t, cx| t.scroll_to_row(999, cx));
                }
                window.render_frame(cx);
                assert_workspace_outlines(window, cx);
                let page = resource(&workspace, right, cx);
                workspace.update(cx, |w, cx| w.select(right, window, cx));
                page.read(cx)
                    .table
                    .clone()
                    .update(cx, |t, cx| t.scroll_to_row(0, cx));
                window.render_frame(cx);
                window
                    .within(("resource-view", page.entity_id()))
                    .click("table-cell-0:0", cx);
                window.click("inspector-output", cx);
                assert_workspace_outlines(window, cx);
                window.click("inspector-details", cx);
                window.click("close-inspector", cx);
                assert_workspace_outlines(window, cx);
                window.press("cmd-j", cx);
                assert_workspace_outlines(window, cx);
                // Removing all groups and reopening uses the same frame contract.
                for group in workspace.read(cx).groups(cx) {
                    workspace.update(cx, |w, cx| {
                        w.close(group.views[0], CloseScope::All, window, cx)
                    });
                }
                window.render_frame(cx);
                assert!(window.find("empty-workspace").visible());
                assert_workspace_outlines(window, cx);
                workspace.update(cx, |w, cx| {
                    w.open(
                        fixtures[0].document.clone(),
                        OpenMode::Permanent,
                        window,
                        cx,
                    );
                });
                window.render_frame(cx);
                assert_workspace_outlines(window, cx);
            })
            .unwrap();
        }
    }
}

#[gpui_kit::test]
fn editable_cells_ranges_unicode_clipboard_and_validation(cx: &mut TestAppContext) {
    use pintle_gpui_kit::components::table::CellAddress;
    let (handle, workbench) = workbench(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let workspace = workbench.read(cx).workspace.clone();
        let view = resource(&workspace, workspace.read(cx).active_view(cx).unwrap(), cx);
        let grid = view.read(cx).editable.clone();
        window.click("editable-cell:0:1", cx);
        assert_eq!(grid.read(cx).active_position(cx), Some((0, 1)));
        window.press("shift-right", cx);
        window.press("shift-down", cx);
        assert_eq!(grid.read(cx).selection().cells.len(), 4);
        window.press("cmd-c", cx);
        assert!(
            cx.read_from_clipboard()
                .unwrap()
                .text()
                .unwrap()
                .contains('\t')
        );
        // Native text events start replacement editing, rather than assembling
        // key names (which would lose composed Unicode text).
        window.input("สมชาย José", cx);
        window.render_frame(cx);
        assert_eq!(grid.read(cx).selection().cells.len(), 4);
        assert_eq!(window.find("cell-editor").value(), Some("สมชาย José"));
        window.press("enter", cx);
        assert!(!grid.read(cx).is_editing());
        assert_eq!(
            view.read(cx)
                .table
                .read(cx)
                .delegate()
                .store
                .read(cx)
                .values[1][2]
                .as_ref(),
            "สมชาย José"
        );
        for row in 0..2 {
            for col in 1..3 {
                assert_eq!(
                    view.read(cx)
                        .table
                        .read(cx)
                        .delegate()
                        .store
                        .read(cx)
                        .values[row][col]
                        .as_ref(),
                    "สมชาย José"
                );
            }
        }
        // F2 preserves existing contents. Escape discards a draft.
        window.press("f2", cx);
        window.press("cmd-a", cx);
        window.input("discard", cx);
        window.press("escape", cx);
        assert!(!grid.read(cx).is_editing());
        // Database blanks enter the draft; only Save requires a name.
        window.click("editable-cell:0:1", cx);
        window.press("f2", cx);
        window.press("cmd-a", cx);
        window.press("backspace", cx);
        window.press("enter", cx);
        assert!(!grid.read(cx).is_editing());
        assert!(grid.read(cx).error().is_none());
        window.click("db-save", cx);
        assert!(grid.read(cx).is_editing());
        assert_eq!(grid.read(cx).error().as_deref(), Some("Name is required"));
        assert_eq!(window.find("cell-editor").focused(), Some(true));
        window.press("escape", cx);
        // Arbitrary disjoint shapes preserve the previous clipboard on error.
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("0", "id"), false, false, cx);
            s.select_cell(CellAddress::new("2", "email"), false, true, cx);
        });
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("unchanged".into()));
        window.press("cmd-c", cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("unchanged")
        );
        assert!(grid.read(cx).error().is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_dropdown_mouse_keyboard_and_shared_split_values(cx: &mut TestAppContext) {
    use pintle_gpui_kit::components::table::CellAddress;
    let (handle, workbench) = workbench(cx);
    let (workspace, first, second, original, duplicate) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| w.split(first, SplitDirection::Down, window, cx))
                .unwrap();
            let original = resource(&workspace, first, cx);
            let duplicate = resource(&workspace, second, cx);
            workspace.update(cx, |w, cx| w.select(first, window, cx));
            window.render_frame(cx);
            let grid = original.read(cx).editable.clone();
            grid.update(cx, |s, cx| {
                s.select_cell(CellAddress::new("0", "status"), false, false, cx)
            });
            let table = original.read(cx).table.clone();
            table.update(cx, |t, cx| t.scroll_to_col(4, cx));
            window.render_frame(cx);
            window.focus(&gpui::Focusable::focus_handle(&grid, cx), cx);
            window.press("alt-down", cx);
            assert!(grid.read(cx).is_editing(), "Alt+Down must open dropdown");
            window.render_frame(cx);
            assert!(window.find("cell-dropdown-popup").visible());
            window.input("inactive", cx);
            window.render_frame(cx);
            window.press("enter", cx);
            assert!(!grid.read(cx).is_editing());
            assert_eq!(
                table.read(cx).delegate().store.read(cx).values[0][4].as_ref(),
                "Inactive"
            );
            (workspace, first, second, original, duplicate)
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        assert_eq!(
            duplicate
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .store
                .read(cx)
                .values[0][4]
                .as_ref(),
            "Inactive"
        );
        assert_eq!(duplicate.read(cx).editable.read(cx).active_cell(), None);
        window.press("f2", cx);
        window.render_frame(cx);
        window
            .within("cell-dropdown-popup")
            .click(("cell-option", 0usize), cx);
        assert!(!original.read(cx).editable.read(cx).is_editing());
        assert_eq!(
            original
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .store
                .read(cx)
                .values[0][4]
                .as_ref(),
            "Active"
        );
        // Existing text is preserved when double-click starts editing.
        let grid = original.read(cx).editable.clone();
        original
            .read(cx)
            .table
            .clone()
            .update(cx, |t, cx| t.scroll_to_col(1, cx));
        window.render_frame(cx);
        window
            .within(("resource-view", original.entity_id()))
            .double_click("editable-cell:0:1", cx);
        window.render_frame(cx);
        assert!(grid.read(cx).is_editing());
        assert_eq!(window.find("cell-editor").value(), Some("สมชาย 1"));
        window.press("cmd-a", cx);
        window.input("Shared value", cx);
        // Switching tabs commits through the page guard.
        workspace.update(cx, |w, cx| w.select(second, window, cx));
        assert!(!grid.read(cx).is_editing());
        assert_eq!(
            duplicate
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .store
                .read(cx)
                .values[0][1]
                .as_ref(),
            "Shared value"
        );
        workspace.update(cx, |w, cx| w.select(first, window, cx));
        window.render_frame(cx);
        assert!(
            workbench
                .read(cx)
                .inspector
                .read(cx)
                .details(cx)
                .iter()
                .any(|(_, v)| v == "Shared value")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_pointer_ranges_add_toggle_and_ime(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler;
    let (handle, workbench) = workbench(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let workspace = workbench.read(cx).workspace.clone();
        let view = resource(&workspace, workspace.read(cx).active_view(cx).unwrap(), cx);
        let grid = view.read(cx).editable.clone();
        let from = window.find("editable-cell:0:0").bounds().center();
        let to = window.find("editable-cell:2:1").bounds().center();
        window.drag(from, to, cx);
        assert_eq!(grid.read(cx).selection().cells.len(), 6);
        let add = gpui::Modifiers {
            platform: true,
            ..Default::default()
        };
        window.click_with_modifiers("editable-cell:4:0", add, cx);
        assert_eq!(grid.read(cx).selection().cells.len(), 7);
        window.click_with_modifiers("editable-cell:4:0", add, cx);
        assert_eq!(grid.read(cx).selection().cells.len(), 6);
        window.click("editable-cell:0:0", cx);
        window.click_with_modifiers(
            "editable-cell:2:1",
            gpui::Modifiers {
                shift: true,
                ..Default::default()
            },
            cx,
        );
        assert_eq!(grid.read(cx).selection().cells.len(), 6);
        // The first marked native text event is forwarded intact to the one
        // retained InputState. Enter must not commit an unfinished composition.
        grid.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "に", Some(1..1), window, cx)
        });
        window.render_frame(cx);
        window.press("enter", cx);
        assert!(grid.read(cx).is_editing());
        assert_eq!(window.find("cell-editor").value(), Some("に"));
        window.press("escape", cx); // composition may consume the first Escape
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_source_updates_reconcile_filter_sort_and_stable_selection(cx: &mut TestAppContext) {
    use pintle_gpui_kit::components::table::{CellAddress, EditableTableDelegate};
    let (handle, workbench) = workbench(cx);
    let (source, other) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| w.split(first, SplitDirection::Down, window, cx))
                .unwrap();
            let source = resource(&workspace, first, cx);
            let other = resource(&workspace, second, cx);
            other.read(cx).editable.clone().update(cx, |s, cx| {
                s.update_delegate(cx, |rows, cx| {
                    rows.query = "สมชาย".into();
                    rows.rebuild(cx);
                });
                s.select_cell(CellAddress::new("0", "name"), false, false, cx);
            });
            source.read(cx).editable.clone().update(cx, |s, cx| {
                s.update_delegate(cx, |rows, cx| {
                    rows.commit(&CellAddress::new("0", "name"), "Changed".into(), window, cx)
                        .unwrap()
                })
            });
            (source, other)
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        assert_eq!(other.read(cx).table.read(cx).delegate().indices.len(), 249);
        assert_eq!(other.read(cx).editable.read(cx).active_cell(), None);
        let grid = source.read(cx).editable.clone();
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("2", "email"), false, false, cx);
            s.update_delegate(cx, |rows, cx| {
                rows.sort(
                    0,
                    gpui::component::table::ColumnSort::Descending,
                    window,
                    cx,
                )
            });
        });
        assert_eq!(
            grid.read(cx).active_cell(),
            Some(CellAddress::new("2", "email"))
        );
        assert_eq!(grid.read(cx).active_position(cx), Some((997, 2)));
        grid.update(cx, |s, cx| {
            s.update_delegate(cx, |rows, cx| {
                rows.commit(&CellAddress::new("2", "id"), "2000".into(), window, cx)
                    .unwrap()
            })
        });
        assert_eq!(
            grid.read(cx).active_cell(),
            Some(CellAddress::new("2", "email"))
        );
        assert_eq!(grid.read(cx).active_position(cx), Some((0, 2)));
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_edge_drag_autoscroll_and_release(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let (grid, table, at) = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let w = workbench.read(cx).workspace.clone();
            let view = resource(&w, w.read(cx).active_view(cx).unwrap(), cx);
            let grid = view.read(cx).editable.clone();
            let table = view.read(cx).table.clone();
            let from = window.find("editable-cell:0:0").bounds().center();
            let frame = window.find("editable-table").bounds();
            let at = gpui::point(from.x, frame.bottom() - px(4.));
            window.dispatch_event(
                gpui::MouseDownEvent {
                    position: from,
                    button: gpui::MouseButton::Left,
                    click_count: 1,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            drag_pointer(window, at, cx);
            (grid, table, at)
        })
        .unwrap();
    cx.run_until_parked();
    for _ in 0..12 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(40));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .unwrap();
    }
    cx.update_window(handle, |_, window, cx| {
        assert!(table.read(cx).visible_range().rows().start > 0);
        assert!(grid.read(cx).selection().cells.len() > 17);
        finish_drag(window, at, cx);
    })
    .unwrap();
    let before = cx.update(|cx| table.read(cx).visible_range().rows().clone());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert_eq!(table.read(cx).visible_range().rows(), &before);
    })
    .unwrap();
}

#[gpui_kit::test]
#[ignore = "manual optimized CPU comparison; run with --release --ignored --nocapture"]
fn editable_scrolling_cpu_reference(cx: &mut TestAppContext) {
    use gpui::component::Sizable;
    use gpui::component::table::{Column, DataTable, TableDelegate, TableState};
    use gpui::{ParentElement, Styled};
    use pintle_gpui_kit::components::table::{CellAddress, EditableTable, EditableTableState};
    struct Reference(std::rc::Rc<Dataset>);
    impl TableDelegate for Reference {
        fn rows_count(&self, _: &gpui::App) -> usize {
            1000
        }
        fn columns_count(&self, _: &gpui::App) -> usize {
            5
        }
        fn column(&self, col: usize, _: &gpui::App) -> Column {
            Column::new(self.0.columns[col], self.0.columns[col]).width(px(if col == 0 {
                180.
            } else {
                200.
            }))
        }
        fn render_td(
            &mut self,
            row: usize,
            col: usize,
            _: &mut gpui::Window,
            cx: &mut gpui::Context<TableState<Self>>,
        ) -> impl gpui::IntoElement {
            gpui::div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(13.))
                .child(self.0.rows[row][col].clone())
        }
    }
    struct Harness {
        reference: Entity<TableState<Reference>>,
        editable: Entity<EditableTableState<super::data::Rows>>,
        show_editable: bool,
    }
    impl gpui::Render for Harness {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            use gpui::{IntoElement, Styled};
            gpui::div().size_full().child(if self.show_editable {
                EditableTable::new(&self.editable).into_any_element()
            } else {
                DataTable::new(&self.reference)
                    .bordered(false)
                    .with_size(gpui::component::Size::Size(px(28.)))
                    .header_row_height(px(28.))
                    .into_any_element()
            })
        }
    }
    let (handle, view) = cx.update(|cx| {
        super::initialize(cx);
        gpui::open_window(super::appearance::window_options(cx), cx, |window, cx| {
            cx.new(|cx| {
                let data = Dataset::fixtures()[0].clone();
                let store = cx.new(|_| super::data::DatasetStore::new(&data));
                Harness {
                    reference: cx.new(|cx| TableState::new(Reference(data.clone()), window, cx)),
                    editable: cx.new(|cx| {
                        EditableTableState::new(super::data::Rows::new(data, store, cx), window, cx)
                    }),
                    show_editable: false,
                }
            })
        })
        .unwrap()
    });
    for mode in 0..3 {
        let mut samples = vec![];
        cx.update_window(handle, |_, window, cx| {
            view.update(cx, |v, cx| {
                v.show_editable = mode != 0;
                cx.notify();
            });
            if mode == 2 {
                view.read(cx).editable.clone().update(cx, |s, cx| {
                    s.select_cell(CellAddress::new("0", "id"), false, false, cx);
                    s.select_cell(CellAddress::new("99", "status"), true, false, cx);
                });
            }
            for i in 0..130 {
                let start = std::time::Instant::now();
                if mode == 0 {
                    view.read(cx)
                        .reference
                        .clone()
                        .update(cx, |t, cx| t.scroll_to_row((i * 7) % 960, cx));
                } else {
                    view.read(cx)
                        .editable
                        .read(cx)
                        .table()
                        .clone()
                        .update(cx, |t, cx| t.scroll_to_row((i * 7) % 960, cx));
                }
                window.render_frame(cx);
                if i >= 10 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                }
            }
        })
        .unwrap();
        samples.sort_by(f64::total_cmp);
        eprintln!(
            "scroll CPU mode={mode} median={:.3}ms p95={:.3}ms (120 headless frames; no GPU/vsync)",
            samples[60], samples[114]
        );
    }
}

#[gpui_kit::test]
fn editable_dropdown_fits_both_modes_sizes_and_draft_survives_move(cx: &mut TestAppContext) {
    use pintle_gpui_kit::components::table::CellAddress;
    for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
        for dimensions in [size(px(1440.), px(900.)), size(px(1024.), px(680.))] {
            let (handle, workbench) = workbench(cx);
            cx.simulate_window_resize(handle, dimensions);
            cx.update_window(handle, |_, window, cx| {
                theme::set_mode(mode, cx);
                let w = workbench.read(cx).workspace.clone();
                let first = w.read(cx).active_view(cx).unwrap();
                let source = resource(&w, first, cx);
                let grid = source.read(cx).editable.clone();
                grid.update(cx, |s, cx| {
                    s.select_cell(CellAddress::new("999", "status"), false, false, cx)
                });
                source.read(cx).table.clone().update(cx, |t, cx| {
                    t.scroll_to_row(999, cx);
                    t.scroll_to_col(4, cx);
                });
                window.focus(&gpui::Focusable::focus_handle(&grid, cx), cx);
                window.render_frame(cx);
                window.press("f2", cx);
                window.render_frame(cx);
                let popup = window.find("cell-dropdown-popup").bounds();
                assert!(popup.left() >= px(0.) && popup.right() <= dimensions.width);
                assert!(popup.top() >= px(0.) && popup.bottom() <= dimensions.height);
                window.press("escape", cx);
                assert!(!grid.read(cx).is_editing());
                assert_workspace_outlines(window, cx);
            })
            .unwrap();
        }
    }
    let (handle, workbench) = workbench(cx);
    let (w, source, target, first) = cx
        .update_window(handle, |_, window, cx| {
            let w = workbench.read(cx).workspace.clone();
            let first = w.read(cx).active_view(cx).unwrap();
            let source = resource(&w, first, cx);
            let second = w
                .update(cx, |w, cx| w.split(first, SplitDirection::Down, window, cx))
                .unwrap();
            let target = w
                .read(cx)
                .groups(cx)
                .into_iter()
                .find(|g| g.views.contains(&second))
                .unwrap()
                .id;
            w.update(cx, |w, cx| w.select(first, window, cx));
            window.render_frame(cx);
            window
                .within(("resource-view", source.entity_id()))
                .double_click("editable-cell:0:1", cx);
            window.press("cmd-a", cx);
            window.input("Pending draft", cx);
            w.update(cx, |w, cx| w.move_view(first, target, None, window, cx));
            assert!(source.read(cx).editable.read(cx).is_editing());
            (w, source, target, first)
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(w.read(cx).active_group(), Some(target));
        assert_eq!(resource(&w, first, cx).entity_id(), source.entity_id());
        assert_eq!(window.find("cell-editor").value(), Some("Pending draft"));
        window.press("escape", cx);
        assert_eq!(
            source
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .store
                .read(cx)
                .values[0][1]
                .as_ref(),
            "สมชาย 1"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_click_release_does_not_leave_an_autoscroll_gesture(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let grid = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let w = workbench.read(cx).workspace.clone();
            let view = resource(&w, w.read(cx).active_view(cx).unwrap(), cx);
            window.click("editable-cell:0:1", cx);
            window.press("shift-right", cx);
            window.press("shift-down", cx);
            view.read(cx).editable.clone()
        })
        .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(grid.read(cx).selection().cells.len(), 4);
        assert_eq!(grid.read(cx).active_position(cx), Some((1, 2)));
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_readonly_and_filter_validation(cx: &mut TestAppContext) {
    use pintle_gpui_kit::components::table::{
        CellAddress, CellEditor, EditableTableDelegate, EditableTableState,
    };
    struct ReadOnly;
    impl EditableTableDelegate for ReadOnly {
        fn rows_count(&self, _: &gpui::App) -> usize {
            1
        }
        fn columns_count(&self, _: &gpui::App) -> usize {
            1
        }
        fn row_key(&self, _: usize, _: &gpui::App) -> gpui::SharedString {
            "stable".into()
        }
        fn column(&self, _: usize, _: &gpui::App) -> gpui::component::table::Column {
            gpui::component::table::Column::new("id", "ID")
        }
        fn value(&self, _: &CellAddress, _: &gpui::App) -> gpui::SharedString {
            "Read only".into()
        }
        fn editor(&self, _: &CellAddress, _: &gpui::App) -> CellEditor {
            CellEditor::ReadOnly
        }
        fn commit(
            &mut self,
            _: &CellAddress,
            _: gpui::SharedString,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) -> Result<(), gpui::SharedString> {
            panic!("Read-only cells cannot commit")
        }
    }
    let (handle, grid) = cx.update(|cx| {
        super::initialize(cx);
        gpui::open_window(super::appearance::window_options(cx), cx, |window, cx| {
            cx.new(|cx| EditableTableState::new(ReadOnly, window, cx))
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("editable-cell:0:0", cx);
        window.input("ignored", cx);
        window.press("f2", cx);
        assert!(!grid.read(cx).is_editing());
        assert_eq!(grid.read(cx).selection().cells.len(), 1);
        grid.update(cx, |s, cx| {
            assert!(s.paste_text("blocked", window, cx).is_err())
        });
        assert!(grid.read(cx).error().unwrap().contains("read-only"));
    })
    .unwrap();
    let (handle, workbench) = workbench(cx);
    let view = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let w = workbench.read(cx).workspace.clone();
            let id = w.update(cx, |w, cx| {
                w.open(
                    Dataset::fixtures()[3].document.clone(),
                    OpenMode::Permanent,
                    window,
                    cx,
                )
            });
            let view = resource(&w, id, cx);
            window.render_frame(cx);
            window.double_click("editable-cell:0:0", cx);
            window.press("cmd-a", cx);
            window.press("backspace", cx);
            window.click("row-filter", cx);
            window.input("José", cx);
            view
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(view.read(cx).editable.read(cx).is_editing());
        assert!(view.read(cx).table.read(cx).delegate().query.is_empty());
        assert_eq!(window.find("row-filter").value(), Some(""));
        assert_eq!(
            view.read(cx).editable.read(cx).error().as_deref(),
            Some("Name is required")
        );
        window.press("escape", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_batch_disjoint_text_atomic_validation_and_shared_views(cx: &mut TestAppContext) {
    use pintle_gpui_kit::components::table::CellAddress;
    let (handle, workbench) = workbench(cx);
    cx.update_window(handle, |_, window, cx| {
        let workspace = workbench.read(cx).workspace.clone();
        let first = workspace.read(cx).active_view(cx).unwrap();
        let second = workspace
            .update(cx, |w, cx| {
                w.split(first, SplitDirection::Right, window, cx)
            })
            .unwrap();
        let original = resource(&workspace, first, cx);
        let duplicate = resource(&workspace, second, cx);
        workspace.update(cx, |w, cx| w.select(first, window, cx));
        let grid = original.read(cx).editable.clone();
        let store = original.read(cx).table.read(cx).delegate().store.clone();
        window.render_frame(cx);
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("0", "name"), false, false, cx);
            s.select_cell(CellAddress::new("2", "name"), false, true, cx);
        });
        window.focus(&gpui::Focusable::focus_handle(&grid, cx), cx);
        window.press("f2", cx);
        window.render_frame(cx);
        // Commit the active cell's unchanged value: other selected cells must
        // still receive it, without touching the intervening unselected row.
        let expected = store.read(cx).values[2][1].clone();
        let untouched = store.read(cx).values[1][1].clone();
        assert_eq!(grid.read(cx).selection().cells.len(), 2);
        grid.update(cx, |s, cx| assert!(s.commit_edit(window, cx)));
        assert_eq!(store.read(cx).values[0][1], expected);
        assert_eq!(store.read(cx).values[2][1], expected);
        assert_eq!(store.read(cx).values[1][1], untouched);
        assert_eq!(
            duplicate.read(cx).editable.read(cx).selection().cells.len(),
            0
        );
        assert_eq!(
            duplicate
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .store
                .read(cx)
                .values[0][1],
            expected
        );

        // The text target accepts a blank draft, but the closed dropdown
        // rejects it. Neither cell may change after that batch failure.
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("2", "status"), false, false, cx);
            s.select_cell(CellAddress::new("0", "email"), false, true, cx);
        });
        let before = store.read(cx).values.clone();
        window.press("f2", cx);
        window.press("cmd-a", cx);
        window.press("backspace", cx);
        window.press("enter", cx);
        assert!(grid.read(cx).is_editing());
        assert!(grid.read(cx).error().unwrap().contains("choose a value"));
        assert_eq!(store.read(cx).values, before);
        assert_eq!(grid.read(cx).selection().cells.len(), 2);
        window.press("escape", cx);
        assert_eq!(store.read(cx).values, before);
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_batch_dropdown_keyboard_mouse_and_incompatible_targets(cx: &mut TestAppContext) {
    use pintle_gpui_kit::components::table::CellAddress;
    let (handle, workbench) = workbench(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let workspace = workbench.read(cx).workspace.clone();
        let view = resource(&workspace, workspace.read(cx).active_view(cx).unwrap(), cx);
        let grid = view.read(cx).editable.clone();
        let table = view.read(cx).table.clone();
        let store = table.read(cx).delegate().store.clone();
        table.update(cx, |t, cx| t.scroll_to_col(4, cx));
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("0", "status"), false, false, cx);
            s.select_cell(CellAddress::new("2", "status"), false, true, cx);
        });
        window.focus(&gpui::Focusable::focus_handle(&grid, cx), cx);
        window.press("alt-down", cx);
        window.input("inactive", cx);
        window.render_frame(cx);
        window.press("enter", cx);
        assert!(!grid.read(cx).is_editing());
        assert_eq!(grid.read(cx).selection().cells.len(), 2);
        for row in [0, 2] {
            assert_eq!(store.read(cx).values[row][4].as_ref(), "Inactive");
        }
        assert_eq!(store.read(cx).values[1][4].as_ref(), "Active");
        window.press("f2", cx);
        window.render_frame(cx);
        window
            .within("cell-dropdown-popup")
            .click(("cell-option", 0usize), cx);
        for row in [0, 2] {
            assert_eq!(store.read(cx).values[row][4].as_ref(), "Active");
        }
        // Merely dismissing a searchable list must not fill other cells with
        // the active cell's old value.
        table.update(cx, |t, cx| {
            use pintle_gpui_kit::components::table::EditableTableDelegate;
            t.delegate_mut()
                .commit(
                    &CellAddress::new("2", "status"),
                    "Inactive".into(),
                    window,
                    cx,
                )
                .unwrap();
        });
        window.press("f2", cx);
        window.input("active", cx);
        grid.update(cx, |s, cx| assert!(s.commit_edit(window, cx)));
        assert_eq!(store.read(cx).values[0][4].as_ref(), "Active");
        assert_eq!(store.read(cx).values[2][4].as_ref(), "Inactive");

        // Text entered into a mixed range must honor every closed dropdown.
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("0", "status"), false, false, cx);
            s.select_cell(CellAddress::new("2", "name"), false, true, cx);
        });
        let before = store.read(cx).values.clone();
        window.input("not an option", cx);
        window.press("enter", cx);
        assert!(grid.read(cx).is_editing());
        assert!(grid.read(cx).error().unwrap().contains("choose a value"));
        assert_eq!(store.read(cx).values, before);
        window.press("escape", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_paste_rectangle_shortcut_unicode_and_editor_paste(cx: &mut TestAppContext) {
    use pintle_gpui_kit::components::table::CellAddress;
    let (handle, workbench) = workbench(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let workspace = workbench.read(cx).workspace.clone();
        let view = resource(&workspace, workspace.read(cx).active_view(cx).unwrap(), cx);
        let grid = view.read(cx).editable.clone();
        let store = view.read(cx).table.read(cx).delegate().store.clone();
        window.click("editable-cell:1:1", cx);
        let start = CellAddress::new("1", "name");
        // An existing disjoint selection does not repeat or reshape the paste;
        // the active cell is always the top-left destination.
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("8", "country"), false, true, cx);
            s.select_cell(start.clone(), false, true, cx);
            s.select_cell(start.clone(), false, true, cx);
        });
        let before = store.read(cx).values.clone();
        let text = "\"ไทย\tJosé\"\t\"line\n\"\"two\"\"\"\r\nAda\t\r\n";
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.into()));
        window.press("cmd-v", cx);
        assert!(!grid.read(cx).is_editing());
        assert_eq!(grid.read(cx).active_cell(), Some(start));
        assert_eq!(grid.read(cx).selection().cells.len(), 4);
        let data = &store.read(cx).values;
        assert_eq!(data[1][1].as_ref(), "ไทย\tJosé");
        assert_eq!(data[1][2].as_ref(), "line\n\"two\"");
        assert_eq!(data[2][1].as_ref(), "Ada");
        assert_eq!(data[2][2].as_ref(), "");
        assert_eq!(data[0], before[0]);
        assert_eq!(data[8], before[8]);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some(text)
        );
        // In an open editor, Cmd+V belongs to Kit InputState, not the grid.
        window.press("f2", cx);
        window.press("cmd-a", cx);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("Input draft".into()));
        window.press("cmd-v", cx);
        window.render_frame(cx);
        assert!(grid.read(cx).is_editing());
        assert_eq!(window.find("cell-editor").value(), Some("Input draft"));
        assert_eq!(store.read(cx).values[1][1].as_ref(), "ไทย\tJosé");
        window.press("escape", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_paste_atomic_validation_dropdown_bounds_and_bad_shape(cx: &mut TestAppContext) {
    use pintle_gpui_kit::components::table::CellAddress;
    let (handle, workbench) = workbench(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let workspace = workbench.read(cx).workspace.clone();
        let view = resource(&workspace, workspace.read(cx).active_view(cx).unwrap(), cx);
        let grid = view.read(cx).editable.clone();
        let store = view.read(cx).table.read(cx).delegate().store.clone();
        window.click("editable-cell:0:1", cx);
        let before = store.read(cx).values.clone();
        let selection = grid.read(cx).selection();
        for text in ["a\tb\nc", "\"unclosed"] {
            grid.update(cx, |s, cx| assert!(s.paste_text(text, window, cx).is_err()));
            assert_eq!(store.read(cx).values, before);
            assert_eq!(grid.read(cx).selection(), selection);
        }
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("0", "country"), false, false, cx)
        });
        let selection = grid.read(cx).selection();
        // First row is valid, second has a disallowed dropdown option.
        grid.update(cx, |s, cx| {
            assert!(
                s.paste_text("TH\tInactive\nJP\tUnknown", window, cx)
                    .is_err()
            )
        });
        assert_eq!(store.read(cx).values, before);
        assert_eq!(grid.read(cx).selection(), selection);
        grid.update(cx, |s, cx| {
            s.paste_text("TH\tInactive\nJP\tActive", window, cx)
                .unwrap()
        });
        assert_eq!(store.read(cx).values[0][3].as_ref(), "TH");
        assert_eq!(store.read(cx).values[0][4].as_ref(), "Inactive");
        assert_eq!(store.read(cx).values[1][3].as_ref(), "JP");
        let accepted = store.read(cx).values.clone();
        for (cell, text) in [
            (CellAddress::new("0", "status"), "Active\tOverflow"),
            (CellAddress::new("999", "name"), "Last\nOverflow"),
        ] {
            grid.update(cx, |s, cx| {
                s.select_cell(cell, false, false, cx);
                assert!(s.paste_text(text, window, cx).is_err());
            });
            assert_eq!(store.read(cx).values, accepted);
        }
        let id = workspace.update(cx, |w, cx| {
            w.open(
                Dataset::fixtures()[3].document.clone(),
                OpenMode::Permanent,
                window,
                cx,
            )
        });
        let pods = resource(&workspace, id, cx);
        let grid = pods.read(cx).editable.clone();
        let store = pods.read(cx).table.read(cx).delegate().store.clone();
        let before = store.read(cx).values.clone();
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("0", "name"), false, false, cx);
            assert!(
                s.paste_text("Valid\tdefault\r\n\tstaging", window, cx)
                    .is_err()
            );
        });
        assert_eq!(store.read(cx).values, before);
        assert_eq!(grid.read(cx).error().as_deref(), Some("Name is required"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn editable_paste_sorted_filtered_keys_shared_views_and_native_handler(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler;
    use pintle_gpui_kit::components::table::{CellAddress, EditableTableDelegate};
    let (handle, workbench) = workbench(cx);
    let (source, other, store) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| w.split(first, SplitDirection::Down, window, cx))
                .unwrap();
            let source = resource(&workspace, first, cx);
            let other = resource(&workspace, second, cx);
            workspace.update(cx, |w, cx| w.select(first, window, cx));
            let grid = source.read(cx).editable.clone();
            let store = source.read(cx).table.read(cx).delegate().store.clone();
            grid.update(cx, |s, cx| {
                s.update_delegate(cx, |rows, cx| {
                    rows.query = "สมชาย".into();
                    rows.sort(
                        0,
                        gpui::component::table::ColumnSort::Descending,
                        window,
                        cx,
                    );
                });
            });
            let keys: Vec<_> = source.read(cx).table.read(cx).delegate().indices[..2].to_vec();
            grid.update(cx, |s, cx| {
                s.select_cell(
                    CellAddress::new(keys[0].to_string(), "name"),
                    false,
                    false,
                    cx,
                );
                s.paste(
                    gpui::ClipboardItem::new_string("Changed first\nChanged second".into()),
                    window,
                    cx,
                );
            });
            assert_eq!(store.read(cx).values[keys[0]][1].as_ref(), "Changed first");
            assert_eq!(store.read(cx).values[keys[1]][1].as_ref(), "Changed second");
            // Both edited rows disappeared from this view's filter; the snapshot
            // must still target the two original source rows, never their successors.
            assert_eq!(source.read(cx).table.read(cx).delegate().indices.len(), 248);
            assert!(grid.read(cx).selection().cells.is_empty());
            (source, other, store)
        })
        .unwrap();
    cx.update_window(handle, |_, _, cx| {
        assert_eq!(other.read(cx).table.read(cx).delegate().indices.len(), 1000);
        assert!(
            other
                .read(cx)
                .editable
                .read(cx)
                .selection()
                .cells
                .is_empty()
        );
        assert_eq!(other.read(cx).table.read(cx).delegate().store, store);
        assert_eq!(source.read(cx).table.read(cx).delegate().query, "สมชาย");
    })
    .unwrap();
}

#[gpui_kit::test]
fn terminal_lazy_sessions_shortcuts_and_panel_lifetime(cx: &mut TestAppContext) {
    use crate::inspector::InspectorTab;
    let (handle, workbench) = workbench(cx);
    let inspector = cx.update(|cx| workbench.read(cx).inspector.clone());
    let panel = cx.update(|cx| inspector.read(cx).terminal.clone());
    let first = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(panel.read(cx).session_count(), 0);
            window.click("inspector-terminal", cx);
            assert_eq!(panel.read(cx).session_count(), 1);
            assert!(window.find("terminal-view").visible());
            let first = panel.read(cx).active_test_session().unwrap();
            first.update(cx, |s, cx| s.feed_test_output(b"first-session", cx));
            let sidebar = workbench.read(cx).layout.read(cx).sidebar_visible;
            for key in [
                "ctrl-b",
                "ctrl-j",
                "ctrl-k",
                "ctrl-c",
                "ctrl-q",
                "left",
                "right",
                "home",
                "end",
                "tab",
                "shift-tab",
            ] {
                window.press(key, cx);
                assert!(panel.read(cx).is_focused(window, cx));
            }
            assert_eq!(
                first.read(cx).take_test_input(),
                b"\x02\n\x0b\x03\x11\x1b[D\x1b[C\x1b[H\x1b[F\t\x1b[Z"
            );
            assert_eq!(inspector.read(cx).tab, InspectorTab::Terminal);
            assert!(inspector.read(cx).layout.read(cx).panel_visible);
            assert_eq!(workbench.read(cx).layout.read(cx).sidebar_visible, sidebar);
            assert!(!window.has_active_dialog(cx));
            window.click("terminal-new", cx);
            assert_eq!(panel.read(cx).session_count(), 2);
            assert_ne!(panel.read(cx).active_test_session().unwrap(), first);
            window.click("terminal-close-session", cx);
            assert_eq!(panel.read(cx).active_test_session().unwrap(), first);
            assert!(first.read(cx).text().contains("first-session"));
            let height = window.find("inspector-panel").bounds().size.height;
            window.press("ctrl-`", cx);
            assert!(!inspector.read(cx).layout.read(cx).panel_visible);
            assert_eq!(
                first.read(cx).status(),
                pintle_gpui_kit::terminal::TerminalStatus::Running
            );
            window.press("ctrl-`", cx);
            assert!(inspector.read(cx).layout.read(cx).panel_visible);
            assert_eq!(window.find("inspector-panel").bounds().size.height, height);
            assert_eq!(panel.read(cx).active_test_session().unwrap(), first);
            window.click("inspector-output", cx);
            window.click("inspector-terminal", cx);
            assert!(first.read(cx).text().contains("first-session"));
            window.click("terminal-restart", cx);
            assert!(first.read(cx).text().trim().is_empty());
            window.click("terminal-close-session", cx);
            assert_eq!(panel.read(cx).session_count(), 0);
            assert!(window.find("terminal-empty").visible());
            window.click("inspector-output", cx);
            window.click("inspector-terminal", cx);
            assert_eq!(panel.read(cx).session_count(), 0);
            first
        })
        .unwrap();
    palette_step(cx, handle, |window, cx| window.press("cmd-k", cx));
    palette_step(cx, handle, |window, cx| window.input("New Terminal", cx));
    palette_step(cx, handle, |window, cx| window.press("enter", cx));
    cx.update_window(handle, |_, window, cx| {
        // Opening from the palette must dismiss the modal and focus the new shell.
        assert!(!window.has_active_dialog(cx));
        assert_eq!(panel.read(cx).session_count(), 1);
        let session = panel.read(cx).active_test_session().unwrap();
        assert_ne!(session, first);
        window.press("ctrl-c", cx);
        assert_eq!(session.read(cx).take_test_input(), b"\x03");
    })
    .unwrap();
}

#[gpui_kit::test]
fn terminal_close_shortcut_preserves_documents_and_focus(cx: &mut TestAppContext) {
    use pintle_gpui_kit::terminal::TerminalStatus;
    let (handle, workbench) = workbench(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let workspace = workbench.read(cx).workspace.clone();
        let documents = workspace.read(cx).groups(cx)[0].views.clone();
        let selected = workspace.read(cx).active_view(cx);
        assert!(selected.is_some());
        let inspector = workbench.read(cx).inspector.clone();
        let panel = inspector.read(cx).terminal.clone();
        window.press("ctrl-`", cx);
        let first = panel.read(cx).active_test_session().unwrap();
        window.click("terminal-new", cx);
        let second = panel.read(cx).active_test_session().unwrap();
        assert_ne!(first, second);
        window.press("cmd-w", cx);
        assert_eq!(second.read(cx).status(), TerminalStatus::Closed);
        assert_eq!(panel.read(cx).session_count(), 1);
        assert_eq!(panel.read(cx).active_test_session(), Some(first.clone()));
        assert!(panel.read(cx).is_focused(window, cx));
        window.press("tab", cx);
        assert_eq!(first.read(cx).take_test_input(), b"\t");
        window.press("cmd-w", cx);
        assert_eq!(first.read(cx).status(), TerminalStatus::Closed);
        assert_eq!(panel.read(cx).session_count(), 0);
        assert!(panel.read(cx).is_focused(window, cx));
        assert!(window.find("terminal-empty").visible());
        assert!(inspector.read(cx).layout.read(cx).panel_visible);
        window.press("cmd-w", cx);
        assert_eq!(panel.read(cx).session_count(), 0);
        assert_eq!(workspace.read(cx).groups(cx)[0].views, documents);
        assert_eq!(workspace.read(cx).active_view(cx), selected);
    })
    .unwrap();
}
