use super::*;
use pintle_gpui_kit::components::table::{CellAddress, EditableTableDelegate};

fn menu_labels(window: &gpui::Window) -> Vec<String> {
    window
        .find_all_by_role(gpui::Role::MenuItem)
        .into_iter()
        .filter_map(|item| item.label().map(str::to_owned))
        .collect()
}
fn choose(window: &mut gpui::Window, label: &str, cx: &mut gpui::App) {
    let item = window
        .find_all_by_role(gpui::Role::MenuItem)
        .into_iter()
        .find(|item| item.label() == Some(label))
        .unwrap_or_else(|| panic!("Missing menu item {label}: {:?}", menu_labels(window)));
    window
        .within("popup-menu")
        .click(item.path().last().unwrap().clone(), cx);
}

#[gpui_kit::test]
fn context_sidebar_target_open_split_copy_and_protected_nodes(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let workspace = cx.update(|cx| workbench.read(cx).workspace.clone());
    palette_step(cx, handle, |window, cx| {
        window.right_click("nav:db/local/shop/orders", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            workspace
                .read(cx)
                .groups(cx)
                .iter()
                .map(|g| g.views.len())
                .sum::<usize>(),
            1,
            "right click never opens a preview"
        );
        assert!(menu_labels(window).contains(&"Rename".to_owned()));
        choose(window, "Open in Split Right", cx);
    });
    palette_step(cx, handle, |window, cx| {
        let groups = workspace.read(cx).groups(cx);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups.iter().map(|g| g.views.len()).sum::<usize>(), 2);
        assert_eq!(
            workspace
                .read(cx)
                .document(workspace.read(cx).active_view(cx).unwrap(), cx)
                .unwrap()
                .title
                .as_ref(),
            "orders"
        );
        window.right_click("nav:local", cx);
    });
    palette_step(cx, handle, |window, cx| choose(window, "Copy Path", cx));
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "Database/Local connection"
        );
        window.right_click("nav:database", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(
            !menu_labels(window)
                .iter()
                .any(|label| label == "Rename" || label == "Delete")
        );
        choose(window, "Collapse", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(window.find_all("nav:local").is_empty());
        window.right_click("nav:foundation", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(
            !menu_labels(window)
                .iter()
                .any(|label| label == "Rename" || label == "Delete")
        );
        window.press("escape", cx);
        window.click("nav:database", cx);
        window.press("shift-f10", cx);
    });
    palette_step(cx, handle, |window, _| {
        assert!(window.find("popup-menu").visible())
    });
}

