//! Read-only resource-list presentation over upstream retained, virtualized tables.
use gpui::component::{
    ActiveTheme, Sizable,
    table::{DataTable, TableDelegate, TableState},
};
use gpui::{AnyElement, App, Entity, IntoElement, RenderOnce, Window, div, prelude::*, px};
use gpui_kit as gpui;

/// A consistent resource table skin. Data, selection and actions stay with the delegate.
#[derive(IntoElement)]
pub struct ResourceTable<D: TableDelegate> {
    state: Entity<TableState<D>>,
}
impl<D: TableDelegate> ResourceTable<D> {
    pub fn new(state: &Entity<TableState<D>>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}
impl<D: TableDelegate> RenderOnce for ResourceTable<D> {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(13.))
            .border_1()
            .border_color(cx.theme().border)
            .rounded(px(8.))
            .child(
                DataTable::new(&self.state)
                    .stripe(false)
                    .bordered(false)
                    .with_size(px(36.))
                    .header_row_height(px(32.)),
            )
    }
}

/// Header composition with caller-owned title, status and toolbar controls.
#[derive(IntoElement)]
pub struct ResourceListHeader {
    title: AnyElement,
    status: AnyElement,
    toolbar: AnyElement,
}
impl ResourceListHeader {
    pub fn new(
        title: impl IntoElement,
        status: impl IntoElement,
        toolbar: impl IntoElement,
    ) -> Self {
        Self {
            title: title.into_any_element(),
            status: status.into_any_element(),
            toolbar: toolbar.into_any_element(),
        }
    }
}
impl RenderOnce for ResourceListHeader {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .flex_shrink_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().min_w_0().child(self.title))
                    .child(self.status),
            )
            .child(self.toolbar)
    }
}
