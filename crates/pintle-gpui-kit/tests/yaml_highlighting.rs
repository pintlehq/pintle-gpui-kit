#![cfg(feature = "tree-sitter-yaml")]

use pintle_gpui_kit::kit::component::highlighter::{HighlightTheme, SyntaxHighlighter};

#[test]
fn yaml_feature_produces_token_colors_in_both_themes() {
    let text = "# cluster settings\nname: \"development\"\nreplicas: 3\nenabled: true\n";
    let mut parser = SyntaxHighlighter::new("yaml");
    assert!(parser.update(None, &text.into(), None));
    assert!(parser.tree().is_some());
    let mut theme_colors = Vec::new();
    for theme in [
        HighlightTheme::default_light(),
        HighlightTheme::default_dark(),
    ] {
        let spans = parser.styles(&(0..text.len()), theme.as_ref());
        let colors: Vec<_> = ["# cluster settings", "name", "\"development\"", "3", "true"]
            .into_iter()
            .map(|token| {
                let offset = text.find(token).unwrap();
                spans
                    .iter()
                    .find_map(|(range, style)| {
                        range.contains(&offset).then_some(style.color).flatten()
                    })
                    .unwrap_or_else(|| panic!("No color for {token}"))
            })
            .collect();
        assert!(colors.iter().any(|color| *color != colors[0]));
        assert_ne!(colors[0], colors[2], "comments and strings must differ");
        theme_colors.push(colors);
    }
    assert_ne!(theme_colors[0], theme_colors[1]);
}

#[test]
fn yaml_highlighting_handles_incomplete_unicode_multidocument_and_long_lines() {
    let mut parser = SyntaxHighlighter::new("yaml");
    for text in [
        "name: \"unfinished\nreplicas: ".to_string(),
        "---\nname: \"สวัสดี 🌊\"\n---\nenabled: false\n".into(),
        format!(
            "description: \"{}\"\nreplicas: 42\n",
            "ภาษาไทย ".repeat(1000)
        ),
    ] {
        assert!(parser.update(None, &text.as_str().into(), None));
        let spans = parser.styles(&(0..text.len()), HighlightTheme::default_dark().as_ref());
        assert!(spans.iter().any(|(_, style)| style.color.is_some()));
        for (range, _) in spans {
            assert!(range.end <= text.len());
            assert!(text.is_char_boundary(range.start));
            assert!(text.is_char_boundary(range.end));
        }
    }
}
