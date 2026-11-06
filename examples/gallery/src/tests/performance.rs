//! Manual CPU workloads. Event delivery and draws are included; GPU/vsync are not.
use super::*;
use pintle_gpui_kit::components::table::CellAddress;
use std::time::Instant;

fn measure(label: &str, mut operation: impl FnMut(usize)) {
    let mut samples = Vec::with_capacity(30);
    for i in 0..35 {
        let start = Instant::now();
        operation(i);
        if i >= 5 {
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
    }
    samples.sort_by(f64::total_cmp);
    eprintln!(
        "workload={label} median={:.3}ms p95={:.3}ms samples=30",
        samples[15], samples[28]
    );
}

#[gpui_kit::test]
#[ignore = "manual CPU workloads: run alone in dev/release, three times each"]
fn workbench_cpu_workloads(cx: &mut TestAppContext) {
    for (width, height) in [(1024., 680.), (1440., 900.)] {
        let (handle, workbench) = workbench(cx);
        cx.simulate_window_resize(handle, size(px(width), px(height)));
        let (workspace, views, original) = cx
            .update_window(handle, |_, window, cx| {
                let workspace = workbench.read(cx).workspace.clone();
                let first = workspace.read(cx).active_view(cx).unwrap();
                let mut views = vec![first];
                for dataset in Dataset::fixtures().iter().skip(1) {
                    views.push(workspace.update(cx, |w, cx| {
                        w.open(dataset.document.clone(), OpenMode::Permanent, window, cx)
                    }));
                }
                for page in Page::ALL {
                    views.push(workspace.update(cx, |w, cx| {
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
                    }));
                }
                workspace.update(cx, |w, cx| {
                    w.split(first, SplitDirection::Right, window, cx);
                    w.select(first, window, cx);
                });
                let original = resource(&workspace, first, cx);
                (workspace, views, original)
            })
            .unwrap();
        let inspector = cx.update(|cx| workbench.read(cx).inspector.clone());
        palette_step(cx, handle, |_, cx| {
            inspector.update(cx, |inspector, cx| {
                for i in 0..500 {
                    inspector.record(format!("Activity {i}: retained resource event"), cx);
                }
            })
        });

        measure(&format!("{width:.0}x{height:.0}/palette"), |_| {
            palette_step(cx, handle, |window, cx| window.press("cmd-k", cx));
            palette_step(cx, handle, |window, cx| window.input("public", cx));
            palette_step(cx, handle, |window, cx| window.press("down", cx));
            palette_step(cx, handle, |window, cx| window.press("escape", cx));
            palette_step(cx, handle, |window, cx| window.press("escape", cx));
        });
        measure(&format!("{width:.0}x{height:.0}/table"), |i| {
            palette_step(cx, handle, |window, cx| {
                original
                    .read(cx)
                    .table
                    .clone()
                    .update(cx, |table, cx| table.scroll_to_row((i * 7) % 950, cx));
                original.read(cx).editable.clone().update(cx, |grid, cx| {
                    grid.select_cell(
                        CellAddress::new(((i * 7) % 950).to_string(), "name"),
                        false,
                        false,
                        cx,
                    )
                });
                window.render_frame(cx);
            });
            palette_step(cx, handle, |_, _| {});
        });
        measure(&format!("{width:.0}x{height:.0}/tabs"), |i| {
            palette_step(cx, handle, |window, cx| {
                workspace.update(cx, |w, cx| w.select(views[i % views.len()], window, cx))
            });
            palette_step(cx, handle, |_, _| {});
        });
        palette_step(cx, handle, |window, cx| {
            workspace.update(cx, |w, cx| w.select(views[0], window, cx))
        });
        measure(&format!("{width:.0}x{height:.0}/divider"), |i| {
            palette_step(cx, handle, |window, cx| {
                let at = window
                    .within("workspace-sidebar")
                    .find_all(("resizable-handle", 0usize))
                    .into_iter()
                    .min_by_key(|handle| handle.path().len())
                    .unwrap()
                    .bounds()
                    .center();
                let to = at + gpui::point(px(if i % 2 == 0 { 12. } else { -12. }), px(0.));
                window.drag(at, to, cx);
            });
        });
        palette_step(cx, handle, |window, cx| {
            window.click("inspector-output", cx)
        });
        measure(&format!("{width:.0}x{height:.0}/output500"), |_| {
            palette_step(cx, handle, |window, cx| {
                inspector.update(cx, |inspector, cx| inspector.record("Latest activity", cx));
                window.render_frame(cx);
            });
            palette_step(cx, handle, |_, _| {});
        });
    }
}

#[gpui_kit::test]
fn selection_updates_details_without_invalidating_workbench(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};
    let (handle, workbench) = workbench(cx);
    palette_step(cx, handle, |_, _| {});
    let count = Rc::new(Cell::new(0));
    let notifications = count.clone();
    let _subscription = cx.update(|cx| {
        cx.observe(&workbench, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    for row in [0, 1, 1, 2] {
        palette_step(cx, handle, |window, cx| {
            window.click(format!("editable-cell:{row}:1"), cx)
        });
    }
    cx.update(|cx| {
        assert_eq!(
            count.get(),
            0,
            "cell selection must not rebuild the window chrome"
        );
        assert_eq!(
            workbench.read(cx).inspector.read(cx).details(cx)[1]
                .1
                .as_ref(),
            "Yuki 3"
        );
    });
}

#[gpui_kit::test]
fn palette_keeps_geometry_when_results_change(cx: &mut TestAppContext) {
    let (handle, _) = workbench(cx);
    palette_step(cx, handle, |window, cx| window.press("cmd-k", cx));
    let mut bounds = None;
    let mut input_bounds = None;
    for query in ["public", "no-such-resource", ""] {
        palette_step(cx, handle, |window, cx| {
            let current = window.find("workspace-command-palette").bounds();
            let input = window.find_by_label(PALETTE_INPUT).bounds();
            assert_eq!(current.size.height, px(360.));
            assert_eq!(*bounds.get_or_insert(current), current);
            assert_eq!(*input_bounds.get_or_insert(input), input);
            window.press("cmd-a", cx);
            window.press("backspace", cx);
            if !query.is_empty() {
                window.input(query, cx);
            }
        });
    }
}

#[gpui_kit::test]
fn output_virtualizes_history_and_keeps_scroll_offset(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let inspector = cx.update(|cx| workbench.read(cx).inspector.clone());
    palette_step(cx, handle, |window, cx| {
        inspector.update(cx, |i, cx| {
            for entry in 0..550 {
                i.record(format!("Event {entry}"), cx);
            }
        });
        window.click("inspector-output", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(inspector.read(cx).entries.len(), 500);
        let visible_entries = window.within("inspector-output-content").elements().into_iter()
            .filter(|element| matches!(element.path().last(), Some(gpui::ElementId::NamedInteger(name, _)) if name.as_ref() == "output-entry"))
            .count();
        assert!(
            visible_entries > 0 && visible_entries < 30,
            "{visible_entries} rendered entries"
        );
        window.scroll(
            "inspector-output-content",
            gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-160.))),
            cx,
        );
    });
    let offset = cx.update(|cx| inspector.read(cx).output_offset());
    assert!(offset.y < px(0.));
    palette_step(cx, handle, |_, cx| {
        inspector.update(cx, |i, cx| i.record("Newest", cx))
    });
    cx.update(|cx| assert_eq!(inspector.read(cx).output_offset(), offset));
}

#[gpui_kit::test]
fn output_preserves_multiline_activity_and_clear(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let inspector = cx.update(|cx| workbench.read(cx).inspector.clone());
    palette_step(cx, handle, |window, cx| {
        window.click("inspector-output", cx);
        window.click("clear-output", cx);
        inspector.update(cx, |i, cx| {
            i.record("first line\nsecond line", cx);
            i.record("newest", cx);
        });
    });
    palette_step(cx, handle, |window, cx| {
        assert_eq!(inspector.read(cx).entries.len(), 2);
        let first = window.find(("output-entry", 0usize)).bounds();
        let second = window.find(("output-entry", 1usize)).bounds();
        let third = window.find(("output-entry", 2usize)).bounds();
        assert_eq!(first.bottom(), second.top());
        assert_eq!(second.bottom(), third.top());
        window.click("clear-output", cx);
    });
    palette_step(cx, handle, |window, cx| {
        assert!(inspector.read(cx).entries.is_empty());
        assert!(window.try_find(("output-entry", 0usize)).is_none());
        inspector.update(cx, |i, cx| i.record("After clear", cx));
    });
    palette_step(cx, handle, |window, _| {
        assert!(window.find(("output-entry", 0usize)).visible());
        assert!(window.try_find(("output-entry", 1usize)).is_none());
    });
}

#[gpui_kit::test]
fn shared_edits_rebuild_each_projection_once(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let (source, other) = cx
        .update_window(handle, |_, window, cx| {
            let workspace = workbench.read(cx).workspace.clone();
            let first = workspace.read(cx).active_view(cx).unwrap();
            let second = workspace
                .update(cx, |w, cx| {
                    w.split(first, SplitDirection::Right, window, cx)
                })
                .unwrap();
            workspace.update(cx, |w, cx| w.select(first, window, cx));
            (
                resource(&workspace, first, cx),
                resource(&workspace, second, cx),
            )
        })
        .unwrap();
    palette_step(cx, handle, |window, cx| {
        source.read(cx).editable.clone().update(cx, |grid, cx| {
            grid.select_cell(CellAddress::new("0", "name"), false, false, cx);
            grid.paste_text("ปรับปรุง José", window, cx).unwrap();
        })
    });
    cx.update(|cx| {
        for view in [&source, &other] {
            assert_eq!(
                view.read(cx).table.read(cx).delegate().projection_rebuilds,
                1
            );
            assert_eq!(
                view.read(cx)
                    .table
                    .read(cx)
                    .delegate()
                    .store
                    .read(cx)
                    .values[0][1]
                    .as_ref(),
                "ปรับปรุง José"
            );
        }
    });
    palette_step(cx, handle, |_, cx| {
        other.read(cx).editable.clone().update(cx, |grid, cx| {
            grid.update_delegate(cx, |rows, cx| {
                rows.query = "ปรับปรุง".into();
                rows.rebuild(cx);
            })
        })
    });
    cx.update(|cx| assert_eq!(other.read(cx).table.read(cx).delegate().indices, [0]));
}

#[gpui_kit::test]
fn center_drag_has_no_timer_and_cancelled_edge_frame_does_not_scroll(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};
    let (handle, workbench) = workbench(cx);
    let view = cx.update(|cx| {
        let w = workbench.read(cx).workspace.clone();
        resource(&w, w.read(cx).active_view(cx).unwrap(), cx)
    });
    let table = cx.update(|cx| view.read(cx).table.clone());
    let mut at = gpui::point(px(0.), px(0.));
    palette_step(cx, handle, |window, cx| {
        at = window.find("editable-cell:3:1").bounds().center();
        window.dispatch_event(
            gpui::MouseDownEvent {
                position: at,
                button: gpui::MouseButton::Left,
                click_count: 1,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        drag_pointer(window, at + gpui::point(px(0.), px(28.)), cx);
    });
    let count = Rc::new(Cell::new(0));
    let notifications = count.clone();
    let _subscription = cx.update(|cx| {
        cx.observe(&table, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(200));
    cx.run_until_parked();
    palette_step(cx, handle, |window, cx| {
        window.simulate_next_frame(cx);
    });
    assert_eq!(
        count.get(),
        0,
        "holding a drag in the middle must not wake a scroll timer"
    );
    let before = cx.update(|cx| {
        table
            .read(cx)
            .vertical_scroll_handle
            .0
            .borrow()
            .base_handle
            .offset()
    });
    palette_step(cx, handle, |window, cx| {
        let frame = window.find("editable-table").bounds();
        drag_pointer(window, gpui::point(at.x, frame.bottom() - px(2.)), cx);
        window.press("escape", cx);
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    palette_step(cx, handle, |window, cx| {
        window.simulate_next_frame(cx);
        finish_drag(window, at, cx);
    });
    cx.update(|cx| {
        assert_eq!(
            table
                .read(cx)
                .vertical_scroll_handle
                .0
                .borrow()
                .base_handle
                .offset(),
            before
        )
    });
}

#[gpui_kit::test]
fn edge_drag_reaches_last_row_and_stops_at_limit(cx: &mut TestAppContext) {
    let (handle, workbench) = workbench(cx);
    let view = cx.update(|cx| {
        let w = workbench.read(cx).workspace.clone();
        resource(&w, w.read(cx).active_view(cx).unwrap(), cx)
    });
    let table = cx.update(|cx| view.read(cx).table.clone());
    let grid = cx.update(|cx| view.read(cx).editable.clone());
    palette_step(cx, handle, |_, cx| {
        table.update(cx, |t, cx| t.scroll_to_row(980, cx))
    });
    let mut at = gpui::point(px(0.), px(0.));
    palette_step(cx, handle, |window, cx| {
        let from = window.find("editable-cell:990:1").bounds().center();
        at = gpui::point(
            from.x,
            window.find("editable-table").bounds().bottom() - px(4.),
        );
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
    });
    for _ in 0..12 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(50));
        palette_step(cx, handle, |window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        });
    }
    palette_step(cx, handle, |window, cx| {
        assert_eq!(grid.read(cx).active_position(cx).unwrap().0, 999);
        let scroll = table
            .read(cx)
            .vertical_scroll_handle
            .0
            .borrow()
            .base_handle
            .clone();
        assert_eq!(scroll.offset().y, -scroll.max_offset().y);
        assert_eq!(
            window.simulate_next_frame(cx),
            0,
            "the edge loop must stop at the limit"
        );
        finish_drag(window, at, cx);
    });
}
