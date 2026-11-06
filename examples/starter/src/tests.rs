use super::*;
use gpui::{TestAppContext, test::TestWindowExt};
use pintle_gpui_kit::{
    components::table::CellAddress,
    shell::workspace::{CloseScope, SplitDirection, ViewId},
};

fn open(cx: &mut TestAppContext) -> (gpui::AnyWindowHandle, Entity<Starter>) {
    cx.update(|cx| {
        initialize(cx);
        gpui::open_window(window_options(cx), cx, |window, cx| {
            cx.new(|cx| Starter::new(window, cx))
        })
        .unwrap()
    })
}
fn page(workspace: &Entity<Workspace>, id: ViewId, cx: &App) -> Entity<TablePage> {
    workspace
        .read(cx)
        .page(id, cx)
        .unwrap()
        .downcast::<TablePage>()
        .unwrap()
}

#[gpui_kit::test]
fn starter_tree_opens_tabs_and_theme_keeps_views(cx: &mut TestAppContext) {
    let (handle, app) = open(cx);
    let (workspace, initial) = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let workspace = app.read(cx).workspace.clone();
            let initial = workspace.read(cx).active_view(cx).unwrap();
            assert!(!workspace.read(cx).is_preview(initial, cx));
            let tree = app.read(cx).tree.clone();
            tree.update(cx, |tree, cx| tree.set_query("Team", cx));
            window.render_frame(cx);
            window.click("nav:team", cx);
            (workspace, initial)
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let team = workspace.read(cx).active_view(cx).unwrap();
        assert_ne!(initial, team);
        assert_eq!(workspace.read(cx).document(team, cx).unwrap().title, "Team");
        assert!(workspace.read(cx).is_preview(team, cx));
        workspace.update(cx, |w, cx| w.keep_open(team, cx));
        assert_eq!(workspace.read(cx).groups(cx)[0].views.len(), 2);
        let mode = cx.theme().mode;
        window.click("toggle-theme", cx);
        assert_ne!(cx.theme().mode, mode);
        assert_eq!(workspace.read(cx).active_view(cx), Some(team));
        assert_eq!(
            page(&workspace, initial, cx)
                .read(cx)
                .grid
                .read(cx)
                .table()
                .read(cx)
                .delegate()
                .visible
                .len(),
            1000
        );
    })
    .unwrap();
    for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
        for (width, height) in [(1280., 800.), (960., 640.)] {
            cx.simulate_window_resize(handle, size(px(width), px(height)));
            cx.update_window(handle, |_, window, cx| {
                theme::set_mode(mode, cx);
                window.render_frame(cx);
                let group = workspace.read(cx).groups(cx)[0].id;
                let tabs = window.within(("workspace-tabs", group.as_u64()));
                assert_eq!(tabs.find_all_by_role(gpui::Role::Tab).len(), 2);
                assert_eq!(tabs.find_by_label("Contacts").bounds().size.height, px(32.));
                assert_eq!(tabs.find_by_label("Team").selected(), Some(true));
                for id in [initial, workspace.read(cx).active_view(cx).unwrap()] {
                    assert!(tabs.find(("close-view", id.0.as_u64())).visible());
                }
            })
            .unwrap();
        }
    }
}

#[gpui_kit::test]
fn starter_paste_validation_shared_split_and_close_guard(cx: &mut TestAppContext) {
    let (handle, app) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        let workspace = app.read(cx).workspace.clone();
        let first = workspace.read(cx).active_view(cx).unwrap();
        let second = workspace
            .update(cx, |w, cx| w.split(first, SplitDirection::Down, window, cx))
            .unwrap();
        let source = page(&workspace, first, cx);
        let duplicate = page(&workspace, second, cx);
        workspace.update(cx, |w, cx| w.select(first, window, cx));
        window.render_frame(cx);
        let grid = source.read(cx).grid.clone();
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("contacts-0001", "name"), false, false, cx);
            s.paste_text(
                "Starter ไทย\tstarter@example.test\r\nJosé\tjose@example.test",
                window,
                cx,
            )
            .unwrap();
        });
        let store = grid.read(cx).table().read(cx).delegate().store.clone();
        assert_eq!(store.read(cx).records["contacts-0001"][1], "Starter ไทย");
        assert_eq!(
            duplicate
                .read(cx)
                .grid
                .read(cx)
                .table()
                .read(cx)
                .delegate()
                .store,
            store
        );
        assert!(
            duplicate
                .read(cx)
                .grid
                .read(cx)
                .selection()
                .cells
                .is_empty()
        );
        let before = store.read(cx).records.clone();
        grid.update(cx, |s, cx| {
            assert!(
                s.paste_text("Valid\tvalid@example.test\n\tbad@example.test", window, cx)
                    .is_err()
            )
        });
        assert_eq!(store.read(cx).records, before);
        // The dropdown remains Foundation's closed searchable cell editor.
        grid.update(cx, |s, cx| {
            s.select_cell(
                CellAddress::new("contacts-0001", "status"),
                false,
                false,
                cx,
            )
        });
        window.focus(&gpui::Focusable::focus_handle(&grid, cx), cx);
        window.press("alt-down", cx);
        window.input("inactive", cx);
        window.press("enter", cx);
        assert_eq!(store.read(cx).records["contacts-0001"][3], "Inactive");
        grid.update(cx, |s, cx| {
            s.select_cell(CellAddress::new("contacts-0001", "name"), false, false, cx)
        });
        window.press("f2", cx);
        window.press("cmd-a", cx);
        window.press("backspace", cx);
        workspace.update(cx, |w, cx| w.close(first, CloseScope::One, window, cx));
        assert_eq!(workspace.read(cx).active_view(cx), Some(first));
        assert!(grid.read(cx).is_editing());
        assert_eq!(grid.read(cx).error().as_deref(), Some("Name is required"));
        window.press("escape", cx);
        workspace.update(cx, |w, cx| w.close(first, CloseScope::One, window, cx));
        assert_eq!(workspace.read(cx).active_view(cx), Some(second));
    })
    .unwrap();
}

#[gpui_kit::test]
fn starter_filter_validation_and_minimum_window(cx: &mut TestAppContext) {
    let (handle, app) = open(cx);
    cx.simulate_window_resize(handle, size(px(960.), px(640.)));
    let grid = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let workspace = app.read(cx).workspace.clone();
            let view = page(&workspace, workspace.read(cx).active_view(cx).unwrap(), cx);
            window.click("table-filter", cx);
            window.input("สมชาย", cx);
            view.read(cx).grid.clone()
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(grid.read(cx).table().read(cx).delegate().visible.len(), 250);
        assert!(window.find("table-filter").visible());
        assert!(grid.read(cx).table().read(cx).visible_range().rows().len() < 60);
    })
    .unwrap();
}
