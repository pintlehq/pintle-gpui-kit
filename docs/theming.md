# Theming

`ThemeOptions::default()` provides the Pintle workspace `light` and `dark`
`gpui_kit::component::ThemeConfig` values. Both are installed into upstream
`Theme`, which remains the single source of active color and radius values.

```rust,ignore
use pintle_gpui_kit::{kit, theme::{self, ThemeOptions, ThemeMode}};

let mut options = ThemeOptions::default();
options.light.name = "My product Light".into();
options.dark.name = "My product Dark".into();
options.dark.colors.background = Some("#181818".into());
options.mode = Some(ThemeMode::Dark);
pintle_gpui_kit::init_with_theme(options, cx);
theme::set_mode(ThemeMode::Light, cx);
```

`mode: None` samples system appearance on installation. Changes afterward are
explicit; the foundation neither stores preferences nor installs an ongoing
system-appearance observer. Themes are App-wide, not per-window.

`ThemeOptions::accent(mode, Accent { ... })` accepts typed colors for the primary
background, foreground, hover, active, and selection states. It updates primary
controls and focus consistently for that mode. Apply it before directly editing
`light`/`dark` for final overrides; later writes win. Supply both modes for
branding that remains appropriate across appearances. The Gallery contains a
complete example with a default-preset reset.

Read presentation through upstream `ActiveTheme`, e.g. `cx.theme().background`
or `cx.theme().color_tokens()`. Semantic surface maps to `popover`, destructive
to `danger`, and focus to `ring`. Primary controls have their own state tokens;
changing only `primary` does not override every button. Use the accent builder
or explicitly edit those upstream fields.

`install` registers both palettes via `Theme::update`, then calls `Theme::change`
to apply the selected configuration and synchronize styled/base tokens and all
windows. Switching modes therefore retains branding. Invalid upstream color
strings follow Kit's fallback behavior; prefer typed Accent colors or validate
custom configurations in the consuming application.

## Geometry and typography

The default body and monospace sizes are 13 px, with 6/8 px radii.
Kit handles platform font fallback. The facade’s opt-in `assets` feature registers
bundled JetBrains Mono once during initialization. Its SIL OFL license travels
with the font files. `ThemeOptions::neutral()` retains the original system-font
palette, 14 px body and 4/8 px radii.

`theme::tokens` contains the 4-pixel spacing scale, title/caption sizes, and default
sidebar width. These are Pintle conventions, not a second theme engine. Kit's
spacing token getter returns defaults, and only selected typography/radius
fields are applied from its semantic configuration. Pintle does not claim
unsupported dynamic spacing or typography customization.

Actions must carry explicit labels; do not rely on hue alone. Unit tests retain
contrast checks for the neutral preset and verify the shared workspace defaults.
Applications providing overrides own their resulting contrast and accessibility.

Brand identity and logos are ordinary caller-owned elements in shell slots.
Applications can register a custom Kit asset source and use upstream images/icons.

## Shared workspace preset

The kit owns the former Gallery palette. Light content/chrome/tabs are
`#FFFFFF` / `#FAFAFD` / `#EAEAEA`; dark equivalents are `#121314` / `#191A1B` /
`#202122`. Both configurations survive theme toggling and accent reset.

Chrome uses the system UI font at 13 px. Data cells, Details values and Output
use embedded JetBrains Mono (SIL OFL 1.1, license alongside the fonts) at 13 px.
The title and panel bars are 35 px; folder document strips are 36 px, tree/breadcrumb/status
rows 22 px, resource toolbar 32 px and virtualized table rows 28 px. Sidebar,
editor groups and bottom panel use 1 px borders and 8 px corners with 6 px gaps.
The editor group owns clipping; the table has no nested card border or inset.

Tree selection fills the row with a thin focus outline when its keyboard scope
is focused; no custom vertical selection stripe is drawn. Ancestor guides are
1 px and appear on hover/focus. Document tabs are 96–220 px wide, truncate long
names with tooltips, use native folder curves with a shared content seam and no top rule. Close
buttons remain visible on inactive tabs. Preview labels remain italic.

Navigation nodes accept Kit icons and documents may supply an `icon_path`.
The Gallery registers `AllAssets`, because the default `Assets` bundle excludes
some catalog icons. Fonts and icons are embedded with no runtime downloads.

Kit `TitleBar::window_options()` and `TitleBar` provide custom window chrome,
macOS traffic lights, dragging, zoom, resize and Quit. The window remains native.
