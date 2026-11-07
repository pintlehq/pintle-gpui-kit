use super::{Accent, Theme, ThemeMode, ThemeOptions, install, set_mode};
use gpui_kit::{TestAppContext, component::try_parse_color, px, rgb};
use std::rc::Rc;

fn resolve(options: &ThemeOptions, mode: ThemeMode) -> Theme {
    let mut theme = Theme::default();
    theme.apply_config(&Rc::new(if mode.is_dark() {
        options.dark.clone()
    } else {
        options.light.clone()
    }));
    theme
}

fn luminance(color: gpui_kit::Hsla) -> f32 {
    let rgba = color.to_rgb();
    let linear = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgba.r) + 0.7152 * linear(rgba.g) + 0.0722 * linear(rgba.b)
}

fn contrast(a: gpui_kit::Hsla, b: gpui_kit::Hsla) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn neutral_palettes_have_readable_text_and_complete_control_states() {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        let t = resolve(&ThemeOptions::neutral(), mode);
        for (fg, bg) in [
            (t.foreground, t.background),
            (t.muted_foreground, t.background),
            (t.primary_foreground, t.primary),
            (t.danger_foreground, t.danger),
            (t.popover_foreground, t.popover),
        ] {
            assert!(
                contrast(fg, bg) >= 4.5,
                "insufficient text contrast in {mode:?}"
            );
            assert!(fg.s.abs() < 0.001 && bg.s.abs() < 0.001);
        }
        assert_ne!(t.button_primary, t.button_primary_hover);
        assert_ne!(t.button_primary_hover, t.button_primary_active);
        assert_ne!(t.background, t.selection);
        assert_eq!(t.font_size, px(14.));
    }
}

#[test]
fn explicit_config_overrides_win_after_accent_builder() {
    let mut options = ThemeOptions::default().accent(
        ThemeMode::Light,
        Accent {
            color: rgb(0x245bb5).into(),
            foreground: rgb(0xffffff).into(),
            hover: rgb(0x1d4e9e).into(),
            active: rgb(0x173f80).into(),
            selection: rgb(0xdce8fa).into(),
        },
    );
    options.light.colors.button_primary = Some("#123456".into());
    let light = resolve(&options, ThemeMode::Light);
    assert_eq!(light.button_primary, rgb(0x123456).into());
    assert_eq!(light.primary, rgb(0x245bb5).into());
    assert_eq!(
        resolve(&options, ThemeMode::Dark).primary,
        resolve(&ThemeOptions::default(), ThemeMode::Dark).primary
    );
}

#[gpui_kit::test]
fn switching_modes_retains_overrides_and_synchronizes_base(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let mut options = ThemeOptions {
            mode: Some(ThemeMode::Light),
            ..Default::default()
        };
        options.light.colors.primary = Some("#123456".into());
        options.dark.colors.primary = Some("#abcdef".into());
        options.dark.radius = Some(7);
        install(options, cx);
        for (mode, hex, radius) in [
            (ThemeMode::Dark, "#abcdef", 7.),
            (ThemeMode::Light, "#123456", 6.),
            (ThemeMode::Dark, "#abcdef", 7.),
        ] {
            set_mode(mode, cx);
            let theme = Theme::global(cx);
            assert_eq!(theme.highlight_theme.appearance, mode);
            let expected_highlight = if mode.is_dark() {
                gpui_kit::component::highlighter::HighlightTheme::default_dark()
            } else {
                gpui_kit::component::highlighter::HighlightTheme::default_light()
            };
            assert_eq!(
                theme.highlight_theme.style("string"),
                expected_highlight.style("string")
            );
            assert_eq!(theme.primary, try_parse_color(hex).unwrap());
            assert_eq!(theme.tokens.primary.color, theme.primary);
            assert_eq!(theme.radius, px(radius));
            let base = gpui_kit::base::Theme::global(cx);
            assert_eq!(base.tokens.colors.primary, theme.primary);
            assert_eq!(base.tokens.radius.md, theme.radius);
        }
    });
}

#[test]
fn default_workspace_preset_preserves_gallery_identity() {
    for (mode, background, sidebar, tabs) in [
        (ThemeMode::Light, 0xffffff, 0xfafafd, 0xeaeaea),
        (ThemeMode::Dark, 0x121314, 0x191a1b, 0x202122),
    ] {
        let theme = resolve(&ThemeOptions::default(), mode);
        assert_eq!(theme.background, rgb(background).into());
        assert_eq!(theme.sidebar, rgb(sidebar).into());
        assert!(theme.selection.s > 0.5);
        assert_ne!(theme.selection, theme.table_active);
        assert_ne!(theme.selection, theme.list_active);
        assert_eq!(theme.tab, rgb(tabs).into());
        assert_eq!(theme.font_size, px(13.));
        assert_eq!(theme.radius, px(6.));
        assert_eq!(theme.mono_font_family.as_ref(), "JetBrains Mono");
    }
}
