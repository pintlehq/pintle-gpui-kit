use super::*;
use pintle_gpui_kit::components::table::{CellAddress, EditableTableDelegate};

#[gpui_kit::test]
fn database_toolbar_stages_validates_saves_and_reverts_across_splits(cx: &mut TestAppContext) {
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
        window.within(scope).click("row-filter", cx);
        window.input("no matches", cx);
    });
    palette_step(cx, handle, |window, cx| {
        window.within(scope).click("db-add-row", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original.read(cx).table.read(cx).delegate().indices.len(),
            1001
        );
        assert_eq!(other.read(cx).table.read(cx).delegate().indices.len(), 1001);
        assert!(original.read(cx).filter.read(cx).value().is_empty());
        assert_eq!(
            original.read(cx).editable.read(cx).active_cell(),
            Some(CellAddress::new("1000", "name"))
        );
        assert_eq!(window.within(scope).find("cell-editor").value(), Some(""));
        window.within(scope).click("db-save", cx);
        assert!(
            original.read(cx).editable.read(cx).is_editing(),
            "blank name blocks Save"
        );
    });
    palette_step(cx, handle, |window, cx| window.input("New customer", cx));
    palette_step(cx, handle, |window, cx| {
        window.within(scope).click("db-save", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert!(!original.read(cx).editable.read(cx).is_editing());
        assert_eq!(
            other
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .value(&CellAddress::new("1000", "name"), cx)
                .as_ref(),
            "New customer"
        );
        let grid = original.read(cx).editable.clone();
        grid.update(cx, |grid, cx| {
            grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
            grid.select_cell(CellAddress::new("0", "email"), false, true, cx);
        });
        window.render_frame(cx);
    });
    palette_step(cx, handle, |window, cx| {
        window.within(scope).click("db-delete-rows", cx)
    });
    palette_step(cx, handle, |window, cx| {
        let rows = original.read(cx).table.read(cx).delegate();
        assert_eq!(
            rows.indices.len(),
            1000,
            "two selected cells delete one row"
        );
        assert_eq!(rows.row_key(0, cx).as_ref(), "1");
        assert_eq!(
            original.read(cx).editable.read(cx).active_cell(),
            Some(CellAddress::new("1", "name"))
        );
        // Cancel also discards an unfinished editor in another split.
        other.read(cx).editable.clone().update(cx, |grid, cx| {
            grid.select_cell(CellAddress::new("1", "name"), false, false, cx);
            grid.begin_edit(true, window, cx);
        });
    });
    palette_step(cx, handle, |window, cx| {
        window.within(scope).click("db-cancel", cx);
    });
    palette_step(cx, handle, |_, cx| {
        assert!(!other.read(cx).editable.read(cx).is_editing());
        let rows = original.read(cx).table.read(cx).delegate();
        assert_eq!(rows.indices.len(), 1001);
        assert_eq!(rows.row_key(0, cx).as_ref(), "0");
        assert_eq!(
            rows.value(&CellAddress::new("1000", "name"), cx).as_ref(),
            "New customer"
        );
    });
}

