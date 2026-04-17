use textquest_common::window_title::{
    WindowTitleContext, class_long_name, class_short_name, default_window_title_format,
    render_window_title,
};

#[test]
fn renders_default_template_with_server_level_and_class() {
    let rendered = render_window_title(
        &default_window_title_format(),
        &WindowTitleContext {
            server: Some("Teek"),
            character: Some("Frostreaver"),
            level: Some(60),
            class_id: Some(2),
            zone_long_name: Some("Plane of Fire"),
            zone_short_name: Some("powfire"),
        },
    );

    assert_eq!(rendered, "[Teek] Frostreaver (60 CLR)");
}

#[test]
fn leaves_unknown_tokens_intact_and_blanks_missing_known_values() {
    let rendered = render_window_title(
        "{character}|{zone}|{class}|{missing}",
        &WindowTitleContext {
            server: None,
            character: Some("Aelrindel"),
            level: None,
            class_id: None,
            zone_long_name: None,
            zone_short_name: Some("nro"),
        },
    );

    assert_eq!(rendered, "Aelrindel|nro||{missing}");
}

#[test]
fn preserves_unicode_literals_around_tokens() {
    let rendered = render_window_title(
        "🧙 {character} à {zone}",
        &WindowTitleContext {
            server: None,
            character: Some("Aelrindel"),
            level: None,
            class_id: None,
            zone_long_name: Some("Plane of Knowledge"),
            zone_short_name: None,
        },
    );

    assert_eq!(rendered, "🧙 Aelrindel à Plane of Knowledge");
}

#[test]
fn class_name_helpers_cover_known_and_unknown_ids() {
    assert_eq!(class_short_name(2), Some("CLR"));
    assert_eq!(class_long_name(2), Some("Cleric"));
    assert_eq!(class_short_name(250), None);
    assert_eq!(class_long_name(250), None);
}
