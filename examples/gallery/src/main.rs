//! Executable documentation: production code depends only on the public facade.
use gpui::component::{
    ActiveTheme, Colorize, Disableable, Selectable, Sizable, WindowExt,
    badge::Badge,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    tab::{Tab, TabBar},
};
use gpui::{
    App, AppContext, Context, Entity, IntoElement, Render, SharedString, Window, WindowOptions,
    div, prelude::*, px,
};
use pintle_gpui_kit::{
    components::{Panel, SectionHeader},
    kit as gpui,
    shell::AppShell,
    theme,
};
use theme::tokens::*;
// Kit's Action derive expands this name; it still resolves through our facade.
use gpui::TestSupportExt;
use pintle_gpui_kit::kit as gpui_kit;

gpui::actions!(
    gallery,
    [
        Quit,
        OpenCommandPalette,
        ToggleInspector,
        ToggleSidebar,
        ToggleTerminal,
        NewTerminal,
        CloseTerminal
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Overview,
    Components,
    Theme,
    Shell,
}

impl Page {
    const ALL: [Self; 4] = [Self::Overview, Self::Components, Self::Theme, Self::Shell];
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Components => "Components",
            Self::Theme => "Theme",
            Self::Shell => "Shell",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Overview => "nav-overview",
            Self::Components => "nav-components",
            Self::Theme => "nav-theme",
            Self::Shell => "nav-shell",
        }
    }
}

mod appearance;
mod catalog;
mod data;
mod inspector;
mod terminal_panel;
mod workbench;