#[gpui_kit::test]
fn database_save_commits_other_split_and_drafts_survive_reopen(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let (workspace, first, original, other) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| w.split(first, SplitDirection::Down, window, cx))
                .unwrap();
            let original = resource(&workspace, first, cx);
            let other = resource(&workspace, second, cx);
            other.read(cx).editable.clone().update(cx, |grid, cx| {
                grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
                grid.begin_edit(true, window, cx);
            });
            (workspace, first, original, other)
        })
        .unwrap();
    palette_step(cx, handle, |window, cx| {
        window
            .within(("resource-view", original.entity_id()))
            .click("db-cancel", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(
            !other.read(cx).editable.read(cx).is_editing(),
            "first pointer press on Cancel must bypass the other split's unfinished editor"
        );
        other
            .read(cx)
            .editable
            .clone()
            .update(cx, |grid, cx| grid.begin_edit(true, window, cx));
        window.input("Saved through split", cx);
    });
    palette_step(cx, handle, |window, cx| {
        window
            .within(("resource-view", original.entity_id()))
            .click("db-save", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert!(!other.read(cx).editable.read(cx).is_editing());
        original.read(cx).editable.clone().update(cx, |grid, cx| {
            grid.update_delegate(cx, |rows, cx| {
                rows.commit(
                    &CellAddress::new("0", "name"),
                    "Retained draft".into(),
                    window,
                    cx,
                )
                .unwrap();
            })
        });
        workspace.update(cx, |w, cx| w.close(first, CloseScope::All, window, cx));
    });
    palette_step(cx, handle, |window, cx| {
        let id = workspace.update(cx, |w, cx| {
            w.open(
                Dataset::fixtures()[0].document.clone(),
                OpenMode::Permanent,
                window,
                cx,
            )
        });
        let reopened = resource(&workspace, id, cx);
        assert_eq!(
            reopened
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .value(&CellAddress::new("0", "name"), cx)
                .as_ref(),
            "Retained draft"
        );
        window.render_frame(cx);
        window
            .within(("resource-view", reopened.entity_id()))
            .click("db-cancel", cx);
    });
    palette_step(cx, handle, |_, cx| {
        assert_eq!(
            original
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .value(&CellAddress::new("0", "name"), cx)
                .as_ref(),
            "Saved through split"
        );
    });
}

#[gpui_kit::test]
fn kubernetes_multi_namespace_name_search_isolated_and_keyboard_dismissal(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let (original, other) = cx
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
        window.within(scope).click("namespace-trigger", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert!(window.find("namespace-menu").visible());
        window.click("namespace-option:staging", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original.read(cx).table.read(cx).delegate().indices.len(),
            300
        );
        assert_eq!(other.read(cx).table.read(cx).delegate().indices.len(), 1000);
        window.click("namespace-option:production", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original.read(cx).table.read(cx).delegate().indices.len(),
            500
        );
        window.press("escape", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(
            !window
                .find_all("namespace-menu")
                .iter()
                .any(|menu| menu.visible())
        );
        assert_eq!(
            window.within(scope).find("namespace-trigger").focused(),
            Some(true)
        );
        window.within(scope).click("row-filter", cx);
        window.input("STAGING", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(
            original
                .read(cx)
                .table
                .read(cx)
                .delegate()
                .indices
                .is_empty(),
            "search only matches resource names"
        );
        window.within(scope).click("clear-row-filter", cx);
    });
    palette_step(cx, handle, |window, cx| {
        window.within(scope).click("namespace-trigger", cx)
    });
    palette_step(cx, handle, |window, cx| window.click("namespace-clear", cx));
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            original.read(cx).table.read(cx).delegate().indices.len(),
            1000
        );
        window.press("escape", cx);
        window.within(scope).click("row-filter", cx);
        window.input("API-05", cx);
    });
    palette_step(cx, handle, |_, cx| {
        let rows = original.read(cx).table.read(cx).delegate();
        assert!(!rows.indices.is_empty());
        assert!(rows.indices.iter().all(|i| {
            rows.store.read(cx).values[*i][0]
                .to_lowercase()
                .contains("api-05")
        }));
    });
}

