//! Shared Pintle chrome. Applications own content, commands and domain state.
use gpui::component::{
    ActiveTheme, TitleBar,
    resizable::{ResizableState, h_resizable, resizable_panel, v_resizable},
};
use gpui::{prelude::*, *};
use gpui_kit as gpui;
use pintle_gpui_components::{frame_resize_handle_appearance, rounded_frame_overlay};
use pintle_gpui_theme::tokens::*;

/// State survives hiding a region. Observe this entity in the owning view.
pub struct WorkbenchState {
    pub sidebar_visible: bool,
    pub panel_visible: bool,
    sidebar: Entity<ResizableState>,
    panel: Entity<ResizableState>,
}
impl WorkbenchState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            sidebar_visible: true,
            panel_visible: true,
            sidebar: cx.new(|_| ResizableState::default()),
            panel: cx.new(|_| ResizableState::default()),
        }
    }
    pub fn set_sidebar_visible(
        &mut self,
        visible: bool,
        return_focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sidebar_visible = visible;
        if !visible {
            window.focus(return_focus, cx);
        }
        cx.notify();
    }
    pub fn set_panel_visible(
        &mut self,
        visible: bool,
        return_focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.panel_visible = visible;
        if !visible {
            window.focus(return_focus, cx);
        }
        cx.notify();
    }
}

/// A rounded frame that masks child paint without escaping ancestor clipping.
pub fn frame(id: impl Into<ElementId>, cx: &App) -> gpui::base::ObservedElement<Stateful<Div>> {
    div()
        .id(id)
        .test_support()
        .size_full()
        .flex()
        .flex_col()
        .min_w_0()
        .min_h_0()
        .bg(cx.theme().sidebar)
        .border_1()
        .border_color(transparent_black())
        .rounded(WORKBENCH_RADIUS)
        .relative()
        .overflow_hidden()
        .child(rounded_frame_overlay(
            WORKBENCH_RADIUS,
            cx.theme().sidebar,
            cx.theme().border,
        ))
}
pub fn sidebar(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    content: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    frame(id, cx)
        .child(
            div()
                .h(WORKBENCH_TITLE_HEIGHT)
                .flex_shrink_0()
                .flex()
                .items_center()
                .px_3()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.into()),
        )
        .child(div().flex_1().min_h_0().child(content))
}
pub fn activity_bar() -> gpui::base::ObservedElement<Stateful<Div>> {
    div()
        .id("activity-bar")
        .test_support()
        .w(WORKBENCH_ACTIVITY_WIDTH)
        .flex_shrink_0()
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .py_1()
        .gap_1()
}
pub fn title_bar(cx: &App) -> TitleBar {
    TitleBar::new()
        .h(WORKBENCH_TITLE_HEIGHT)
        .bg(cx.theme().title_bar)
        .border_b_0()
}
/// Centered search with native traffic-light clearance and a trailing action slot.
pub fn title_content(search: impl IntoElement, actions: impl IntoElement) -> impl IntoElement {
    div()
        .size_full()
        .relative()
        .flex()
        .items_center()
        .justify_end()
        .child(
            div()
                .absolute()
                .left_0()
                .right(if cfg!(target_os = "macos") {
                    px(80.)
                } else {
                    px(0.)
                })
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(search),
        )
        .child(actions)
}
pub fn status_bar(
    left: impl IntoElement,
    right: impl IntoElement,
    cx: &App,
) -> gpui::base::ObservedElement<Stateful<Div>> {
    div()
        .id("workspace-status")
        .test_support()
        .h(WORKBENCH_STATUS_HEIGHT)
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .text_size(px(12.))
        .text_color(cx.theme().muted_foreground)
        .child(left)
        .child(right)
}
/// Header of a bottom panel. Supply tab controls and application-owned actions.
pub fn panel_tabs(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .h(WORKBENCH_TITLE_HEIGHT)
        .flex_shrink_0()
        .flex()
        .items_center()
        .px_2()
        .gap_1()
        .role(Role::TabList)
}
/// Shared panel tab styling; apps supply selection/focus and callbacks.
pub fn panel_tab(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
) -> gpui::component::button::Button {
    use gpui::component::{Selectable, Sizable, button::ButtonVariants};
    let label = label.into();
    gpui::component::button::Button::new(id)
        .ghost()
        .small()
        .label(label.clone())
        .h(px(27.))
        .rounded(px(4.))
        .selected(selected)
        .role(Role::Tab)
        .toggled(selected)
        .accessibility_label(label)
}

