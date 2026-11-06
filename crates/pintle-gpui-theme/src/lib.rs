//! Workspace and neutral themes using Kit's global theme, configuration and mode switching.
//!
//! Call [`install`] after `gpui_kit::init`, or use `pintle_gpui_kit::init`.
//! Colors and radii live in Kit. [`tokens`] holds only Pintle layout conventions.
use std::rc::Rc;

pub use gpui_kit::component::{ActiveTheme, Theme, ThemeConfig, ThemeMode};
use gpui_kit::{App, Hsla};

mod palette;
pub mod tokens;
mod workspace;

/// A complete accessible accent family. Callers own foreground contrast.
#[derive(Clone, Copy, Debug)]
pub struct Accent {
    pub color: Hsla,
    pub foreground: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    pub selection: Hsla,
}

/// Both complete configurations are retained by Kit when modes change.
///
/// Start with `default()`, optionally call `accent`, then edit `light`/`dark`
/// fields for final overrides. Set `mode` to force a startup mode; `None`
/// samples the system appearance once. No preference storage is performed.
#[derive(Clone, Debug)]
pub struct ThemeOptions {
    pub light: ThemeConfig,
    pub dark: ThemeConfig,
    pub mode: Option<ThemeMode>,
}

impl Default for ThemeOptions {
    fn default() -> Self {
        workspace::options()
    }
}

impl ThemeOptions {
    /// Neutral system-font theme, independent of the Pintle workspace preset.
    pub fn neutral() -> Self {
        Self {
            light: palette::config(ThemeMode::Light),
            dark: palette::config(ThemeMode::Dark),
            mode: None,
        }
    }
}

impl ThemeOptions {
    /// Apply a mode-specific accent to primary controls, focus and selection.
    /// Later direct configuration edits take precedence.
    pub fn accent(mut self, mode: ThemeMode, accent: Accent) -> Self {
        let config = if mode.is_dark() {
            &mut self.dark
        } else {
            &mut self.light
        };
        let colors = &mut config.colors;
        macro_rules! set {
            ($value:expr; $($field:ident),+ $(,)?) => {
                $(colors.$field = Some(config_color($value).into());)+
            };
        }
        set!(accent.color; primary, button_primary, sidebar_primary, ring, caret,
            progress_bar, slider_bar, link, drag_border);
        set!(accent.foreground; primary_foreground, button_primary_foreground,
            sidebar_primary_foreground);
        set!(accent.hover; primary_hover, button_primary_hover, link_hover);
        set!(accent.active; primary_active, button_primary_active, link_active);
        set!(accent.selection; accent, selection, list_active, tab_active, sidebar_accent);
        self
    }
}

// Kit configuration takes strings. Round rather than truncate RGB channels:
// an HSL round-trip can put an exact 8-bit value just below its original byte.
fn config_color(color: Hsla) -> String {
    let rgba = color.to_rgb();
    let byte = |channel: f32| (channel.clamp(0., 1.) * 255.).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        byte(rgba.r),
        byte(rgba.g),
        byte(rgba.b),
        byte(rgba.a)
    )
}

/// Install both palettes and refresh all windows. Kit must already be initialized.
pub fn install(mut options: ThemeOptions, cx: &mut App) {
    let mode = options
        .mode
        .unwrap_or_else(|| cx.window_appearance().into());
    // A supplied config cannot accidentally reverse the two mode slots.
    options.light.mode = ThemeMode::Light;
    options.dark.mode = ThemeMode::Dark;
    Theme::update(cx, |theme| {
        theme.light_theme = Rc::new(options.light);
        theme.dark_theme = Rc::new(options.dark);
    });
    Theme::change(mode, None, cx);
}

/// Switch modes without losing installed configuration or branding.
pub fn set_mode(mode: ThemeMode, cx: &mut App) {
    Theme::change(mode, None, cx);
}

#[cfg(test)]
mod tests;
