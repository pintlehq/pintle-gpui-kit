//! Lightweight compositions; use `gpui_kit::component` for standard controls.
use gpui::{AnyElement, App, IntoElement, RenderOnce, SharedString, Window, div, prelude::*};
use gpui_kit as gpui;
use pintle_gpui_theme::{ActiveTheme, tokens::*};

pub mod context_menu;
pub mod resource_table;
pub mod table;

mod frame;
pub use frame::{frame_resize_handle_appearance, rounded_frame_overlay};

/// A section title and description with a consumer-owned action slot.
#[derive(IntoElement)]
pub struct SectionHeader {
    title: SharedString,
    description: Option<SharedString>,
    action: Option<AnyElement>,
}

impl SectionHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            action: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for SectionHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .items_start()
            .gap(SPACE_4)
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(SPACE_1)
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(self.title),
                    )
                    .children(
                        self.description
                            .map(|text| div().text_color(cx.theme().muted_foreground).child(text)),
                    ),
            )
            .children(
                self.action
                    .map(|action| div().flex_shrink_0().child(action)),
            )
    }
}

/// A themed section with optional header and arbitrary content.
///
/// A panel does not own scrolling or state. Place it in a scrollable parent
/// when its content can exceed the available height.
#[derive(IntoElement)]
pub struct Panel {
    header: Option<AnyElement>,
    children: Vec<AnyElement>,
    padding: gpui::Pixels,
}

impl Default for Panel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel {
    pub fn new() -> Self {
        Self {
            header: None,
            children: Vec::new(),
            padding: SPACE_4,
        }
    }

    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    pub fn padding(mut self, padding: gpui::Pixels) -> Self {
        self.padding = padding;
        self
    }
}

impl ParentElement for Panel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Panel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .gap(SPACE_4)
            .p(self.padding)
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .children(self.header)
            .children(self.children)
    }
}
