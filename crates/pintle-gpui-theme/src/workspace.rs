pub(super) fn options() -> super::ThemeOptions {
    let mut options = super::ThemeOptions::neutral();
    for config in [&mut options.light, &mut options.dark] {
        let dark = config.mode.is_dark();
        config.name = if dark {
            "Pintle Workspace Dark"
        } else {
            "Pintle Workspace Light"
        }
        .into();
        config.font_family = Some(".SystemUIFont".into());
        config.mono_font_family = Some("JetBrains Mono".into());
        config.font_size = Some(13.);
        config.mono_font_size = Some(13.);
        config.radius = Some(6);
        config.radius_lg = Some(8);
        let c = &mut config.colors;
        macro_rules! paint {
            ($light:literal, $dark:literal; $($field:ident),+ $(,)?) => {
                $(c.$field = Some(if dark { $dark } else { $light }.into());)+
            };
        }
        paint!("#ffffff", "#121314"; background, table, table_even, list, list_even, tab_active, button, accordion, scrollbar);
        paint!("#fafafd", "#191a1b"; sidebar, title_bar, status_bar, table_head, table_foot, list_head);
        paint!("#eaeaea", "#202122"; tab_bar, tab);
        paint!("#ffffff", "#1c1c1f"; popover);
        paint!("#ffffff", "#19191c"; group_box, description_list_label);
        paint!("#3b3b3b", "#fafafd"; foreground, button_foreground, tab_active_foreground, sidebar_foreground, popover_foreground, group_box_foreground, accent_foreground, secondary_foreground, button_secondary_foreground, sidebar_accent_foreground);
        paint!("#3f3f46", "#cccccc"; muted_foreground, tab_foreground, table_head_foreground, table_foot_foreground, group_box_title_foreground, description_list_label_foreground);
        paint!("#e5e5e5", "#2b2b2b"; border, sidebar_border, table_row_border, title_bar_border, status_bar_border, window_border, chart_grid);
        paint!("#e5e5e5", "#3f3f46"; input);
        paint!("#f4f4f5", "#242526"; muted, list_hover, table_hover, button_hover, button_secondary_hover, secondary_hover, skeleton, tab_bar_segmented);
        paint!("#f4f4f5", "#2b2b2b"; accent, secondary, button_secondary);
        paint!("#e5e5e5", "#37373d"; selection, list_active, table_active, sidebar_accent, button_active, button_secondary_active, secondary_active, drop_target);
        paint!("#005fb8", "#0078d4"; primary, button_primary, sidebar_primary, progress_bar, slider_bar, caret, link, list_active_border, table_active_border, drag_border);
        paint!("#ffffff", "#ffffff"; primary_foreground, button_primary_foreground, sidebar_primary_foreground);
        paint!("#005fb8", "#0078d4"; ring);
        paint!("#616161", "#9d9d9d"; tab_foreground);
        paint!("#3b3b3b", "#ffffff"; tab_active_foreground);
        paint!("#3f3f46", "#e5e5e5"; primary_hover, button_primary_hover, link_hover);
        paint!("#52525b", "#d4d4d8"; primary_active, button_primary_active, link_active);
        paint!("#be3c38", "#e36b6b"; danger, button_danger);
        paint!("#ffffff", "#1c0c0c"; danger_foreground, button_danger_foreground);
        paint!("#227a40", "#6dc28a"; success, button_success);
        paint!("#a1700c", "#e8b339"; warning, button_warning);
        paint!("#21639e", "#7bb2e8"; info, button_info);
    }
    options
}
