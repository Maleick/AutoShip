use std::borrow::Cow;

/// Default in-game title template used when no character override is present.
#[must_use]
pub fn default_window_title_format() -> String {
    "[{server}] {character} ({level} {class_short})".to_string()
}

/// Render a title template using the current character context.
///
/// Supported tokens:
/// - `{server}`
/// - `{character}`
/// - `{level}`
/// - `{class}` (long name, e.g. `Cleric`)
/// - `{class_short}` (short name, e.g. `CLR`)
/// - `{zone}` (prefers long name, falls back to short)
/// - `{zone_long}`
/// - `{zone_short}`
#[must_use]
pub fn render_window_title(template: &str, ctx: &WindowTitleContext<'_>) -> String {
    let mut rendered = String::with_capacity(template.len() + 16);
    let mut last_literal_start = 0usize;
    let mut chars = template.char_indices().peekable();

    while let Some((open, ch)) = chars.next() {
        if ch != '{' {
            continue;
        }

        let mut close = None;
        for (idx, inner_ch) in chars.by_ref() {
            if inner_ch == '}' {
                close = Some(idx);
                break;
            }
        }

        let Some(close) = close else {
            break;
        };

        rendered.push_str(&template[last_literal_start..open]);

        let token = &template[open + '{'.len_utf8()..close];
        if let Some(value) = resolve_token(token, ctx) {
            rendered.push_str(value.as_ref());
        } else {
            rendered.push_str(&template[open..close + '}'.len_utf8()]);
        }
        last_literal_start = close + '}'.len_utf8();
    }

    rendered.push_str(&template[last_literal_start..]);
    rendered
}

fn resolve_token<'a>(token: &str, ctx: &'a WindowTitleContext<'a>) -> Option<Cow<'a, str>> {
    match token {
        "server" => Some(Cow::Borrowed(ctx.server.unwrap_or_default())),
        "character" => Some(Cow::Borrowed(ctx.character.unwrap_or_default())),
        "level" => Some(
            ctx.level
                .map_or(Cow::Borrowed(""), |level| Cow::Owned(level.to_string())),
        ),
        "class" => Some(Cow::Borrowed(
            ctx.class_id.and_then(class_long_name).unwrap_or_default(),
        )),
        "class_short" => Some(Cow::Borrowed(
            ctx.class_id.and_then(class_short_name).unwrap_or_default(),
        )),
        "zone" => Some(Cow::Borrowed(
            ctx.zone_long_name
                .or(ctx.zone_short_name)
                .unwrap_or_default(),
        )),
        "zone_long" => Some(Cow::Borrowed(ctx.zone_long_name.unwrap_or_default())),
        "zone_short" => Some(Cow::Borrowed(ctx.zone_short_name.unwrap_or_default())),
        _ => None,
    }
}

/// Input values available while formatting a window title.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindowTitleContext<'a> {
    pub server: Option<&'a str>,
    pub character: Option<&'a str>,
    pub level: Option<u8>,
    pub class_id: Option<u8>,
    pub zone_long_name: Option<&'a str>,
    pub zone_short_name: Option<&'a str>,
}

/// Convert an EQ class id into a long display name.
#[must_use]
pub fn class_long_name(class_id: u8) -> Option<&'static str> {
    match class_id {
        1 => Some("Warrior"),
        2 => Some("Cleric"),
        3 => Some("Paladin"),
        4 => Some("Ranger"),
        5 => Some("Shadow Knight"),
        6 => Some("Druid"),
        7 => Some("Monk"),
        8 => Some("Bard"),
        9 => Some("Rogue"),
        10 => Some("Shaman"),
        11 => Some("Necromancer"),
        12 => Some("Wizard"),
        13 => Some("Magician"),
        14 => Some("Enchanter"),
        15 => Some("Beastlord"),
        16 => Some("Berserker"),
        _ => None,
    }
}

