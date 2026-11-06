//! Optional layout composition. This crate is unrelated to upstream `gpui-shell`.
use gpui::{AnyElement, App, IntoElement, Pixels, RenderOnce, Window, div, prelude::*};
use gpui_kit as gpui;
use pintle_gpui_theme::{ActiveTheme, tokens::*};

/// A desktop layout with application-owned regions and state.
///
/// Header and status span the window; toolbar sits above main content.
/// Omitted regions take no space. Content and sidebar own their scrolling.
#[derive(IntoElement)]
pub struct AppShell {
    content: AnyElement,
    sidebar: Option<AnyElement>,
    header: Option<AnyElement>,
    toolbar: Option<AnyElement>,
    status_bar: Option<AnyElement>,
    sidebar_width: Pixels,
}

impl AppShell {
    pub fn new(content: impl IntoElement) -> Self {
        Self {
            content: content.into_any_element(),
            sidebar: None,
            header: None,
            toolbar: None,
            status_bar: None,
            sidebar_width: SIDEBAR_WIDTH,
        }
    }

    pub fn sidebar(mut self, content: impl IntoElement) -> Self {
        self.sidebar = Some(content.into_any_element());
        self
    }
    pub fn header(mut self, content: impl IntoElement) -> Self {
        self.header = Some(content.into_any_element());
        self
    }
    pub fn toolbar(mut self, content: impl IntoElement) -> Self {
        self.toolbar = Some(content.into_any_element());
        self
    }
    pub fn status_bar(mut self, content: impl IntoElement) -> Self {
        self.status_bar = Some(content.into_any_element());
        self
    }
    pub fn sidebar_width(mut self, width: Pixels) -> Self {
        self.sidebar_width = width.max(gpui::px(0.));
        self
    }
}

impl RenderOnce for AppShell {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(cx.theme().font_family.clone())
            .text_size(cx.theme().font_size)
            .children(self.header.map(|header| {
                div()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(header)
            }))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .children(self.sidebar.map(|sidebar| {
                        div()
                            .w(self.sidebar_width)
                            .h_full()
                            .flex_shrink_0()
                            .min_h_0()
                            .overflow_hidden()
                            .bg(cx.theme().sidebar)
                            .text_color(cx.theme().sidebar_foreground)
                            .border_r_1()
                            .border_color(cx.theme().sidebar_border)
                            .child(sidebar)
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .children(self.toolbar.map(|toolbar| {
                                div()
                                    .flex_shrink_0()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(toolbar)
                            }))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .overflow_hidden()
                                    .child(self.content),
                            ),
                    ),
            )
            .children(self.status_bar.map(|status| {
                div()
                    .flex_shrink_0()
                    .bg(cx.theme().status_bar)
                    .border_t_1()
                    .border_color(cx.theme().status_bar_border)
                    .child(status)
            }))
    }
}

/// Use the shared panel composition inside shell content regions.
pub use pintle_gpui_components::Panel;

/// Docked, retained document workspaces and searchable navigation.
pub mod workspace;

/// Application-supplied searchable commands.
pub mod command_palette;
/// Shared desktop chrome and retained layout state.
pub mod workbench;

#[cfg(test)]
mod tests;