#[gpui_kit::test]
fn context_rename_updates_catalog_retained_views_and_palette(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let (workspace, first, second, original, other) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| w.split(first, SplitDirection::Down, window, cx))
                .unwrap();
            let original = resource(&workspace, first, cx);
            let other = resource(&workspace, second, cx);
            original.read(cx).editable.clone().update(cx, |grid, cx| {
                grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
                grid.begin_edit(true, window, cx);
            });
            (workspace, first, second, original, other)
        })
        .unwrap();
    palette_step(cx, handle, |window, cx| {
        window.input("Uncommitted customer", cx)
    });
    palette_step(cx, handle, |window, cx| {
        window.right_click("nav:db/local/shop/customers", cx)
    });
    palette_step(cx, handle, |window, cx| choose(window, "Rename", cx));
    palette_step(cx, handle, |window, cx| {
        window.click("rename-node-input", cx);
        window.press("cmd-a", cx);
        window.input("orders", cx);
        window.press("enter", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(window.find("rename-node-error").visible());
        window.press("cmd-a", cx);
        window.input("clients", cx);
        window.press("enter", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(!window.has_active_dialog(cx));
        assert_eq!(
            resource(&workspace, first, cx).entity_id(),
            original.entity_id()
        );
        assert_eq!(
            resource(&workspace, second, cx).entity_id(),
            other.entity_id()
        );
        assert_eq!(
            workspace
                .read(cx)
                .document(first, cx)
                .unwrap()
                .title
                .as_ref(),
            "clients"
        );
        assert_eq!(
            workspace
                .read(cx)
                .document(second, cx)
                .unwrap()
                .title
                .as_ref(),
            "clients"
        );
        assert!(original.read(cx).editable.read(cx).is_editing());
        assert_eq!(
            window
                .within(("resource-view", original.entity_id()))
                .find("cell-editor")
                .value(),
            Some("Uncommitted customer")
        );
        window.right_click("nav:local", cx);
    });
    palette_step(cx, handle, |window, cx| choose(window, "Rename", cx));
    palette_step(cx, handle, |window, cx| {
        window.click("rename-node-input", cx);
        window.press("cmd-a", cx);
        window.input("Remote", cx);
        window.click("rename-node-confirm", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            workspace
                .read(cx)
                .document(first, cx)
                .unwrap()
                .context
                .as_ref(),
            "Remote / shop / public"
        );
        original
            .read(cx)
            .editable
            .clone()
            .update(cx, |grid, cx| grid.cancel_edit(window, cx));
        window.press("cmd-k", cx);
    });
    palette_step(cx, handle, |window, cx| window.input("clients Remote", cx));
    palette_step(cx, handle, |window, cx| {
        // Search either token separately: Kit treats the complete query as a substring.
        window.press("cmd-a", cx);
        window.input("clients", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(!window.find_all("command-empty").iter().any(|e| e.visible()));
        window.press("enter", cx);
    });
    palette_step(cx, handle, |_, cx| {
        assert_eq!(
            workspace
                .read(cx)
                .document(workspace.read(cx).active_view(cx).unwrap(), cx)
                .unwrap()
                .title
                .as_ref(),
            "clients"
        )
    });
}

#[gpui_kit::test]
fn context_delete_subtree_is_atomic_with_unrelated_validation(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let (workspace, original, pods) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let original = resource(&workspace, first, cx);
            workspace.update(cx, |w, cx| {
                w.split(first, SplitDirection::Right, window, cx)
            });
            let pods = workspace.update(cx, |w, cx| {
                w.open(
                    Dataset::fixtures()[3].document.clone(),
                    OpenMode::Permanent,
                    window,
                    cx,
                )
            });
            let pods = resource(&workspace, pods, cx);
            pods.read(cx).editable.clone().update(cx, |grid, cx| {
                grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
                grid.begin_edit(true, window, cx);
            });
            (workspace, original, pods)
        })
        .unwrap();
    palette_step(cx, handle, |window, cx| {
        window.right_click("nav:public", cx)
    });
    palette_step(cx, handle, |window, cx| choose(window, "Delete", cx));
    palette_step(cx, handle, |window, cx| {
        assert!(window.find("delete-node-summary").visible());
        window.click("delete-node-cancel", cx);
    });
    palette_step(cx, handle, |window, cx| {
        window.right_click("nav:public", cx)
    });
    palette_step(cx, handle, |window, cx| choose(window, "Delete", cx));
    palette_step(cx, handle, |window, cx| {
        window.click("delete-node-confirm", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert!(workbench.read(cx).catalog.read(cx).node("public").is_some());
        assert_eq!(
            workspace
                .read(cx)
                .groups(cx)
                .iter()
                .map(|g| g.views.len())
                .sum::<usize>(),
            3
        );
        assert!(pods.read(cx).editable.read(cx).error().is_some());
        pods.read(cx)
            .editable
            .clone()
            .update(cx, |grid, cx| grid.cancel_edit(window, cx));
        original.read(cx).editable.clone().update(cx, |grid, cx| {
            grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
            grid.begin_edit(true, window, cx);
        });
        window.right_click("nav:public", cx);
    });
    palette_step(cx, handle, |window, cx| choose(window, "Delete", cx));
    palette_step(cx, handle, |window, cx| window.press("enter", cx));
    palette_step(cx, handle, |window, cx| {
        window.simulate_next_frame(cx);
        assert!(workbench.read(cx).catalog.read(cx).node("public").is_none());
        assert_eq!(workbench.read(cx).catalog.read(cx).sources.len(), 3);
        assert_eq!(
            workspace
                .read(cx)
                .groups(cx)
                .iter()
                .map(|g| g.views.len())
                .sum::<usize>(),
            1
        );
        assert!(!original.read(cx).editable.read(cx).is_editing());
        assert!(window.find_all("nav:public").is_empty());
        window.press("cmd-k", cx);
    });
    palette_step(cx, handle, |window, cx| window.input("customers", cx));
    palette_step(cx, handle, |window, _| {
        assert!(window.find("command-empty").visible())
    });
}

#[gpui_kit::test]
fn context_table_selection_copy_rows_paste_and_cancel_invalid_split(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let (original, other) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| w.split(first, SplitDirection::Down, window, cx))
                .unwrap();
            (
                resource(&workspace, first, cx),
                resource(&workspace, second, cx),
            )
        })
        .unwrap();
    let scope = ("resource-view", original.entity_id());
    palette_step(cx, handle, |window, cx| {
        original.read(cx).editable.clone().update(cx, |grid, cx| {
            grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
            grid.select_cell(CellAddress::new("1", "email"), false, true, cx);
        });
        window.within(scope).right_click("editable-cell:0:1", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original.read(cx).editable.read(cx).selection().cells.len(),
            2
        );
        choose(window, "Copy Rows", cx);
    });
    palette_step(cx, handle, |window, cx| {
        let clipboard = cx.read_from_clipboard().unwrap().text().unwrap();
        assert_eq!(clipboard.lines().count(), 2);
        assert!(clipboard.starts_with("1\tสมชาย 1\t"));
        window.within(scope).right_click("editable-cell:2:1", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original.read(cx).editable.read(cx).active_cell(),
            Some(CellAddress::new("2", "name"))
        );
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("Pasted name".to_owned()));
        choose(window, "Paste", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .value(&CellAddress::new("2", "name"), cx)
                .as_ref(),
            "Pasted name"
        );
        other.read(cx).editable.clone().update(cx, |grid, cx| {
            grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
            grid.begin_edit(true, window, cx);
        });
        window.within(scope).right_click("editable-cell:1:1", cx);
    });
    palette_step(cx, handle, |window, cx| {
        choose(window, "Cancel Changes", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert!(!other.read(cx).editable.read(cx).is_editing());
        assert_ne!(
            original
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .value(&CellAddress::new("2", "name"), cx)
                .as_ref(),
            "Pasted name"
        );
        window.focus(
            &gpui::Focusable::focus_handle(&original.read(cx).editable, cx),
            cx,
        );
        window.press("shift-f10", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(window.find("popup-menu").visible());
        window.press("menu", cx);
    });
    palette_step(cx, handle, |window, cx| window.press("escape", cx));
    palette_step(cx, handle, |window, cx| {
        assert!(gpui::Focusable::focus_handle(&original.read(cx).editable, cx).is_focused(window))
    });
}

#[gpui_kit::test]
fn context_kubernetes_stable_target_namespace_and_geometry(cx: &mut TestAppContext) {
    for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
        for (width, height) in [(1024., 680.), (1440., 900.)] {
            let (handle, workbench) = workbench(cx);
            cx.simulate_window_resize(handle, size(px(width), px(height)));
            let mut pods = None;
            palette_step(cx, handle, |window, cx| {
                theme::set_mode(mode, cx);
                let workspace = workbench.read(cx).workspace.clone();
                let id = workspace.update(cx, |w, cx| {
                    w.open(
                        Dataset::fixtures()[3].document.clone(),
                        OpenMode::Permanent,
                        window,
                        cx,
                    )
                });
                pods = Some(resource(&workspace, id, cx));
                window.right_click("editable-cell:0:1", cx);
            });
            let pods = pods.unwrap();
            palette_step(cx, handle, |window, cx| {
                let bounds = window.find("popup-menu").bounds();
                assert!(bounds.left() >= px(0.) && bounds.right() <= px(width));
                assert!(bounds.top() >= px(0.) && bounds.bottom() <= px(height));
                assert!(
                    !menu_labels(window)
                        .iter()
                        .any(|label| label == "Delete Selected Rows" || label == "Save Changes")
                );
                pods.read(cx).editable.clone().update(cx, |grid, cx| {
                    grid.update_delegate(cx, |rows, cx| {
                        rows.sort(
                            0,
                            gpui::component::table::ColumnSort::Descending,
                            window,
                            cx,
                        )
                    })
                });
                choose(window, "Copy Name", cx);
            });
            palette_step(cx, handle, |window, cx| {
                assert_eq!(
                    cx.read_from_clipboard().unwrap().text().unwrap(),
                    "api-0001",
                    "menu target follows stable keys after sorting"
                );
                window.right_click("editable-cell:0:1", cx);
            });
            palette_step(cx, handle, |window, cx| {
                choose(window, "Filter to This Namespace", cx)
            });
            palette_step(cx, handle, |window, cx| {
                assert_eq!(pods.read(cx).table.read(cx).delegate().indices.len(), 200);
                assert!(
                    pods.read(cx)
                        .table
                        .read(cx)
                        .delegate()
                        .namespaces
                        .contains("production")
                );
                window.click("row-filter", cx);
                window.input("no matches", cx);
            });
            palette_step(cx, handle, |window, cx| {
                window.click_with_options(
                    "editable-table",
                    gpui::test::ClickOptions::new()
                        .with_button(gpui::MouseButton::Right)
                        .with_offset(gpui::point(px(60.), px(100.))),
                    cx,
                )
            });
            palette_step(cx, handle, |window, cx| choose(window, "Clear Filters", cx));
            palette_step(cx, handle, |_, cx| {
                assert_eq!(pods.read(cx).table.read(cx).delegate().indices.len(), 1000)
            });
            palette_step(cx, handle, |window, cx| {
                pods.read(cx).table.clone().update(cx, |table, cx| {
                    table.scroll_to_row(999, cx);
                    table.scroll_to_col(4, cx);
                });
                window.render_frame(cx);
                window.right_click("editable-cell:999:4", cx);
            });
            palette_step(cx, handle, |window, cx| {
                let bounds = window.find("popup-menu").bounds();
                assert!(
                    bounds.right() <= px(width) && bounds.bottom() <= px(height),
                    "edge menus must fit the window"
                );
                choose(window, "Copy Namespace", cx);
            });
            palette_step(cx, handle, |_, cx| {
                assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "default")
            });
        }
    }
}