#[gpui_kit::test]
fn resource_headers_fit_theme_size_matrix_and_namespace_validation(cx: &mut TestAppContext) {
    for mode in [theme::ThemeMode::Light, theme::ThemeMode::Dark] {
        for (width, height) in [(1024., 680.), (1440., 900.)] {
            let (handle, workbench) = workbench(cx);
            cx.simulate_window_resize(handle, size(px(width), px(height)));
            let mut kubernetes = None;
            palette_step(cx, handle, |window, cx| {
                theme::set_mode(mode, cx);
                window.render_frame(cx);
                let header = window.find("resource-toolbar").bounds();
                assert_eq!(header.size.height, px(32.));
                for id in [
                    "db-add-row",
                    "db-delete-rows",
                    "db-save",
                    "db-cancel",
                    "row-filter",
                ] {
                    let control = window.find(id);
                    assert!(control.visible());
                    assert!(header.contains(&control.bounds().center()));
                }
                let workspace = workbench.read(cx).workspace.clone();
                let id = workspace.update(cx, |w, cx| {
                    w.open(
                        Dataset::fixtures()[3].document.clone(),
                        OpenMode::Permanent,
                        window,
                        cx,
                    )
                });
                kubernetes = Some(resource(&workspace, id, cx));
            });
            let view = kubernetes.unwrap();
            palette_step(cx, handle, |window, cx| {
                assert!(window.find_all("db-add-row").is_empty());
                assert_eq!(
                    window.find("row-filter").placeholder(),
                    Some("Search Pods…")
                );
                let header = window.find("resource-toolbar").bounds();
                assert!(header.contains(&window.find("namespace-trigger").bounds().center()));
                assert!(header.contains(&window.find("row-filter").bounds().center()));
                window.click("namespace-trigger", cx);
            });
            palette_step(cx, handle, |window, cx| {
                window.press("tab", cx); // Clear
                window.press("tab", cx); // default
                window.press("space", cx);
            });
            palette_step(cx, handle, |window, cx| {
                assert_eq!(view.read(cx).table.read(cx).delegate().indices.len(), 500);
                window.press("escape", cx);
                view.read(cx).editable.clone().update(cx, |grid, cx| {
                    grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
                    grid.begin_edit(true, window, cx);
                });
            });
            palette_step(cx, handle, |window, cx| {
                window.click("namespace-trigger", cx)
            });
            palette_step(cx, handle, |window, cx| {
                window.click("namespace-option:staging", cx)
            });
            palette_step(cx, handle, |window, cx| {
                let rows = view.read(cx).table.read(cx).delegate();
                assert_eq!(
                    rows.namespaces
                        .iter()
                        .map(|s| s.as_ref())
                        .collect::<Vec<_>>(),
                    vec!["default"]
                );
                assert_eq!(window.find("cell-editor").focused(), Some(true));
                assert!(view.read(cx).editable.read(cx).error().is_some());
                assert!(
                    !window
                        .find_all("namespace-menu")
                        .iter()
                        .any(|menu| menu.visible())
                );
            });
        }
    }
}

#[gpui_kit::test]
fn database_add_multiple_unfinished_records_keeps_partial_values_across_splits(
    cx: &mut TestAppContext,
) {
    let (handle, workbench) = workbench(cx);
    let (original, other, store) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| {
                    w.split(first, SplitDirection::Right, window, cx)
                })
                .unwrap();
            let original = resource(&workspace, first, cx);
            let other = resource(&workspace, second, cx);
            let store = original.read(cx).table.read(cx).delegate().store.clone();
            (original, other, store)
        })
        .unwrap();
    let scope = ("resource-view", original.entity_id());
    for index in 0..3 {
        palette_step(cx, handle, |window, cx| {
            window.within(scope).click("db-add-row", cx)
        });
        palette_step(cx, handle, |window, cx| {
            assert_eq!(store.read(cx).values.len(), 1001 + index);
            assert_eq!(store.read(cx).draft_summary(cx).0, index + 1);
            assert_eq!(
                original.read(cx).editable.read(cx).active_cell(),
                Some(CellAddress::new((1000 + index).to_string(), "name"))
            );
            assert_eq!(window.within(scope).find("cell-editor").value(), Some(""));
            assert_eq!(
                window.within(scope).find("cell-editor").focused(),
                Some(true)
            );
        });
    }
    palette_step(cx, handle, |window, cx| {
        window.input("Partially entered", cx)
    });
    palette_step(cx, handle, |window, cx| {
        window
            .within(("resource-view", other.entity_id()))
            .click("db-add-row", cx)
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(store.read(cx).values[1002][1].as_ref(), "Partially entered");
        assert_eq!(store.read(cx).values.len(), 1004);
        let rows = original.read(cx).table.read(cx).delegate();
        let keys: std::collections::BTreeSet<_> =
            (1000..1004).map(|row| rows.row_key(row, cx)).collect();
        let ids: std::collections::BTreeSet<_> = store.read(cx).values[1000..]
            .iter()
            .map(|row| row[0].clone())
            .collect();
        assert_eq!(keys.len(), 4);
        assert_eq!(ids.len(), 4);
        assert_eq!(other.read(cx).table.read(cx).delegate().indices.len(), 1004);
        assert_eq!(
            other.read(cx).editable.read(cx).active_cell(),
            Some(CellAddress::new("1003", "name"))
        );
        window.within(scope).click("db-save", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            store.read(cx).draft_summary(cx).0,
            4,
            "failed Save must retain every new record"
        );
        assert_eq!(
            original.read(cx).editable.read(cx).active_cell(),
            Some(CellAddress::new("1000", "name"))
        );
        assert_eq!(
            original.read(cx).editable.read(cx).error().as_deref(),
            Some("Name is required")
        );
        assert_eq!(
            window.within(scope).find("cell-editor").focused(),
            Some(true)
        );
        window.within(scope).click("db-cancel", cx);
    });
    palette_step(cx, handle, |_, cx| {
        assert_eq!(store.read(cx).values.len(), 1000);
        assert_eq!(store.read(cx).draft_summary(cx), (0, 0, 0, 0));
        assert!(!original.read(cx).editable.read(cx).is_editing());
        assert!(!other.read(cx).editable.read(cx).is_editing());
        assert!(workbench.read(cx).inspector.read(cx).details(cx).is_empty());
    });
}

