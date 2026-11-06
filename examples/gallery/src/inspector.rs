//! Gallery-owned details and bounded activity output for retained resource views.
use super::*;
use crate::data::ResourceView;
use gpui::assets::IconName;
use gpui::{FocusHandle, ScrollHandle, UniformListScrollHandle, WeakEntity, uniform_list};
use pintle_gpui_kit::shell::workbench::{self as chrome, WorkbenchState};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectorTab {
    Details,
    Output,
    Terminal,
}
pub struct Inspector {
    pub tab: InspectorTab,
    pub entries: VecDeque<String>,
    pub workspace: Option<WeakEntity<pintle_gpui_kit::shell::workspace::Workspace>>,
    pub layout: Entity<WorkbenchState>,
    focus: FocusHandle,
    details_focus: FocusHandle,
    output_focus: FocusHandle,
    terminal_focus: FocusHandle,
    pub terminal: Entity<crate::terminal_panel::TerminalPanel>,
    output_scroll: UniformListScrollHandle,
    details_scroll: ScrollHandle,
    last_details: Vec<(SharedString, SharedString)>,
    last_document: Option<pintle_gpui_kit::shell::workspace::ViewId>,
    multiline_entries: usize,
    output_lines: Option<Vec<String>>,
    output_lines_dirty: bool,
}
impl Inspector {
    #[cfg(test)]
    pub(crate) fn output_offset(&self) -> gpui::Point<gpui::Pixels> {
        self.output_scroll.0.borrow().base_handle.offset()
    }
    pub fn new(layout: Entity<WorkbenchState>, cx: &mut Context<Self>) -> Self {
        Self {
            tab: InspectorTab::Details,
            entries: VecDeque::new(),
            workspace: None,
            layout,
            focus: cx.focus_handle(),
            details_focus: cx.focus_handle(),
            output_focus: cx.focus_handle(),
            terminal_focus: cx.focus_handle(),
            terminal: cx.new(crate::terminal_panel::TerminalPanel::new),
            output_scroll: UniformListScrollHandle::default(),
            details_scroll: ScrollHandle::default(),
            last_details: vec![],
            last_document: None,
            multiline_entries: 0,
            output_lines: None,
            output_lines_dirty: false,
        }
    }
    pub fn record(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        if self.entries.len() == 500
            && let Some(oldest) = self.entries.pop_front()
        {
            self.multiline_entries -= usize::from(oldest.contains('\n'));
        }
        let message = message.into();
        self.multiline_entries += usize::from(message.contains('\n'));
        self.entries.push_back(message);
        self.output_lines_dirty = true;
        if self.layout.read(cx).panel_visible && self.tab == InspectorTab::Output {
            cx.notify();
        }
    }
    pub fn refresh_details(&mut self, cx: &mut Context<Self>) {
        let document = self
            .workspace
            .as_ref()
            .and_then(|w| w.upgrade())
            .and_then(|w| w.read(cx).active_view(cx));
        let details = self.details(cx);
        if document == self.last_document && details == self.last_details {
            return;
        }
        self.last_document = document;
        self.last_details = details;
        if self.layout.read(cx).panel_visible && self.tab == InspectorTab::Details {
            cx.notify();
        }
    }
    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.upgrade())
            .map(|workspace| workspace.read(cx).active_focus_handle(cx))
            .unwrap_or_else(|| self.focus.clone());
        self.layout.update(cx, |layout, cx| {
            layout.set_panel_visible(!layout.panel_visible, &focus, window, cx)
        });
        if self.layout.read(cx).panel_visible && self.tab == InspectorTab::Terminal {
            self.terminal
                .update(cx, |panel, cx| panel.focus_active(window, cx));
        }
        cx.notify();
    }
    fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.layout.update(cx, |layout, cx| {
            layout.set_panel_visible(true, &self.focus, window, cx)
        });
    }
    fn tab_focus(&self, tab: InspectorTab) -> &FocusHandle {
        match tab {
            InspectorTab::Details => &self.details_focus,
            InspectorTab::Output => &self.output_focus,
            InspectorTab::Terminal => &self.terminal_focus,
        }
    }
    pub fn select_tab(&mut self, tab: InspectorTab, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        if tab == InspectorTab::Terminal {
            self.terminal.update(cx, |panel, cx| panel.open(window, cx));
        } else {
            window.focus(self.tab_focus(tab), cx);
        }
        cx.notify();
    }
    pub fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.layout.read(cx).panel_visible
            && self.tab == InspectorTab::Terminal
            && self.terminal.read(cx).is_focused(window, cx)
        {
            self.toggle(window, cx);
        } else {
            self.show(window, cx);
            self.select_tab(InspectorTab::Terminal, window, cx);
        }
    }
    pub fn new_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show(window, cx);
        self.tab = InspectorTab::Terminal;
        self.terminal
            .update(cx, |panel, cx| panel.new_session(window, cx));
        cx.notify();
    }
    pub fn details(&self, cx: &App) -> Vec<(SharedString, SharedString)> {
        let Some(workspace) = self.workspace.as_ref().and_then(|w| w.upgrade()) else {
            return vec![];
        };
        let workspace = workspace.read(cx);
        let Some(view) = workspace
            .active_view(cx)
            .and_then(|id| workspace.page(id, cx))
            .and_then(|view| view.downcast::<ResourceView>().ok())
        else {
            return vec![];
        };
        let table = view.read(cx).table.read(cx);
        let Some(cell) = view.read(cx).editable.read(cx).active_cell() else {
            return vec![];
        };
        let rows = table.delegate();
        let store = rows.store.read(cx);
        let Some(values) = store.row_values(&cell.row) else {
            return vec![];
        };
        rows.data
            .columns
            .iter()
            .zip(values)
            .map(|(label, value)| ((*label).into(), value.clone()))
            .collect()
    }
}
impl Render for Inspector {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.tab == InspectorTab::Output && self.output_lines_dirty {
            self.output_lines = (self.multiline_entries > 0).then(|| {
                self.entries
                    .iter()
                    .rev()
                    .flat_map(|entry| entry.split('\n').map(str::to_owned))
                    .collect()
            });
            self.output_lines_dirty = false;
        }
        let details = self.details(cx);
        let title = self
            .workspace
            .as_ref()
            .and_then(|w| w.upgrade())
            .and_then(|w| {
                let workspace = w.read(cx);
                workspace
                    .active_view(cx)
                    .and_then(|id| workspace.document(id, cx))
            })
            .map(|d| format!("{} · {}", d.title, d.context))
            .unwrap_or("Workspace".into());
        let details_active = self.tab == InspectorTab::Details;
        chrome::frame("inspector-panel", cx)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                let on_tab = [
                    InspectorTab::Details,
                    InspectorTab::Output,
                    InspectorTab::Terminal,
                ]
                .into_iter()
                .any(|tab| this.tab_focus(tab).is_focused(window));
                if on_tab
                    && matches!(
                        event.keystroke.key.as_str(),
                        "left" | "right" | "home" | "end"
                    )
                {
                    let tabs = [
                        InspectorTab::Details,
                        InspectorTab::Output,
                        InspectorTab::Terminal,
                    ];
                    let index = tabs.iter().position(|tab| *tab == this.tab).unwrap();
                    let index =
                        chrome::next_panel_tab(&event.keystroke.key, index, tabs.len()).unwrap();
                    this.select_tab(tabs[index], window, cx);
                    // Roving tab focus keeps arrows available for subsequent tab navigation.
                    window.focus(this.tab_focus(tabs[index]), cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                chrome::panel_tabs("inspector-tabs")
                    .children(
                        [
                            (InspectorTab::Details, "Details", "inspector-details"),
                            (InspectorTab::Output, "Output", "inspector-output"),
                            (InspectorTab::Terminal, "Terminal", "inspector-terminal"),
                        ]
                        .into_iter()
                        .map(|(tab, label, id)| {
                            chrome::panel_tab(id, label, self.tab == tab)
                                .track_focus(self.tab_focus(tab))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_tab(tab, window, cx);
                                }))
                        }),
                    )
                    .child(div().flex_1())
                    .when(self.tab == InspectorTab::Output, |bar| {
                        bar.child(
                            Button::new("clear-output")
                                .icon(IconName::Trash)
                                .ghost()
                                .xsmall()
                                .size(px(24.))
                                .accessibility_label("Clear output")
                                .tooltip("Clear output")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    window.focus(&this.output_focus, cx);
                                    this.entries.clear();
                                    this.multiline_entries = 0;
                                    this.output_lines = None;
                                    this.output_lines_dirty = false;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new("close-inspector")
                            .icon(IconName::X)
                            .ghost()
                            .xsmall()
                            .size(px(24.))
                            .accessibility_label("Close panel")
                            .tooltip("Close panel")
                            .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx))),
                    ),
            )
            .child(if self.tab == InspectorTab::Terminal {
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.terminal.clone())
                    .into_any_element()
            } else if details_active {
                div()
                    .id("inspector-detail-content")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_scroll()
                    .track_scroll(&self.details_scroll)
                    .px_4()
                    .py_2()
                    .child(
                        div()
                            .mb_2()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .when(details.is_empty(), |body| {
                        body.child(
                            div()
                                .id("inspector-empty")
                                .test_support()
                                .text_color(cx.theme().muted_foreground)
                                .child("Select a row to inspect"),
                        )
                    })
                    .children(details.into_iter().enumerate().map(|(ix, (label, value))| {
                        div()
                            .id(("detail-field", ix))
                            .test_support()
                            .flex()
                            .gap_3()
                            .py_1()
                            .child(
                                div()
                                    .w(px(120.))
                                    .flex_shrink_0()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(label),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .text_size(px(13.))
                                    .child(value),
                            )
                    }))
                    .into_any_element()
            } else {
                div()
                    .id("inspector-output-content")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .px_4()
                    .py_2()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(13.))
                    .when(self.entries.is_empty(), |body| {
                        body.child("No activity yet")
                    })
                    .child(
                        uniform_list(
                            "activity-output",
                            self.output_lines
                                .as_ref()
                                .map_or(self.entries.len(), Vec::len),
                            {
                                let owner = cx.entity();
                                move |range, _, cx| {
                                    let inspector = owner.read(cx);
                                    range
                                        .map(|ix| {
                                            div()
                                                .id(("output-entry", ix))
                                                .test_support()
                                                .h(px(20.))
                                                .whitespace_nowrap()
                                                .child(inspector.output_lines.as_ref().map_or_else(
                                                    || {
                                                        inspector.entries
                                                            [inspector.entries.len() - 1 - ix]
                                                            .clone()
                                                    },
                                                    |lines| lines[ix].clone(),
                                                ))
                                        })
                                        .collect::<Vec<_>>()
                                }
                            },
                        )
                        .size_full()
                        .with_horizontal_sizing_behavior(
                            gpui::ListHorizontalSizingBehavior::Unconstrained,
                        )
                        .track_scroll(&self.output_scroll),
                    )
                    .into_any_element()
            })
    }
}