#[gpui_kit::test]
fn context_database_background_crud_and_copy_after_filter(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let original = cx.update(|cx| {
        let workspace = &workbench.read(cx).workspace;
        resource(workspace, workspace.read(cx).active_view(cx).unwrap(), cx)
    });
    palette_step(cx, handle, |window, cx| {
        window.click("row-filter", cx);
        window.input("no matches", cx);
    });
    palette_step(cx, handle, |window, cx| {
        window.click_with_options(
            "editable-table",
            gpui::test::ClickOptions::new()
                .with_button(gpui::MouseButton::Right)
                .with_offset(gpui::point(px(60.), px(100.))),
            cx,
        )
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            menu_labels(window),
            vec!["Add Row", "Save Changes", "Cancel Changes"]
        );
        choose(window, "Add Row", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original.read(cx).table.read(cx).delegate().indices.len(),
            1001
        );
        assert!(original.read(cx).editable.read(cx).is_editing());
        window.right_click("editable-cell:1000:0", cx);
    });
    palette_step(cx, handle, |window, cx| choose(window, "Save Changes", cx));
    palette_step(cx, handle, |window, cx| {
        assert!(original.read(cx).editable.read(cx).error().is_some());
        window.input("Context customer", cx);
        window.right_click("editable-cell:1000:0", cx);
    });
    palette_step(cx, handle, |window, cx| choose(window, "Save Changes", cx));
    palette_step(cx, handle, |window, cx| {
        assert!(!original.read(cx).editable.read(cx).is_editing());
        window.right_click("editable-cell:1000:1", cx);
    });
    palette_step(cx, handle, |window, cx| {
        choose(window, "Delete Selected Rows", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original.read(cx).table.read(cx).delegate().indices.len(),
            1000
        );
        window.right_click("editable-cell:999:1", cx);
    });
    palette_step(cx, handle, |window, cx| {
        choose(window, "Cancel Changes", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original.read(cx).table.read(cx).delegate().indices.len(),
            1001
        );
        original
            .read(cx)
            .table
            .clone()
            .update(cx, |table, cx| table.scroll_to_row(0, cx));
        window.render_frame(cx);
        window.right_click("editable-cell:0:1", cx);
    });
    palette_step(cx, handle, |window, cx| {
        original.read(cx).editable.clone().update(cx, |grid, cx| {
            grid.update_delegate(cx, |rows, cx| {
                rows.query = "yuki".into();
                rows.rebuild(cx);
            })
        });
        choose(window, "Copy", cx);
    });
    palette_step(cx, handle, |_, cx| {
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "สมชาย 1")
    });
}