#[gpui_kit::test]
fn database_save_reveals_hidden_missing_names_and_checkpoints_only_complete_drafts(
    cx: &mut TestAppContext,
) {
    let (handle, workbench) = workbench(cx);
    let (view, other, store) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| {
                    w.split(first, SplitDirection::Right, window, cx)
                })
                .unwrap();
            let view = resource(&workspace, first, cx);
            let other = resource(&workspace, second, cx);
            let store = view.read(cx).table.read(cx).delegate().store.clone();
            (view, other, store)
        })
        .unwrap();
    let scope = ("resource-view", view.entity_id());
    palette_step(cx, handle, |window, cx| {
        window.within(scope).double_click("editable-cell:0:1", cx);
        window.press("cmd-a", cx);
        window.input("   ", cx);
        window.press("tab", cx);
    });
    for _ in 0..2 {
        palette_step(cx, handle, |window, cx| {
            window.within(scope).click("db-add-row", cx)
        });
    }
    palette_step(cx, handle, |window, cx| {
        window.within(scope).click("row-filter", cx);
        window.input("José", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(view.read(cx).table.read(cx).delegate().indices.len(), 250);
        assert_eq!(store.read(cx).values[0][1].as_ref(), "   ");
        assert_eq!(store.read(cx).draft_summary(cx).0, 2);
        assert_eq!(store.read(cx).draft_summary(cx).1, 1);
        window.within(scope).click("db-save", cx);
    });
    for (key, value) in [
        ("0", "Existing fixed"),
        ("1000", "First draft"),
        ("1001", "Second draft"),
    ] {
        palette_step(cx, handle, |window, cx| {
            assert_eq!(
                view.read(cx).editable.read(cx).active_cell(),
                Some(CellAddress::new(key, "name"))
            );
            assert_eq!(
                view.read(cx).editable.read(cx).error().as_deref(),
                Some("Name is required")
            );
            assert!(view.read(cx).filter.read(cx).value().is_empty());
            assert_eq!(
                window.within(scope).find("cell-editor").focused(),
                Some(true)
            );
            assert_eq!(
                store.read(cx).draft_summary(cx).0,
                2,
                "an incomplete Save cannot checkpoint a subset"
            );
            window.press("cmd-a", cx);
            window.input(value, cx);
        });
        palette_step(cx, handle, |window, cx| {
            window.within(scope).click("db-save", cx)
        });
    }
    palette_step(cx, handle, |_, cx| {
        assert_eq!(store.read(cx).draft_summary(cx), (0, 0, 0, 0));
        assert!(!view.read(cx).editable.read(cx).is_editing());
        assert_eq!(other.read(cx).table.read(cx).delegate().indices.len(), 1002);
        assert_eq!(store.read(cx).values[0][1].as_ref(), "Existing fixed");
        assert_eq!(store.read(cx).values[1000][1].as_ref(), "First draft");
        assert_eq!(store.read(cx).values[1001][1].as_ref(), "Second draft");
    });
    palette_step(cx, handle, |window, cx| {
        view.read(cx).editable.clone().update(cx, |grid, cx| {
            grid.update_delegate(cx, |rows, cx| {
                rows.commit(&CellAddress::new("0", "name"), "".into(), window, cx)
                    .unwrap();
            })
        });
        window.within(scope).click("db-add-row", cx);
    });
    palette_step(cx, handle, |window, cx| {
        window.within(scope).click("db-cancel", cx)
    });
    palette_step(cx, handle, |_, cx| {
        assert_eq!(store.read(cx).values.len(), 1002);
        assert_eq!(store.read(cx).values[0][1].as_ref(), "Existing fixed");
        assert_eq!(store.read(cx).draft_summary(cx), (0, 0, 0, 0));
    });
}

