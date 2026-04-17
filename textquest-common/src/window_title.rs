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
