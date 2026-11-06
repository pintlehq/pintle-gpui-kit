//! Optional bundled fonts; initialized once with the facade.
use std::borrow::Cow;
pub(crate) fn install(cx: &gpui_kit::App) {
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-SemiBold.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf")),
        ])
        .expect("load bundled JetBrains Mono fonts");
}