#[gpui_kit::test]
fn database_blank_batch_and_paste_drafts_survive_sort_filter_and_reopen(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let (workspace, original, other, store, saved) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| {
                    w.split(first, SplitDirection::Right, window, cx)
                })
                .unwrap();
            let original = resource(&workspace, first, cx);
            let other = resource(&workspace, second, cx);
            let store = original.read(cx).table.read(cx).delegate().store.clone();
            let saved = store.read(cx).values.clone();
            workspace.update(cx, |w, cx| w.select(first, window, cx));
            (workspace, original, other, store, saved)
        })
        .unwrap();
    let grid = cx.update(|cx| original.read(cx).editable.clone());
    palette_step(cx, handle, |window, cx| {
        grid.update(cx, |grid, cx| {
            grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
            grid.select_cell(CellAddress::new("2", "name"), false, true, cx);
        });
        window.focus(&gpui::Focusable::focus_handle(&grid, cx), cx);
        window.press("f2", cx);
        window.press("cmd-a", cx);
        window.press("backspace", cx);
        window.press("enter", cx);
        assert!(!grid.read(cx).is_editing());
        assert!(grid.read(cx).error().is_none());
        assert!(store.read(cx).values[0][1].is_empty());
        assert!(store.read(cx).values[2][1].is_empty());
        grid.update(cx, |grid, cx| {
            grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
            grid.paste_text("ONLY_MATCH\tmail\n\tother", window, cx)
                .unwrap();
            grid.update_delegate(cx, |rows, cx| {
                rows.sort(
                    1,
                    gpui::component::table::ColumnSort::Descending,
                    window,
                    cx,
                )
            });
        });
        window
            .within(("resource-view", original.entity_id()))
            .click("row-filter", cx);
        window.input("ONLY_MATCH", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(original.read(cx).table.read(cx).delegate().indices.len(), 1);
        assert_eq!(other.read(cx).table.read(cx).delegate().indices.len(), 1000);
        assert_eq!(store.read(cx).values[0][1].as_ref(), "ONLY_MATCH");
        assert!(store.read(cx).values[1][1].is_empty());
        assert!(store.read(cx).values[2][1].is_empty());
        let ids: Vec<_> = workspace
            .read(cx)
            .groups(cx)
            .into_iter()
            .flat_map(|group| group.views)
            .collect();
        for id in ids {
            workspace.update(cx, |w, cx| w.close(id, CloseScope::One, window, cx));
        }
        assert!(workspace.read(cx).groups(cx).is_empty());
    });
    let mut reopened = None;
    palette_step(cx, handle, |window, cx| {
        let id = workspace.update(cx, |w, cx| {
            w.open(
                Dataset::fixtures()[0].document.clone(),
                OpenMode::Permanent,
                window,
                cx,
            )
        });
        reopened = Some(resource(&workspace, id, cx));
    });
    let reopened = reopened.unwrap();
    palette_step(cx, handle, |window, cx| {
        assert!(store.read(cx).values[1][1].is_empty());
        window
            .within(("resource-view", reopened.entity_id()))
            .click("db-save", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(
            reopened.read(cx).editable.read(cx).active_cell(),
            Some(CellAddress::new("1", "name"))
        );
        assert_eq!(
            reopened.read(cx).editable.read(cx).error().as_deref(),
            Some("Name is required")
        );
        window
            .within(("resource-view", reopened.entity_id()))
            .click("db-cancel", cx);
    });
    palette_step(cx, handle, |_, cx| assert_eq!(store.read(cx).values, saved));
}