#[derive(IntoElement)]
pub struct Workbench {
    state: Entity<WorkbenchState>,
    content: AnyElement,
    title: Option<AnyElement>,
    activity: Option<AnyElement>,
    sidebar: Option<AnyElement>,
    panel: Option<AnyElement>,
    status: Option<AnyElement>,
}
impl Workbench {
    pub fn new(state: &Entity<WorkbenchState>, content: impl IntoElement) -> Self {
        Self {
            state: state.clone(),
            content: content.into_any_element(),
            title: None,
            activity: None,
            sidebar: None,
            panel: None,
            status: None,
        }
    }
    pub fn title(mut self, value: impl IntoElement) -> Self {
        self.title = Some(value.into_any_element());
        self
    }
    pub fn activity(mut self, value: impl IntoElement) -> Self {
        self.activity = Some(value.into_any_element());
        self
    }
    pub fn sidebar(mut self, value: impl IntoElement) -> Self {
        self.sidebar = Some(value.into_any_element());
        self
    }
    pub fn panel(mut self, value: impl IntoElement) -> Self {
        self.panel = Some(value.into_any_element());
        self
    }
    pub fn status(mut self, value: impl IntoElement) -> Self {
        self.status = Some(value.into_any_element());
        self
    }
}
impl RenderOnce for Workbench {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let body_height = window.viewport_size().height
            - if self.title.is_some() {
                WORKBENCH_TITLE_HEIGHT
            } else {
                px(0.)
            }
            - if self.status.is_some() {
                WORKBENCH_STATUS_HEIGHT
            } else {
                px(0.)
            }
            - WORKBENCH_GAP;
        let panel_max = (body_height * 0.5).max(WORKBENCH_PANEL_MIN);
        let editor = if let Some(panel) = self.panel.filter(|_| state.panel_visible) {
            v_resizable("workspace-inspector")
                .with_handle_appearance(frame_resize_handle_appearance())
                .with_state(&state.panel)
                .child(
                    resizable_panel()
                        .size_range(WORKBENCH_PANEL_HEIGHT..px(10000.))
                        .child(self.content),
                )
                .child(
                    resizable_panel()
                        .size(WORKBENCH_PANEL_HEIGHT + WORKBENCH_GAP)
                        .size_range(
                            (WORKBENCH_PANEL_MIN + WORKBENCH_GAP)..(panel_max + WORKBENCH_GAP),
                        )
                        .fixed()
                        .child(div().pt(WORKBENCH_GAP).size_full().child(panel)),
                )
                .into_any_element()
        } else {
            div().size_full().child(self.content).into_any_element()
        };
        let area = if let Some(sidebar) = self.sidebar.filter(|_| state.sidebar_visible) {
            h_resizable("workspace-sidebar")
                .with_handle_appearance(frame_resize_handle_appearance())
                .with_state(&state.sidebar)
                .child(
                    resizable_panel()
                        .size(WORKBENCH_SIDEBAR_WIDTH)
                        .size_range(WORKBENCH_SIDEBAR_MIN..WORKBENCH_SIDEBAR_MAX)
                        .fixed()
                        .child(sidebar),
                )
                .child(resizable_panel().child(div().pl(WORKBENCH_GAP).size_full().child(editor)))
                .into_any_element()
        } else {
            editor
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().foreground)
            .font_family(cx.theme().font_family.clone())
            .text_size(px(13.))
            .children(self.title)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .pr(WORKBENCH_GAP)
                    .pb(WORKBENCH_GAP)
                    .children(self.activity)
                    .child(div().flex_1().min_w_0().min_h_0().child(area)),
            )
            .children(self.status)
    }
}
pub fn window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1440.), px(900.)),
            cx,
        ))),
        window_min_size: Some(size(px(1024.), px(680.))),
        ..TitleBar::window_options()
    }
}

/// Roving keyboard selection for panel headers; callers focus the returned tab.
pub fn next_panel_tab(key: &str, current: usize, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match key {
        "home" => Some(0),
        "end" => Some(count - 1),
        "left" => Some((current + count - 1) % count),
        "right" => Some((current + 1) % count),
        _ => None,
    }
}
/// Consistent title-bar search affordance. Applications attach the click action.
pub fn search_button(label: impl Into<SharedString>, cx: &App) -> gpui::component::button::Button {
    use gpui::component::{Icon, IconName, button::Button};
    Button::new("command-search")
        .outline()
        .h(px(24.))
        .w(px(380.))
        .rounded(px(5.))
        .bg(cx.theme().sidebar)
        .accessibility_label("Search resources and commands")
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(12.))
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(IconName::Search).size(px(14.)))
                .child(div().flex_1().child(label.into()))
                .child(if cfg!(target_os = "macos") {
                    "⌘ K"
                } else {
                    "Ctrl K"
                }),
        )
}