struct Gallery {
    focus: gpui::FocusHandle,
    embedded: bool,
    page: Page,
    name: Entity<InputState>,
    saved_name: Option<SharedString>,
    count: usize,
    tab: usize,
    show_sidebar: bool,
    show_header: bool,
    show_toolbar: bool,
    show_status: bool,
    branded: bool,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            embedded: false,
            page: Page::Overview,
            name: cx.new(|cx| InputState::new(window, cx).placeholder("Display name")),
            saved_name: None,
            count: 0,
            tab: 0,
            show_sidebar: true,
            show_header: true,
            show_toolbar: true,
            show_status: true,
            branded: false,
        }
    }

    fn overview(&self, cx: &App) -> gpui::AnyElement {
        div().flex().flex_col().gap(SPACE_6)
            .child(SectionHeader::new("One foundation. Independent applications.")
                .description("Shared native UI for Lightship, Gridbox, and Sendbox."))
            .child(Panel::new().header(SectionHeader::new("Pintle GPUI Kit")
                .description(format!("Version {} · GPUI Kit 0.8.1 · Apache-2.0", env!("CARGO_PKG_VERSION"))))
                .child("A small, composable layer over GPUI Kit. Standard controls remain upstream; Pintle supplies visual conventions and reusable application structure."))
            .child(Panel::new().header(SectionHeader::new("Available modules"))
                .children([
                    "Theme — neutral palettes, semantic colors, and product overrides.",
                    "Components — SectionHeader and Panel compositions.",
                    "Shell — optional sidebar, header, toolbar, content, and status slots.",
                    "Kit — direct access to upstream controls and the native runtime.",
                ].map(|text| div().child(text))))
            .child(div().text_color(cx.theme().muted_foreground)
                .child("Keep a tab open to preserve its demo state while exploring other resources. Theme changes preserve all open views."))
            .into_any_element()
    }

    fn components(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let saved: SharedString = self
            .saved_name
            .as_ref()
            .map(|name| format!("Saved: {name}").into())
            .unwrap_or_else(|| "No name saved".into());
        div().flex().flex_col().gap(SPACE_6)
            .child(Panel::new().header(SectionHeader::new("Input and dialog")
                .description("Enter a name, then save it. Escape dismisses the confirmation."))
                .child(Input::new(&self.name).id("display-name").aria_label("Display name"))
                .child(Button::new("save-name").primary().label("Save name")
                    .on_click(cx.listener(|this, _, window, cx| {
                        let value = this.name.read(cx).value();
                        this.saved_name = Some(value.clone());
                        window.open_dialog(cx, move |dialog, _, _| {
                            dialog.title("Name saved").child(format!("Saved locally for this session: {value}"))
                                .footer(Button::new("dismiss-save").label("Done")
                                    .on_click(|_, window, cx| window.close_dialog(cx)))
                                .centered()
                        });
                        cx.notify();
                    })))
                .child(div().id("save-status").role(gpui::Role::Status)
                    .test_support().aria_label(saved.clone()).child(saved)))
            .child(Panel::new().header(SectionHeader::new("Buttons and badge")
                .description("The badge reflects the counter. Reset returns it to zero."))
                .child(div().flex().flex_wrap().items_center().gap(SPACE_4)
                    .child(Badge::new().count(self.count).color(cx.theme().primary)
                        .child(Button::new("increment").label("Add one").on_click(cx.listener(|this, _, _, cx| {
                            this.count = this.count.saturating_add(1); cx.notify();
                        }))))
                    .child(Button::new("reset-count").danger().label("Reset count")
                        .disabled(self.count == 0).on_click(cx.listener(|this, _, _, cx| {
                            this.count = 0; cx.notify();
                        })))
                    .child(div().id("count-status").test_support()
                        .aria_label(format!("Count: {}", self.count)).child(format!("Count: {}", self.count)))))
            .child(Panel::new().header(SectionHeader::new("Controlled tabs"))
                .child(TabBar::new("demo-tabs").selected_index(self.tab)
                    .child(Tab::new().label("Preview"))
                    .child(Tab::new().label("Details"))
                    .on_click(cx.listener(|this, index, _, cx| { this.tab = *index; cx.notify(); })))
                .child(div().id("tab-content").test_support()
                    .aria_label(if self.tab == 0 { "Preview content" } else { "Details content" })
                    .child(if self.tab == 0 { "Preview: this view is composed from upstream controls." }
                        else { "Details: selection lives in the Gallery entity, outside the TabBar." })))
            .into_any_element()
    }

    fn theme(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = cx.theme();
        let colors = t.color_tokens();
        let swatches = [
            ("Background", colors.background),
            ("Foreground", colors.foreground),
            ("Surface", colors.surface),
            ("Muted", colors.muted_foreground),
            ("Primary", colors.primary),
            ("Secondary", colors.secondary),
            ("Accent", colors.accent),
            ("Destructive", colors.destructive),
            ("Border", colors.border),
            ("Focus", colors.ring),
            ("Selection", colors.selection),
        ];
        div()
            .flex()
            .flex_col()
            .gap(SPACE_6)
            .child(SectionHeader::new("Semantic colors").description(
                "All controls read the same installed theme. Brand overrides persist across modes.",
            ))
            .child(
                Button::new("toggle-brand")
                    .label(if self.branded {
                        "Reset neutral palette"
                    } else {
                        "Preview blue accent"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.branded = !this.branded;
                        let mut options = theme::ThemeOptions {
                            mode: Some(cx.theme().mode),
                            ..appearance::options()
                        };
                        if this.branded {
                            options = options
                                .accent(
                                    theme::ThemeMode::Light,
                                    theme::Accent {
                                        color: gpui::rgb(0x245bb5).into(),
                                        foreground: gpui::rgb(0xffffff).into(),
                                        hover: gpui::rgb(0x1d4e9e).into(),
                                        active: gpui::rgb(0x173f80).into(),
                                        selection: gpui::rgb(0xdce8fa).into(),
                                    },
                                )
                                .accent(
                                    theme::ThemeMode::Dark,
                                    theme::Accent {
                                        color: gpui::rgb(0x91b9f7).into(),
                                        foreground: gpui::rgb(0x101827).into(),
                                        hover: gpui::rgb(0xa6c7fa).into(),
                                        active: gpui::rgb(0xbcd5fc).into(),
                                        selection: gpui::rgb(0x263e61).into(),
                                    },
                                );
                        }
                        theme::install(options, cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(SPACE_3)
                    .children(swatches.map(|(name, color)| {
                        div()
                            .w(px(144.))
                            .flex()
                            .flex_col()
                            .gap(SPACE_2)
                            .child(
                                div()
                                    .h(px(48.))
                                    .w_full()
                                    .rounded(t.radius)
                                    .border_1()
                                    .border_color(t.border)
                                    .bg(color),
                            )
                            .child(name)
                            .child(
                                div()
                                    .text_size(CAPTION_SIZE)
                                    .text_color(t.muted_foreground)
                                    .child(color.to_hex()),
                            )
                    })),
            )
            .child(
                Panel::new()
                    .header(
                        SectionHeader::new("Typography")
                            .description("System UI font with the platform's monospace fallback."),
                    )
                    .child(
                        div()
                            .text_size(TITLE_SIZE)
                            .child("Compact, readable hierarchy"),
                    )
                    .child("Body · 14 px · The quick brown fox jumps over the lazy dog.")
                    .child(
                        div()
                            .font_family(t.mono_font_family.clone())
                            .text_size(t.mono_font_size)
                            .child("let foundation = native + composable;"),
                    ),
            )
            .child(
                Button::new("theme-preview")
                    .primary()
                    .label("Primary appearance")
                    .disabled(true),
            )
            .into_any_element()
    }

    fn shell(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let preview = AppShell::new(div().id("shell-content").test_support().size_full().p(SPACE_4)
            .child(Panel::new().header(SectionHeader::new("Your application content"))
                .child("The application owns navigation, events, and state. Regions are ordinary elements.")))
            .sidebar_width(px(136.))
            .when(self.show_sidebar, |shell| shell.sidebar(div().id("shell-sidebar").test_support().p(SPACE_3).child("Navigation slot")))
            .when(self.show_header, |shell| shell.header(div().id("shell-header").test_support().p(SPACE_3).child("Application identity")))
            .when(self.show_toolbar, |shell| shell.toolbar(div().id("shell-toolbar").test_support().p(SPACE_3).child("Toolbar slot")))
            .when(self.show_status, |shell| shell.status_bar(div().id("shell-status").test_support().p(SPACE_2).child("Status slot")));
        div()
            .flex()
            .flex_col()
            .gap(SPACE_4)
            .child(
                SectionHeader::new("Compose only the regions you need").description(
                    "Toggle the regions below; content expands into the available space.",
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(SPACE_2)
                    .child(
                        Button::new("toggle-sidebar")
                            .label("Sidebar")
                            .selected(self.show_sidebar)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_sidebar = !this.show_sidebar;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("toggle-header")
                            .label("Header")
                            .selected(self.show_header)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_header = !this.show_header;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("toggle-toolbar")
                            .label("Toolbar")
                            .selected(self.show_toolbar)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_toolbar = !this.show_toolbar;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("toggle-status")
                            .label("Status bar")
                            .selected(self.show_status)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_status = !this.show_status;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .h(px(320.))
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(preview),
            )
            .into_any_element()
    }
}

impl gpui::Focusable for Gallery {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus.clone()
    }
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = match self.page {
            Page::Overview => self.overview(cx),
            Page::Components => self.components(cx),
            Page::Theme => self.theme(cx),
            Page::Shell => self.shell(cx),
        };
        if self.embedded {
            return div()
                .id("foundation-page")
                .track_focus(&self.focus)
                .size_full()
                .overflow_y_scroll()
                .p(SPACE_4)
                .child(page)
                .into_any_element();
        }
        let mode = cx.theme().mode;
        AppShell::new(
            div()
                .id("page-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(SPACE_6)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(SPACE_6)
                        .child(
                            div()
                                .id("page-title")
                                .test_support()
                                .aria_label(self.page.title())
                                .text_size(TITLE_SIZE)
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(self.page.title()),
                        )
                        .child(page),
                ),
        )
        .header(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(SPACE_4)
                .py(SPACE_3)
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Pintle GPUI Gallery"),
                )
                .child(
                    div()
                        .flex()
                        .gap(SPACE_2)
                        .child(
                            Button::new("mode-light")
                                .small()
                                .label("Light")
                                .selected(!mode.is_dark())
                                .on_click(|_, _, cx| theme::set_mode(theme::ThemeMode::Light, cx)),
                        )
                        .child(
                            Button::new("mode-dark")
                                .small()
                                .label("Dark")
                                .selected(mode.is_dark())
                                .on_click(|_, _, cx| theme::set_mode(theme::ThemeMode::Dark, cx)),
                        ),
                ),
        )
        .sidebar(
            div()
                .id("navigation")
                .size_full()
                .overflow_y_scroll()
                .p(SPACE_3)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(SPACE_1)
                        .children(Page::ALL.map(|page| {
                            Button::new(page.id())
                                .ghost()
                                .label(page.title())
                                .w_full()
                                .selected(self.page == page)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.page = page;
                                    cx.notify();
                                }))
                        })),
                ),
        )
        .status_bar(
            div()
                .flex()
                .justify_between()
                .px(SPACE_4)
                .py(SPACE_2)
                .text_size(CAPTION_SIZE)
                .text_color(cx.theme().muted_foreground)
                .child("Native GPUI · public foundation API")
                .child(format!("{} · v{}", mode.name(), env!("CARGO_PKG_VERSION"))),
        )
        .into_any_element()
    }
}