#[gpui_kit::test]
fn context_native_editor_menu_and_workspace_close_policies(cx: &mut TestAppContext) {
    use pintle_gpui_kit::shell::workspace::ClosePolicy;
    let (handle, workbench) = workbench(cx);
    let (workspace, id, original) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let initial = workspace.read(cx).active_view(cx).unwrap();
            let id = workspace.update(cx, |w, cx| {
                w.open(
                    Dataset::fixtures()[3].document.clone(),
                    OpenMode::Permanent,
                    window,
                    cx,
                )
            });
            workspace.update(cx, |w, cx| w.close(initial, CloseScope::One, window, cx));
            let original = resource(&workspace, id, cx);
            original.read(cx).editable.clone().update(cx, |grid, cx| {
                grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
                grid.begin_edit(true, window, cx);
            });
            (workspace, id, original)
        })
        .unwrap();
    palette_step(cx, handle, |window, cx| {
        window.right_click("cell-editor", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert!(!menu_labels(window).contains(&"Add Row".to_owned()));
        assert!(original.read(cx).editable.read(cx).is_editing());
        window.press("escape", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(original.read(cx).editable.read(cx).is_editing());
        let mut document = workspace.read(cx).document(id, cx).unwrap();
        document.title = "Pods alias".into();
        document.context = "Renamed context".into();
        workspace.update(cx, |w, cx| w.update_document_metadata(&document, cx));
        assert_eq!(
            resource(&workspace, id, cx).entity_id(),
            original.entity_id()
        );
        assert!(!workspace.update(cx, |w, cx| w.close_resources(
            &[document.key.clone()],
            ClosePolicy::Validate,
            window,
            cx
        )));
        assert!(workspace.read(cx).document(id, cx).is_some());
        assert!(workspace.update(cx, |w, cx| w.close_resources(
            &[document.key],
            ClosePolicy::Discard,
            window,
            cx
        )));
        assert!(workspace.read(cx).groups(cx).is_empty());
    });
}

#[gpui_kit::test]
fn context_database_add_repeats_without_required_names_and_save_keeps_all_drafts(
    cx: &mut TestAppContext,
) {
    let (handle, workbench) = workbench(cx);
    let view = cx.update(|cx| {
        let workspace = workbench.read(cx).workspace.clone();
        resource(&workspace, workspace.read(cx).active_view(cx).unwrap(), cx)
    });
    for (index, menu_row) in [(0, 0), (1, 1000), (2, 1001)] {
        palette_step(cx, handle, |window, cx| {
            window.right_click(format!("editable-cell:{menu_row}:0"), cx)
        });
        palette_step(cx, handle, |window, cx| choose(window, "Add Row", cx));
        palette_step(cx, handle, |window, cx| {
            assert_eq!(
                view.read(cx).table.read(cx).delegate().indices.len(),
                1001 + index
            );
            assert_eq!(
                view.read(cx).editable.read(cx).active_cell(),
                Some(CellAddress::new((1000 + index).to_string(), "name"))
            );
            assert_eq!(window.find("cell-editor").value(), Some(""));
        });
    }
    palette_step(cx, handle, |window, cx| {
        window.right_click("editable-cell:1002:0", cx)
    });
    palette_step(cx, handle, |window, cx| choose(window, "Save Changes", cx));
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            view.read(cx)
                .table
                .read(cx)
                .delegate()
                .store
                .read(cx)
                .draft_summary(cx)
                .0,
            3
        );
        assert_eq!(
            view.read(cx).editable.read(cx).active_cell(),
            Some(CellAddress::new("1000", "name"))
        );
        assert_eq!(
            view.read(cx).editable.read(cx).error().as_deref(),
            Some("Name is required")
        );
        window.right_click("editable-cell:1000:0", cx);
    });
    palette_step(cx, handle, |window, cx| {
        choose(window, "Cancel Changes", cx)
    });
    palette_step(cx, handle, |_, cx| {
        assert_eq!(view.read(cx).table.read(cx).delegate().indices.len(), 1000);
        assert_eq!(
            view.read(cx)
                .table
                .read(cx)
                .delegate()
                .store
                .read(cx)
                .draft_summary(cx),
            (0, 0, 0, 0)
        );
    });
}
