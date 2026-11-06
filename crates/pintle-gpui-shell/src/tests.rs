use crate::{
    command_palette::{CommandEntry, CommandPalette},
    workbench::{self, Workbench, WorkbenchState},
};
use gpui::test::TestWindowExt;
use gpui_kit::{
    self as gpui, AppContext, Context, Entity, FocusHandle, IntoElement, Render, TestAppContext,
    TestSupportExt, Window, div, prelude::*, px,
};

struct Consumer {
    state: Entity<WorkbenchState>,
    focus: FocusHandle,
    _observation: gpui::Subscription,
}
impl Render for Consumer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Workbench::new(
            &self.state,
            div()
                .id("content")
                .aria_label("Custom content")
                .test_support()
                .track_focus(&self.focus)
                .size_full()
                .child("Custom content"),
        )
        .sidebar(workbench::sidebar(
            "sidebar",
            "Custom sidebar",
            div().child("Tree slot"),
            cx,
        ))
        .panel(workbench::frame("panel", cx).child("Custom panel"))
    }
}
#[gpui_kit::test]
fn independent_consumer_slots_visibility_and_focus(cx: &mut TestAppContext) {
    let (handle, view) = cx.update(|cx| {
        gpui::init(cx);
        pintle_gpui_theme::install(Default::default(), cx);
        gpui::open_window(workbench::window_options(cx), cx, |_, cx| {
            cx.new(|cx| {
                let state = cx.new(|cx| WorkbenchState::new(cx));
                let observation = cx.observe(&state, |_, _, cx| cx.notify());
                Consumer {
                    state,
                    focus: cx.focus_handle(),
                    _observation: observation,
                }
            })
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let original = window.find("content").bounds();
        let state = view.read(cx).state.clone();
        let focus = view.read(cx).focus.clone();
        state.update(cx, |state, cx| {
            state.set_sidebar_visible(false, &focus, window, cx);
            state.set_panel_visible(false, &focus, window, cx);
        });
        window.render_frame(cx);
        let expanded = window.find("content").bounds();
        assert!(expanded.size.width > original.size.width + px(200.));
        assert!(expanded.size.height > original.size.height + px(120.));
        assert_eq!(window.focused(cx), Some(focus.clone()));
        state.update(cx, |state, cx| {
            state.set_sidebar_visible(true, &focus, window, cx);
            state.set_panel_visible(true, &focus, window, cx);
        });
        window.render_frame(cx);
        assert_eq!(window.find("content").bounds(), original);
        assert_eq!(window.find("content").label(), Some("Custom content"));
    })
    .unwrap();
}
#[gpui_kit::test]
fn palette_returns_stable_id_after_filtering_and_catalog_replacement(cx: &mut TestAppContext) {
    use std::{cell::RefCell, rc::Rc};
    let selected = Rc::new(RefCell::new(None));
    let (handle, view) = cx.update(|cx| {
        gpui::init(cx);
        pintle_gpui_theme::install(Default::default(), cx);
        gpui::open_window(workbench::window_options(cx), cx, |window, cx| {
            cx.new(|cx| {
                let mut palette = CommandPalette::new(
                    vec![CommandEntry::new("old", "Old", "Old label")],
                    window,
                    cx,
                );
                palette.set_entries(vec![
                    CommandEntry::new("one", "First", "One"),
                    CommandEntry::new("two", "Second", "Same label").keywords(["needle"]),
                    CommandEntry::new("three", "First", "Same label"),
                ]);
                PaletteConsumer {
                    palette,
                    selected: selected.clone(),
                }
            })
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        view.read(cx)
            .palette
            .state
            .clone()
            .update(cx, |state, cx| state.focus(window, cx));
        window.input("needle", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, _| {
        let _ = window;
        assert_eq!(selected.borrow().as_deref(), Some("two"));
    })
    .unwrap();
}
struct PaletteConsumer {
    palette: CommandPalette,
    selected: std::rc::Rc<std::cell::RefCell<Option<gpui::SharedString>>>,
}
impl Render for PaletteConsumer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.clone();
        self.palette
            .render(cx, move |id, _, _| *selected.borrow_mut() = Some(id))
    }
}

#[gpui_kit::test]
fn navigation_options_preserve_defaults_and_render_consumer_decoration(cx: &mut TestAppContext) {
    use crate::workspace::{Document, NavigationNode, NavigationTree, NavigationTreeOptions};
    use std::rc::Rc;
    assert_eq!(NavigationTreeOptions::default().row_height, px(22.));
    let (handle, tree) = cx.update(|cx| {
        gpui::init(cx);
        pintle_gpui_theme::install(Default::default(), cx);
        gpui::open_window(workbench::window_options(cx), cx, |window, cx| {
            cx.new(|cx| {
                NavigationTree::with_options(
                    vec![
                        NavigationNode::folder(
                            "first",
                            "First",
                            vec![NavigationNode::resource(Document::new(
                                "item", "Item", "First",
                            ))],
                        ),
                        NavigationNode::folder(
                            "second",
                            "Second",
                            vec![NavigationNode::resource(Document::new(
                                "other", "Other", "Second",
                            ))],
                        ),
                    ],
                    NavigationTreeOptions {
                        row_height: px(28.),
                        font_family: Some("Monaco".into()),
                        initial_expanded: Some(["first".into()].into_iter().collect()),
                        decoration: Some(Rc::new(|id, _| {
                            div()
                                .id(gpui::SharedString::from(format!("decoration:{id}")))
                                .aria_label("Consumer status")
                                .test_support()
                                .child("Ready")
                                .into_any_element()
                        })),
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(tree.read(cx).is_expanded(&"first".into(), cx));
        assert!(!tree.read(cx).is_expanded(&"second".into(), cx));
        assert_eq!(window.find("nav:first").bounds().size.height, px(28.));
        assert_eq!(
            window.find("decoration:first").label(),
            Some("Consumer status")
        );
        tree.update(cx, |tree, cx| tree.set_query("Other", cx));
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(tree.read(cx).is_expanded(&"second".into(), cx));
        tree.update(cx, |tree, cx| tree.set_query("", cx));
    })
    .unwrap();
    cx.update_window(handle, |_, _, cx| {
        assert!(!tree.read(cx).is_expanded(&"second".into(), cx))
    })
    .unwrap();
}

struct ReadOnlyRows;
impl gpui::component::table::TableDelegate for ReadOnlyRows {
    fn rows_count(&self, _: &gpui::App) -> usize {
        2
    }
    fn columns_count(&self, _: &gpui::App) -> usize {
        1
    }
    fn column(&self, _: usize, _: &gpui::App) -> gpui::component::table::Column {
        gpui::component::table::Column::new("name", "NAME")
    }
    fn render_td(
        &mut self,
        row: usize,
        _: usize,
        _: &mut Window,
        _: &mut Context<gpui::component::table::TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .id(("consumer-row", row))
            .aria_label(format!("Row {row}"))
            .test_support()
            .child(format!("Row {row}"))
    }
}
struct ResourceConsumer {
    floating: bool,
    activations: usize,
    table: Entity<gpui::component::table::TableState<ReadOnlyRows>>,
}
impl Render for ResourceConsumer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use pintle_gpui_components::resource_table::{
            FloatingActionBar, ResourceListHeader, ResourceTable,
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(ResourceListHeader::new(
                div()
                    .id("custom-title")
                    .aria_label("Inventory")
                    .test_support()
                    .child("Inventory"),
                div().child("Offline"),
                div()
                    .id("custom-toolbar")
                    .aria_label("Custom controls")
                    .test_support()
                    .child("Custom controls"),
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(
                        ResourceTable::new(&self.table).when(self.floating, |table| {
                            table.floating_actions(FloatingActionBar::new(
                                "Custom selection",
                                gpui::component::button::Button::new("consumer-bulk-action")
                                    .label("Process")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.activations += 1;
                                        cx.notify();
                                    })),
                                gpui::component::button::Button::new("consumer-bulk-dismiss")
                                    .label("Dismiss")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.floating = false;
                                        cx.notify();
                                    })),
                            ))
                        }),
                    ),
            )
    }
}
#[gpui_kit::test]
fn resource_table_accepts_independent_delegate_and_header_slots(cx: &mut TestAppContext) {
    let handle = cx.update(|cx| {
        gpui::init(cx);
        pintle_gpui_theme::install(Default::default(), cx);
        gpui::open_window(workbench::window_options(cx), cx, |window, cx| {
            cx.new(|cx| ResourceConsumer {
                floating: false,
                activations: 0,
                table: cx
                    .new(|cx| gpui::component::table::TableState::new(ReadOnlyRows, window, cx)),
            })
        })
        .unwrap()
        .0
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("custom-title").label(), Some("Inventory"));
        assert_eq!(
            window.find("custom-toolbar").label(),
            Some("Custom controls")
        );
        let first = window.find(("consumer-row", 0usize)).bounds();
        let second = window.find(("consumer-row", 1usize)).bounds();
        assert_eq!(second.origin.y - first.origin.y, px(36.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn resource_table_floating_actions_accept_caller_controls(cx: &mut TestAppContext) {
    let (handle, consumer) = cx.update(|cx| {
        gpui::init(cx);
        pintle_gpui_theme::install(Default::default(), cx);
        gpui::open_window(workbench::window_options(cx), cx, |window, cx| {
            cx.new(|cx| ResourceConsumer {
                floating: true,
                activations: 0,
                table: cx
                    .new(|cx| gpui::component::table::TableState::new(ReadOnlyRows, window, cx)),
            })
        })
        .unwrap()
    });
    let mut viewport = None;
    let mut first_row = None;
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        viewport = Some(window.find("resource-table-viewport").bounds());
        first_row = Some(window.find(("consumer-row", 0usize)).bounds());
        let bar = window.find("floating-action-bar").bounds();
        assert_eq!(bar.bottom(), viewport.unwrap().bottom() - px(12.));
        window.click("consumer-bulk-action", cx);
        assert_eq!(consumer.read(cx).activations, 1);
        window.click("consumer-bulk-dismiss", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find_all("floating-action-bar").is_empty());
        assert_eq!(
            window.find("resource-table-viewport").bounds(),
            viewport.unwrap()
        );
        assert_eq!(
            window.find(("consumer-row", 0usize)).bounds(),
            first_row.unwrap()
        );
        assert!(window.find(("consumer-row", 1usize)).visible());
    })
    .unwrap();
}