fn initialize(cx: &mut App) {
    pintle_gpui_kit::init(cx);
    theme::install(appearance::options(), cx);
    cx.on_action(|_: &Quit, cx| cx.quit());
    let quit_key = if cfg!(target_os = "macos") {
        "cmd-q"
    } else {
        "ctrl-q"
    };
    cx.bind_keys([
        gpui::KeyBinding::new(
            quit_key,
            Quit,
            if cfg!(target_os = "macos") {
                None
            } else {
                Some("!PintleTerminal")
            },
        ),
        gpui::KeyBinding::new("ctrl-`", ToggleTerminal, None),
        gpui::KeyBinding::new("cmd-w", CloseTerminal, Some("PintleTerminalPanel")),
        gpui::KeyBinding::new("cmd-k", OpenCommandPalette, None),
        gpui::KeyBinding::new("ctrl-k", OpenCommandPalette, Some("!PintleTerminal")),
        gpui::KeyBinding::new("cmd-j", ToggleInspector, None),
        gpui::KeyBinding::new("ctrl-j", ToggleInspector, Some("!PintleTerminal")),
        gpui::KeyBinding::new("cmd-b", ToggleSidebar, None),
        gpui::KeyBinding::new("ctrl-b", ToggleSidebar, Some("!PintleTerminal")),
    ]);
    cx.set_menus(vec![gpui::Menu {
        name: "Pintle GPUI Gallery".into(),
        items: vec![gpui::MenuItem::action("Quit", Quit)],
        disabled: false,
    }]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}

fn main() {
    gpui::application()
        .with_assets(gpui::assets::AllAssets)
        .run(|cx| {
            initialize(cx);
            gpui::open_window(appearance::window_options(cx), cx, |window, cx| {
                window.set_window_title("Pintle");
                cx.new(|cx| workbench::Workbench::new(window, cx))
            })
            .expect("open Gallery window");
            cx.activate(true);
        });
}

#[cfg(test)]
mod tests;