/// Convert an EQ class id into the conventional short class label.
#[must_use]
pub fn class_short_name(class_id: u8) -> Option<&'static str> {
    match class_id {
        1 => Some("WAR"),
        2 => Some("CLR"),
        3 => Some("PAL"),
        4 => Some("RNG"),
        5 => Some("SK"),
        6 => Some("DRU"),
        7 => Some("MNK"),
        8 => Some("BRD"),
        9 => Some("ROG"),
        10 => Some("SHM"),
        11 => Some("NEC"),
        12 => Some("WIZ"),
        13 => Some("MAG"),
        14 => Some("ENC"),
        15 => Some("BST"),
        16 => Some("BER"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_ctx<'a>() -> WindowTitleContext<'a> {
        WindowTitleContext {
            server: Some("Firiona Vie"),
            character: Some("Aelrindel"),
            level: Some(65),
            class_id: Some(2), // Cleric
            zone_long_name: Some("The Nexus"),
            zone_short_name: Some("nexus"),
        }
    }

    // ── class_long_name ──────────────────────────────────────────────────

    #[test]
    fn class_long_name_covers_all_sixteen_classes() {
        let expected = [
            (1u8, "Warrior"),
            (2, "Cleric"),
            (3, "Paladin"),
            (4, "Ranger"),
            (5, "Shadow Knight"),
            (6, "Druid"),
            (7, "Monk"),
            (8, "Bard"),
            (9, "Rogue"),
            (10, "Shaman"),
            (11, "Necromancer"),
            (12, "Wizard"),
            (13, "Magician"),
            (14, "Enchanter"),
            (15, "Beastlord"),
            (16, "Berserker"),
        ];
        for (id, name) in expected {
            assert_eq!(class_long_name(id), Some(name), "class id {id}");
        }
    }

    #[test]
    fn class_long_name_returns_none_for_unknown_id() {
        assert_eq!(class_long_name(0), None);
        assert_eq!(class_long_name(17), None);
        assert_eq!(class_long_name(255), None);
    }

    // ── class_short_name ─────────────────────────────────────────────────

    #[test]
    fn class_short_name_covers_all_sixteen_classes() {
        let expected = [
            (1u8, "WAR"),
            (2, "CLR"),
            (3, "PAL"),
            (4, "RNG"),
            (5, "SK"),
            (6, "DRU"),
            (7, "MNK"),
            (8, "BRD"),
            (9, "ROG"),
            (10, "SHM"),
            (11, "NEC"),
            (12, "WIZ"),
            (13, "MAG"),
            (14, "ENC"),
            (15, "BST"),
            (16, "BER"),
        ];
        for (id, abbrev) in expected {
            assert_eq!(class_short_name(id), Some(abbrev), "class id {id}");
        }
    }

    #[test]
    fn class_short_name_returns_none_for_unknown_id() {
        assert_eq!(class_short_name(0), None);
        assert_eq!(class_short_name(255), None);
    }

    // ── render_window_title ──────────────────────────────────────────────

    #[test]
    fn render_window_title_default_format() {
        let ctx = full_ctx();
        let title = render_window_title(&default_window_title_format(), &ctx);
        assert_eq!(title, "[Firiona Vie] Aelrindel (65 CLR)");
    }

    #[test]
    fn render_window_title_expands_all_tokens() {
        let ctx = full_ctx();
        let template =
            "{server} {character} {level} {class} {class_short} {zone} {zone_long} {zone_short}";
        let title = render_window_title(template, &ctx);
        assert_eq!(
            title,
            "Firiona Vie Aelrindel 65 Cleric CLR The Nexus The Nexus nexus"
        );
    }

    #[test]
    fn render_window_title_zone_falls_back_to_short_name() {
        let ctx = WindowTitleContext {
            server: Some("Antonica"),
            character: Some("Bryndas"),
            level: Some(20),
            class_id: Some(1),
            zone_long_name: None,
            zone_short_name: Some("ecommons"),
        };
        let title = render_window_title("{zone}", &ctx);
        assert_eq!(title, "ecommons");
    }

    #[test]
    fn render_window_title_unknown_token_preserved_verbatim() {
        let ctx = full_ctx();
        let title = render_window_title("prefix {unknown_token} suffix", &ctx);
        assert_eq!(title, "prefix {unknown_token} suffix");
    }

    #[test]
    fn render_window_title_missing_context_uses_empty_string() {
        let ctx = WindowTitleContext::default();
        // The literal "] " and " (" each contribute a space, so two spaces appear
        // between the server and character sections when both are empty strings.
        let title = render_window_title("[{server}] {character} ({level} {class_short})", &ctx);
        assert_eq!(title, "[]  ( )");
    }

    #[test]
    fn render_window_title_unclosed_brace_emits_remaining_literal() {
        let ctx = full_ctx();
        // An unclosed `{` causes the renderer to stop resolving tokens; the
        // tail of the template (including the `{`) is appended verbatim.
        let title = render_window_title("text {unclosed", &ctx);
        assert_eq!(title, "text {unclosed");
    }

    #[test]
    fn render_window_title_plain_string_passes_through() {
        let ctx = full_ctx();
        let title = render_window_title("No tokens here", &ctx);
        assert_eq!(title, "No tokens here");
    }
}
