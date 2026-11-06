//! Shared Pintle appearance.
use super::*;
pub fn options() -> theme::ThemeOptions {
    theme::ThemeOptions::default()
}
pub fn window_options(cx: &App) -> WindowOptions {
    pintle_gpui_kit::shell::workbench::window_options(cx)
}
