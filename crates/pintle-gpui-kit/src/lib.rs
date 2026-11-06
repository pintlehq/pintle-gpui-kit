//! Public entry point for Pintle's native desktop UI foundation.
#![cfg_attr(all(feature = "shell", feature = "assets"), doc = include_str!("../../../docs/integration.md"))]
pub use gpui_kit as kit;
#[cfg(feature = "components")]
pub use pintle_gpui_components as components;
#[cfg(feature = "shell")]
pub use pintle_gpui_shell as shell;
#[cfg(feature = "terminal")]
pub use pintle_gpui_terminal as terminal;
#[cfg(feature = "theme")]
pub use pintle_gpui_theme as theme;

#[cfg(feature = "assets")]
mod assets;

struct Initialized;
impl kit::Global for Initialized {}

/// Initialize Kit once, before constructing views or opening windows.
pub fn init(cx: &mut kit::App) {
    if !cx.has_global::<Initialized>() {
        kit::init(cx);
        #[cfg(feature = "assets")]
        assets::install(cx);
        #[cfg(feature = "theme")]
        theme::install(theme::ThemeOptions::default(), cx);
        cx.set_global(Initialized);
    }
}

/// Initialize Kit and install or replace the application's Pintle theme.
#[cfg(feature = "theme")]
pub fn init_with_theme(options: theme::ThemeOptions, cx: &mut kit::App) {
    init(cx);
    theme::install(options, cx);
}
