//! Retain one table and filter per document view, including each split.
use crate::model::{Rows, Store};
use gpui::component::{
    ActiveTheme, Sizable,
    input::{Input, InputEvent, InputState},
};
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Subscription,
    Window, div, prelude::*, px,
};
use pintle_gpui_kit::{
    components::table::{EditableTable, EditableTableState},
    kit as gpui,
    shell::workspace::Document,
};

pub struct TablePage {
    pub grid: Entity<EditableTableState<Rows>>,
    pub filter: Entity<InputState>,
    document: Document,
    _subscriptions: Vec<Subscription>,
}
impl TablePage {
    pub fn new(
        document: Document,
        store: Entity<Store>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let grid = cx.new(|cx| EditableTableState::new(Rows::new(store.clone(), cx), window, cx));
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter rows…"));
        let weak_grid = grid.downgrade();
        let shared = cx.observe(&store, move |_, _, cx| {
            let _ = weak_grid.update(cx, |grid, cx| {
                grid.update_delegate(cx, |rows, cx| rows.rebuild(cx))
            });
        });
        let filtering = cx.subscribe_in(
            &filter,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let query = input.read(cx).value().to_lowercase();
                let accepted = this.grid.update(cx, |grid, cx| {
                    if !grid.commit_edit(window, cx) {
                        return false;
                    }
                    grid.update_delegate(cx, |rows, cx| {
                        rows.query = query;
                        rows.rebuild(cx);
                    });
                    true
                });
                if !accepted {
                    let previous = this.grid.read(cx).table().read(cx).delegate().query.clone();
                    input.update(cx, |input, cx| input.set_value(previous, window, cx));
                }
                cx.notify();
            },
        );
        Self {
            grid,
            filter,
            document,
            _subscriptions: vec![shared, filtering],
        }
    }
}
impl Focusable for TablePage {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.grid.focus_handle(cx)
    }
}
impl Render for TablePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.grid.read(cx).table().read(cx).delegate().visible.len();
        div()
            .id(("table-page", cx.entity_id()))
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                div()
                    .h(px(24.))
                    .flex_shrink_0()
                    .px_3()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} / {}",
                        self.document.context, self.document.title
                    )),
            )
            .child(
                div()
                    .h(px(40.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .child(
                        div().flex_1().min_w_0().max_w(px(400.)).child(
                            Input::new(&self.filter)
                                .id("table-filter")
                                .aria_label(format!("Filter {}", self.document.title))
                                .small(),
                        ),
                    )
                    .child(div().text_size(px(12.)).child(format!("{count} rows"))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(EditableTable::new(&self.grid)),
            )
    }
}
