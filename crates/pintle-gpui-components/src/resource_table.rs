//! Read-only resource-list presentation over upstream retained, virtualized tables.
use gpui::component::{
    ActiveTheme, Sizable,
    table::{DataTable, TableDelegate, TableState},
};
use gpui::{
    AnyElement, App, Entity, IntoElement, RenderOnce, TestSupportExt, Window, div, prelude::*, px,
};
use gpui_kit as gpui;

/// A consistent resource table skin. Data, selection and actions stay with the delegate.
#[derive(IntoElement)]
pub struct ResourceTable<D: TableDelegate> {
    state: Entity<TableState<D>>,
    floating_actions: Option<AnyElement>,
}
impl<D: TableDelegate> ResourceTable<D> {
    pub fn new(state: &Entity<TableState<D>>) -> Self {
        Self {
            state: state.clone(),
            floating_actions: None,
        }
    }

    /// Pin caller-owned actions below the scrolling viewport. Reserves room so
    /// the last row remains reachable, independently of horizontal scrolling.
    pub fn floating_actions(mut self, actions: impl IntoElement) -> Self {
        self.floating_actions = Some(actions.into_any_element());
        self
    }
}
impl<D: TableDelegate> RenderOnce for ResourceTable<D> {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .relative()
            .flex()
            .flex_col()
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
                div().flex_1().min_h_0().min_w_0().child(
                    DataTable::new(&self.state)
                        .stripe(false)
                        .bordered(false)
                        .with_size(px(36.))
                        .header_row_height(px(32.)),
                ),
            )
            .when_some(self.floating_actions, |table, actions| {
                table.child(div().h(px(72.)).flex_shrink_0()).child(
                    div()
                        .absolute()
                        .bottom(px(12.))
                        .left(px(12.))
                        .right(px(12.))
                        .flex()
                        .justify_center()
                        .child(actions),
                )
            })
    }
}

/// Floating selection controls with caller-owned summary, actions and dismissal.
/// This presentation component does not own selection or command semantics.
#[derive(IntoElement)]
pub struct FloatingActionBar {
    summary: AnyElement,
    actions: AnyElement,
    dismiss: AnyElement,
}
impl FloatingActionBar {
    pub fn new(
        summary: impl IntoElement,
        actions: impl IntoElement,
        dismiss: impl IntoElement,
    ) -> Self {
        Self {
            summary: summary.into_any_element(),
            actions: actions.into_any_element(),
            dismiss: dismiss.into_any_element(),
        }
    }
}
impl RenderOnce for FloatingActionBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .id("floating-action-bar")
            .test_support()
            .flex()
            .items_center()
            .h(px(48.))
            .max_w_full()
            .min_w_0()
            .px_2()
            .gap_2()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(px(8.))
            .shadow_lg()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .child(div().min_w_0().px_2().child(self.summary))
            .child(
                div()
                    .h(px(28.))
                    .border_l_1()
                    .border_color(cx.theme().border),
            )
            .child(self.actions)
            .child(
                div()
                    .h(px(28.))
                    .border_l_1()
                    .border_color(cx.theme().border),
            )
            .child(self.dismiss)
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
