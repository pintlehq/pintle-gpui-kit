//! Complete neutral control palette; no upstream colored defaults leak into controls.
use gpui_kit::component::{ThemeConfig, ThemeConfigColors, ThemeMode};

pub(super) fn config(mode: ThemeMode) -> ThemeConfig {
    let dark = mode.is_dark();
    let mut colors = ThemeConfigColors::default();
    macro_rules! paint {
        ($light:literal, $dark:literal; $($field:ident),+ $(,)?) => {
            $(colors.$field = Some(if dark { $dark } else { $light }.into());)+
        };
    }
    // Background
    paint!("#fafafa", "#171717"; background, list, scrollbar, tab_bar, table);
    // Surface
    paint!("#ffffff", "#202020"; accordion, button, group_box, description_list_label, list_even, list_head, popover, sidebar, slider_thumb, switch_thumb, tab, table_even, table_head, table_foot, title_bar, status_bar);
    // Foreground
    paint!("#202020", "#ededed"; accent_foreground, button_foreground, button_secondary_foreground, group_box_foreground, caret, foreground, link, link_active, link_hover, popover_foreground, progress_bar, ring, scrollbar_thumb, scrollbar_thumb_hover, secondary_foreground, sidebar_accent_foreground, sidebar_foreground, slider_bar, tab_active_foreground, tab_foreground, table_head_foreground, table_foot_foreground);
    // Muted
    paint!("#626262", "#aaaaaa"; group_box_title_foreground, description_list_label_foreground, muted_foreground);
    // Border
    paint!("#d4d4d4", "#444444"; border, chart_grid, drag_border, input, list_active_border, sidebar_border, table_active_border, table_row_border, title_bar_border, status_bar_border, window_border);
    // Hover
    paint!("#eeeeee", "#2e2e2e"; button_hover, button_secondary, button_secondary_hover, chart_1, chart_2, chart_3, chart_4, chart_5, chart_bullish, chart_bearish, list_hover, muted, secondary, secondary_hover, skeleton, tab_bar_segmented, table_hover);
    // Selection
    paint!("#dedede", "#414141"; accent, button_active, button_secondary_active, drop_target, list_active, secondary_active, selection, sidebar_accent, switch, tab_active, table_active);
    // Primary
    paint!("#262626", "#e5e5e5"; button_danger, button_info, button_primary, button_success, button_warning, danger, info, primary, sidebar_primary, success, warning);
    // Inverse
    paint!("#ffffff", "#171717"; button_danger_foreground, button_info_foreground, button_primary_foreground, button_success_foreground, button_warning_foreground, danger_foreground, info_foreground, primary_foreground, sidebar_primary_foreground, success_foreground, warning_foreground);
    // Primary hover
    paint!("#404040", "#cccccc"; button_danger_hover, button_info_hover, button_primary_hover, button_success_hover, button_warning_hover, danger_hover, info_hover, primary_hover, success_hover, warning_hover);
    // Primary active
    paint!("#525252", "#b8b8b8"; button_danger_active, button_info_active, button_primary_active, button_success_active, button_warning_active, danger_active, info_active, primary_active, success_active, warning_active);
    // Overlay
    paint!("#00000066", "#00000099"; overlay);
    ThemeConfig {
        name: if dark { "Pintle Dark" } else { "Pintle Light" }.into(),
        mode,
        font_size: Some(14.),
        mono_font_size: Some(13.),
        radius: Some(4),
        radius_lg: Some(8),
        shadow: Some(false),
        colors,
        ..Default::default()
    }
}
