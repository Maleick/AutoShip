//! Pixel-art sprite system for EQ class emblems with state-based animation.
//!
//! Inspired by gavraz/recon's Tamagotchi system. Each sprite is a 10x10 pixel
//! grid rendered with Unicode half-block characters (▀▄) for 2x vertical
//! resolution. Sprites are palette-indexed for easy color theming.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::eq::structs::{EqClass, StandState};

// ── Constants ────────────────────────────────────────────────────────

/// Width of a sprite in pixels.
pub const SPRITE_W: usize = 10;
/// Height of a sprite in pixels.
pub const SPRITE_H: usize = 10;
/// Terminal lines needed to render one sprite (2 pixel rows per line).
pub const SPRITE_RENDER_H: u16 = (SPRITE_H as u16).div_ceil(2); // 5

type Sprite = [[u8; SPRITE_W]; SPRITE_H];
type Palette = &'static [(u8, u8, u8)];

// ── Half-block renderer ──────────────────────────────────────────────
// Each terminal line encodes 2 pixel rows via ▀ (upper half) with
// fg = top pixel color, bg = bottom pixel color.

/// Render a sprite into terminal lines using Unicode half-block characters.
#[must_use]
pub fn render_sprite_lines(sprite: &Sprite, palette: Palette) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(SPRITE_RENDER_H as usize);

    for y in (0..SPRITE_H).step_by(2) {
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(SPRITE_W);

        for (x, &top) in sprite[y].iter().enumerate() {
            let bot = if y + 1 < SPRITE_H {
                sprite[y + 1][x]
            } else {
                0
            };

            if top == 0 && bot == 0 {
                spans.push(Span::raw(" "));
            } else if top == 0 {
                let (r, g, b) = palette[bot as usize];
                spans.push(Span::styled(
                    "\u{2584}", // ▄
                    Style::default().fg(Color::Rgb(r, g, b)),
                ));
            } else if bot == 0 {
                let (r, g, b) = palette[top as usize];
                spans.push(Span::styled(
                    "\u{2580}", // ▀
                    Style::default().fg(Color::Rgb(r, g, b)),
                ));
            } else {
                let (tr, tg, tb) = palette[top as usize];
                let (br, bg, bb) = palette[bot as usize];
                spans.push(Span::styled(
                    "\u{2580}", // ▀
                    Style::default()
                        .fg(Color::Rgb(tr, tg, tb))
                        .bg(Color::Rgb(br, bg, bb)),
                ));
            }
        }

        lines.push(Line::from(spans));
    }

    lines
}

// ── Public API ───────────────────────────────────────────────────────

/// Get the sprite + palette for a given class and state, with animation frame.
#[must_use]
pub fn class_sprite(class: Option<&EqClass>, state: &StandState, tick: u64) -> Vec<Line<'static>> {
    let frame = (tick / 2) as usize; // animate every 2 ticks (~500ms at 250ms refresh)

    let (sprite, palette) = match state {
        StandState::Dead => (&SPRITE_DEAD[frame % 2], PAL_DEAD),
        StandState::Feigned => (&SPRITE_FEIGNED[0], PAL_FEIGNED),
        StandState::Sitting => {
            let pal = class_palette(class);
            (&SPRITE_SITTING[frame % 2], pal)
        }
        _ => {
            // Standing / combat / other — use class-specific sprite
            let pal = class_palette(class);
            let sprites = class_sprites(class);
            let idx = frame % sprites.len();
            (&sprites[idx], pal)
        }
    };

    render_sprite_lines(sprite, palette)
}

// ── Class palettes ───────────────────────────────────────────────────
// Index 0 is always transparent. Each class gets a themed color set.

fn class_palette(class: Option<&EqClass>) -> Palette {
    match class {
        Some(EqClass::Warrior | EqClass::Berserker) => PAL_WARRIOR,
        Some(EqClass::Cleric) => PAL_CLERIC,
        Some(EqClass::Paladin) => PAL_PALADIN,
        Some(EqClass::ShadowKnight) => PAL_SHADOWKNIGHT,
        Some(EqClass::Ranger | EqClass::Druid | EqClass::Beastlord) => PAL_RANGER,
        Some(EqClass::Monk | EqClass::Rogue) => PAL_MONK,
        Some(EqClass::Bard) => PAL_BARD,
        Some(EqClass::Enchanter) => PAL_ENCHANTER,
        Some(EqClass::Wizard | EqClass::Magician) => PAL_WIZARD,
        Some(EqClass::Necromancer) => PAL_NECROMANCER,
        Some(EqClass::Shaman) => PAL_SHAMAN,
        None => PAL_GENERIC,
    }
}

fn class_sprites(class: Option<&EqClass>) -> &'static [Sprite] {
    match class {
        Some(EqClass::Warrior | EqClass::Berserker) => &SPRITE_WARRIOR,
        Some(EqClass::Cleric) => &SPRITE_CLERIC,
        Some(EqClass::Paladin) => &SPRITE_PALADIN,
        Some(EqClass::ShadowKnight) => &SPRITE_SHADOWKNIGHT,
        Some(EqClass::Ranger | EqClass::Druid | EqClass::Beastlord) => &SPRITE_RANGER,
        Some(EqClass::Monk | EqClass::Rogue) => &SPRITE_MONK,
        Some(EqClass::Bard) => &SPRITE_BARD,
        Some(EqClass::Enchanter) => &SPRITE_ENCHANTER,
        Some(EqClass::Wizard | EqClass::Magician) => &SPRITE_WIZARD,
        Some(EqClass::Necromancer) => &SPRITE_NECROMANCER,
        Some(EqClass::Shaman) => &SPRITE_SHAMAN,
        None => &SPRITE_GENERIC,
    }
}

// ── Color palettes ───────────────────────────────────────────────────
// 1=primary body, 2=secondary/darker, 3=accent, 4=highlight, 5=detail, 6=emblem, 7=feet/base

// Warrior: steel and red
const PAL_WARRIOR: Palette = &[
    (0, 0, 0),       // 0: transparent
    (180, 180, 200), // 1: steel armor
    (130, 130, 150), // 2: dark steel
    (200, 50, 50),   // 3: red accent (plume/cape)
    (220, 220, 240), // 4: bright steel highlight
    (100, 100, 120), // 5: shadow
    (255, 80, 80),   // 6: sword/emblem glow
    (160, 160, 180), // 7: boots
];

// Cleric: white and gold
const PAL_CLERIC: Palette = &[
    (0, 0, 0),
    (240, 240, 250), // 1: white robes
    (200, 200, 220), // 2: robe shadow
    (255, 215, 0),   // 3: gold holy symbol
    (255, 255, 255), // 4: bright white glow
    (180, 180, 200), // 5: detail
    (255, 230, 100), // 6: holy cross glow
    (200, 200, 210), // 7: sandals
];

// Paladin: silver and blue
const PAL_PALADIN: Palette = &[
    (0, 0, 0),
    (200, 210, 230), // 1: silver-blue armor
    (150, 165, 190), // 2: darker armor
    (100, 150, 255), // 3: holy blue accent
    (230, 235, 250), // 4: bright highlight
    (120, 140, 170), // 5: shadow
    (140, 180, 255), // 6: blue glow
    (170, 180, 200), // 7: boots
];

// Shadow Knight: dark purple and green
const PAL_SHADOWKNIGHT: Palette = &[
    (0, 0, 0),
    (80, 60, 100),  // 1: dark armor
    (50, 35, 70),   // 2: darker
    (0, 200, 80),   // 3: sickly green accent
    (120, 90, 150), // 4: highlight
    (40, 25, 50),   // 5: deep shadow
    (0, 255, 100),  // 6: evil glow
    (60, 45, 80),   // 7: boots
];

// Ranger: forest green and brown
const PAL_RANGER: Palette = &[
    (0, 0, 0),
    (80, 140, 60),  // 1: forest green
    (55, 100, 40),  // 2: darker green
    (160, 120, 60), // 3: leather brown
    (120, 180, 90), // 4: light green highlight
    (40, 80, 30),   // 5: shadow
    (200, 160, 80), // 6: bow accent
    (100, 75, 40),  // 7: boots
];

// Monk: orange and brown (martial arts)
const PAL_MONK: Palette = &[
    (0, 0, 0),
    (220, 160, 50),  // 1: orange gi
    (180, 120, 30),  // 2: darker
    (255, 255, 255), // 3: white belt/wraps
    (255, 200, 80),  // 4: highlight
    (140, 90, 20),   // 5: shadow
    (255, 220, 100), // 6: chi glow
    (160, 100, 30),  // 7: sandals
];

// Bard: purple and gold (performer)
const PAL_BARD: Palette = &[
    (0, 0, 0),
    (150, 80, 200),  // 1: purple tunic
    (110, 50, 160),  // 2: darker purple
    (255, 215, 0),   // 3: gold trim
    (190, 120, 240), // 4: light purple
    (80, 40, 120),   // 5: shadow
    (255, 230, 80),  // 6: instrument glow
    (120, 60, 170),  // 7: boots
];

// Enchanter: teal and silver (mystical)
const PAL_ENCHANTER: Palette = &[
    (0, 0, 0),
    (60, 180, 200),  // 1: teal robes
    (40, 130, 150),  // 2: darker teal
    (200, 200, 255), // 3: silver sparkle
    (100, 220, 240), // 4: bright teal
    (30, 100, 120),  // 5: shadow
    (180, 180, 255), // 6: mesmerize glow
    (50, 150, 170),  // 7: sandals
];

// Wizard: blue and white (arcane power)
const PAL_WIZARD: Palette = &[
    (0, 0, 0),
    (60, 80, 200),   // 1: blue robes
    (40, 55, 150),   // 2: darker blue
    (200, 200, 255), // 3: arcane white
    (100, 120, 240), // 4: bright blue
    (30, 40, 120),   // 5: shadow
    (150, 150, 255), // 6: arcane glow
    (50, 65, 170),   // 7: sandals
];

// Necromancer: black and sickly green
const PAL_NECROMANCER: Palette = &[
    (0, 0, 0),
    (60, 60, 60),  // 1: dark robes
    (35, 35, 35),  // 2: darker
    (0, 200, 80),  // 3: necro green
    (90, 90, 90),  // 4: grey highlight
    (20, 20, 20),  // 5: deep shadow
    (0, 255, 100), // 6: death glow
    (45, 45, 45),  // 7: boots
];

// Shaman: earth tones with spirit blue
const PAL_SHAMAN: Palette = &[
    (0, 0, 0),
    (160, 120, 80),  // 1: earth brown
    (120, 85, 55),   // 2: darker brown
    (100, 180, 255), // 3: spirit blue
    (200, 160, 110), // 4: light brown
    (80, 55, 30),    // 5: shadow
    (140, 200, 255), // 6: spirit glow
    (130, 95, 60),   // 7: boots
];

// Generic fallback: grey
const PAL_GENERIC: Palette = &[
    (0, 0, 0),
    (160, 160, 160), // 1: grey
    (120, 120, 120), // 2: darker grey
    (200, 200, 200), // 3: light accent
    (190, 190, 190), // 4: highlight
    (80, 80, 80),    // 5: shadow
    (220, 220, 220), // 6: glow
    (140, 140, 140), // 7: boots
];

// ── Shared state sprites ─────────────────────────────────────────────

// Dead: X eyes, grey, 2-frame flicker
const PAL_DEAD: Palette = &[
    (0, 0, 0),
    (120, 120, 120), // 1: grey body
    (80, 80, 80),    // 2: darker
    (200, 50, 50),   // 3: red X eyes
    (160, 160, 160), // 4: highlight
    (60, 60, 60),    // 5: shadow
    (255, 60, 60),   // 6: death red
    (100, 100, 100), // 7: base
];

const SPRITE_DEAD: [Sprite; 2] = [
    [
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 1, 1, 1, 1, 1, 1, 0],
        [0, 1, 3, 0, 3, 3, 0, 3, 1, 0],
        [0, 1, 0, 3, 0, 0, 3, 0, 1, 0],
        [0, 1, 3, 0, 3, 3, 0, 3, 1, 0],
        [0, 1, 1, 1, 1, 1, 1, 1, 1, 0],
        [0, 0, 2, 1, 1, 1, 1, 2, 0, 0],
        [0, 0, 0, 7, 0, 0, 7, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    [
        [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 1, 1, 1, 1, 1, 1, 0],
        [0, 1, 0, 3, 0, 0, 3, 0, 1, 0],
        [0, 1, 3, 0, 3, 3, 0, 3, 1, 0],
        [0, 1, 0, 3, 0, 0, 3, 0, 1, 0],
        [0, 1, 1, 1, 1, 1, 1, 1, 1, 0],
        [0, 0, 2, 1, 1, 1, 1, 2, 0, 0],
        [0, 0, 0, 7, 0, 0, 7, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Feigned death: lying flat
const PAL_FEIGNED: Palette = &[
    (0, 0, 0),
    (160, 160, 180), // 1: body
    (120, 120, 140), // 2: darker
    (200, 200, 220), // 3: face
    (60, 60, 80),    // 4: closed eyes
    (100, 100, 120), // 5: shadow
    (140, 140, 160), // 6: detail
    (80, 80, 100),   // 7: ground shadow
];

const SPRITE_FEIGNED: [Sprite; 1] = [[
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 7, 7, 7, 7, 7, 7, 7, 7, 0],
    [0, 1, 1, 3, 1, 1, 1, 1, 1, 0],
    [0, 2, 4, 4, 2, 1, 1, 1, 2, 0],
    [0, 7, 7, 7, 7, 7, 7, 7, 7, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
]];

// Sitting: universal meditate pose, 2-frame subtle animation
const SPRITE_SITTING: [Sprite; 2] = [
    [
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 1, 3, 1, 1, 3, 1, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 2, 2, 2, 2, 1, 1, 0],
        [0, 1, 2, 7, 7, 7, 7, 2, 1, 0],
        [0, 0, 7, 7, 7, 7, 7, 7, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    [
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 6, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 1, 5, 5, 5, 5, 1, 0, 6],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 2, 2, 2, 2, 1, 1, 0],
        [0, 1, 2, 7, 7, 7, 7, 2, 1, 0],
        [0, 0, 7, 7, 7, 7, 7, 7, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// ── Class-specific emblem sprites ────────────────────────────────────
// Each class has an iconic emblem (not a humanoid figure) with 3 animation frames:
//   Frame 0: Base emblem
//   Frame 1: Active / glowing
//   Frame 2: Full power with particle effects (sparkles around edges)

// Warrior: crossed swords emblem
const SPRITE_WARRIOR: [Sprite; 3] = [
    // Frame 0: base crossed swords
    [
        [0, 0, 1, 0, 0, 0, 0, 1, 0, 0],
        [0, 0, 0, 1, 0, 0, 1, 0, 0, 0],
        [0, 0, 0, 0, 4, 4, 0, 0, 0, 0],
        [0, 0, 0, 4, 2, 2, 4, 0, 0, 0],
        [0, 0, 4, 0, 2, 2, 0, 4, 0, 0],
        [0, 4, 0, 0, 2, 2, 0, 0, 4, 0],
        [0, 0, 0, 3, 5, 5, 3, 0, 0, 0],
        [0, 0, 0, 0, 5, 5, 0, 0, 0, 0],
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: active / blades glow
    [
        [0, 0, 6, 0, 0, 0, 0, 6, 0, 0],
        [0, 0, 0, 6, 0, 0, 6, 0, 0, 0],
        [0, 0, 0, 0, 4, 4, 0, 0, 0, 0],
        [0, 0, 0, 4, 2, 2, 4, 0, 0, 0],
        [0, 0, 4, 0, 2, 2, 0, 4, 0, 0],
        [0, 4, 0, 0, 2, 2, 0, 0, 4, 0],
        [0, 0, 0, 6, 5, 5, 6, 0, 0, 0],
        [0, 0, 0, 0, 5, 5, 0, 0, 0, 0],
        [0, 0, 0, 0, 6, 6, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: full power with sparkles
    [
        [6, 0, 6, 0, 0, 0, 0, 6, 0, 6],
        [0, 0, 0, 6, 0, 0, 6, 0, 0, 0],
        [0, 6, 0, 0, 4, 4, 0, 0, 6, 0],
        [0, 0, 0, 4, 6, 6, 4, 0, 0, 0],
        [0, 0, 4, 0, 2, 2, 0, 4, 0, 0],
        [0, 4, 0, 0, 2, 2, 0, 0, 4, 0],
        [6, 0, 0, 6, 5, 5, 6, 0, 0, 6],
        [0, 0, 0, 0, 5, 5, 0, 0, 0, 0],
        [0, 0, 6, 0, 6, 6, 0, 6, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Cleric: ankh / holy cross emblem
const SPRITE_CLERIC: [Sprite; 3] = [
    // Frame 0: base ankh
    [
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 3, 6, 6, 3, 0, 0, 0],
        [0, 0, 0, 3, 6, 6, 3, 0, 0, 0],
        [0, 0, 3, 3, 3, 3, 3, 3, 0, 0],
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 2, 2, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: healing glow
    [
        [0, 0, 0, 6, 3, 3, 6, 0, 0, 0],
        [0, 0, 0, 3, 6, 6, 3, 0, 0, 0],
        [0, 0, 6, 3, 6, 6, 3, 6, 0, 0],
        [0, 6, 3, 3, 3, 3, 3, 3, 6, 0],
        [0, 0, 0, 6, 3, 3, 6, 0, 0, 0],
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 6, 3, 3, 6, 0, 0, 0],
        [0, 0, 0, 0, 2, 2, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: full power with sparkles
    [
        [0, 0, 6, 0, 4, 4, 0, 6, 0, 0],
        [0, 6, 0, 4, 6, 6, 4, 0, 6, 0],
        [0, 0, 0, 3, 6, 6, 3, 0, 0, 0],
        [6, 0, 4, 3, 3, 3, 3, 4, 0, 6],
        [0, 0, 0, 6, 4, 4, 6, 0, 0, 0],
        [0, 6, 0, 0, 3, 3, 0, 0, 6, 0],
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 6, 0, 3, 3, 0, 6, 0, 0],
        [0, 0, 0, 0, 4, 4, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Paladin: shield with cross emblem
const SPRITE_PALADIN: [Sprite; 3] = [
    // Frame 0: base shield
    [
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 1, 3, 3, 1, 1, 1, 0],
        [0, 1, 1, 1, 3, 3, 1, 1, 1, 0],
        [0, 1, 3, 3, 3, 3, 3, 3, 1, 0],
        [0, 1, 1, 1, 3, 3, 1, 1, 1, 0],
        [0, 0, 1, 1, 3, 3, 1, 1, 0, 0],
        [0, 0, 0, 1, 3, 3, 1, 0, 0, 0],
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 0, 2, 2, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: holy strike — cross glows
    [
        [0, 0, 4, 1, 1, 1, 1, 4, 0, 0],
        [0, 1, 1, 1, 6, 6, 1, 1, 1, 0],
        [0, 1, 1, 1, 6, 6, 1, 1, 1, 0],
        [0, 1, 6, 6, 6, 6, 6, 6, 1, 0],
        [0, 1, 1, 1, 6, 6, 1, 1, 1, 0],
        [0, 0, 1, 1, 3, 3, 1, 1, 0, 0],
        [0, 0, 0, 1, 3, 3, 1, 0, 0, 0],
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 0, 2, 2, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: divine aura with sparkles
    [
        [6, 0, 4, 1, 1, 1, 1, 4, 0, 6],
        [0, 1, 1, 1, 6, 6, 1, 1, 1, 0],
        [6, 1, 1, 1, 6, 6, 1, 1, 1, 6],
        [0, 1, 6, 6, 4, 4, 6, 6, 1, 0],
        [6, 1, 1, 1, 6, 6, 1, 1, 1, 6],
        [0, 0, 1, 1, 6, 6, 1, 1, 0, 0],
        [0, 6, 0, 1, 3, 3, 1, 0, 6, 0],
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, 6, 0, 4, 4, 0, 6, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Shadow Knight: skull with horns emblem
const SPRITE_SHADOWKNIGHT: [Sprite; 3] = [
    // Frame 0: base skull
    [
        [0, 4, 0, 0, 0, 0, 0, 0, 4, 0],
        [0, 0, 4, 0, 0, 0, 0, 4, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 1, 3, 1, 1, 3, 1, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 0, 1, 5, 5, 1, 0, 0, 0],
        [0, 0, 1, 5, 1, 1, 5, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: evil glow — eyes ignite
    [
        [0, 4, 0, 0, 0, 0, 0, 0, 4, 0],
        [0, 0, 4, 0, 6, 6, 0, 4, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 1, 6, 1, 1, 6, 1, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 0, 1, 5, 5, 1, 0, 0, 0],
        [0, 0, 1, 5, 1, 1, 5, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: full power with sparkles
    [
        [6, 4, 0, 0, 6, 6, 0, 0, 4, 6],
        [0, 0, 4, 6, 0, 0, 6, 4, 0, 0],
        [0, 6, 0, 1, 1, 1, 1, 0, 6, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 6, 1, 6, 1, 1, 6, 1, 6, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 6, 0, 1, 5, 5, 1, 0, 6, 0],
        [0, 0, 1, 5, 1, 1, 5, 1, 0, 0],
        [0, 0, 6, 1, 1, 1, 1, 6, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Ranger: bow and arrow emblem
const SPRITE_RANGER: [Sprite; 3] = [
    // Frame 0: base bow with arrow
    [
        [0, 0, 0, 0, 6, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 6, 0, 0, 0, 0, 0],
        [0, 0, 3, 0, 6, 0, 3, 0, 0, 0],
        [0, 3, 0, 0, 6, 0, 0, 3, 0, 0],
        [0, 3, 0, 0, 6, 0, 0, 3, 0, 0],
        [0, 3, 0, 0, 6, 0, 0, 3, 0, 0],
        [0, 0, 3, 0, 6, 0, 3, 0, 0, 0],
        [0, 0, 0, 3, 1, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: drawn bow — string pulled, arrowhead glows
    [
        [0, 0, 0, 6, 4, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 6, 0, 0, 0, 0, 0],
        [0, 0, 3, 0, 6, 0, 3, 0, 0, 0],
        [0, 3, 0, 0, 6, 0, 0, 3, 0, 0],
        [0, 3, 0, 0, 6, 0, 0, 0, 3, 0],
        [0, 3, 0, 0, 6, 0, 0, 3, 0, 0],
        [0, 0, 3, 0, 6, 0, 3, 0, 0, 0],
        [0, 0, 0, 3, 1, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: arrow released with sparkles
    [
        [0, 6, 0, 4, 4, 0, 0, 0, 6, 0],
        [0, 0, 0, 0, 4, 0, 0, 0, 0, 0],
        [0, 0, 3, 0, 0, 0, 3, 0, 0, 0],
        [6, 3, 0, 0, 6, 0, 0, 3, 0, 0],
        [0, 3, 0, 6, 0, 6, 0, 3, 0, 0],
        [0, 3, 0, 0, 6, 0, 0, 3, 6, 0],
        [0, 0, 3, 0, 0, 0, 3, 0, 0, 0],
        [0, 0, 0, 3, 1, 3, 0, 0, 0, 0],
        [0, 6, 0, 0, 1, 0, 0, 6, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Monk: open hand / fist emblem
const SPRITE_MONK: [Sprite; 3] = [
    // Frame 0: open palm
    [
        [0, 0, 1, 0, 1, 0, 1, 0, 0, 0],
        [0, 0, 1, 0, 1, 0, 1, 0, 0, 0],
        [0, 1, 1, 0, 1, 0, 1, 1, 0, 0],
        [0, 1, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 1, 1, 2, 2, 1, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 0, 2, 2, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: chi glowing in palm
    [
        [0, 0, 1, 0, 1, 0, 1, 0, 0, 0],
        [0, 0, 1, 0, 1, 0, 1, 0, 0, 0],
        [0, 1, 1, 0, 1, 0, 1, 1, 0, 0],
        [0, 1, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 1, 1, 6, 6, 1, 0, 0, 0],
        [0, 0, 1, 6, 6, 6, 6, 0, 0, 0],
        [0, 0, 1, 1, 6, 6, 1, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 0, 2, 2, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: chi explosion with sparkles
    [
        [6, 0, 1, 0, 1, 0, 1, 0, 0, 6],
        [0, 0, 1, 0, 1, 0, 1, 0, 0, 0],
        [0, 1, 1, 6, 1, 6, 1, 1, 0, 0],
        [6, 1, 1, 1, 6, 6, 1, 1, 6, 0],
        [0, 0, 1, 6, 4, 4, 6, 0, 0, 0],
        [0, 6, 1, 6, 4, 4, 6, 0, 6, 0],
        [0, 0, 1, 1, 6, 6, 1, 0, 0, 0],
        [0, 0, 6, 1, 1, 1, 0, 6, 0, 0],
        [0, 0, 0, 0, 2, 2, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Bard: harp emblem with musical notes
const SPRITE_BARD: [Sprite; 3] = [
    // Frame 0: base harp
    [
        [0, 0, 0, 3, 3, 3, 0, 0, 0, 0],
        [0, 0, 3, 0, 0, 0, 3, 0, 0, 0],
        [0, 3, 0, 6, 6, 6, 0, 3, 0, 0],
        [0, 3, 0, 6, 6, 6, 0, 3, 0, 0],
        [0, 3, 0, 6, 6, 6, 0, 3, 0, 0],
        [0, 0, 3, 0, 6, 0, 3, 0, 0, 0],
        [0, 0, 0, 3, 6, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 3, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: playing — notes float, strings vibrate
    [
        [0, 0, 0, 3, 3, 3, 0, 0, 4, 0],
        [0, 0, 3, 0, 0, 0, 3, 0, 0, 0],
        [0, 3, 0, 4, 4, 4, 0, 3, 0, 0],
        [4, 3, 0, 6, 6, 6, 0, 3, 0, 0],
        [0, 3, 0, 4, 4, 4, 0, 3, 0, 0],
        [0, 0, 3, 0, 6, 0, 3, 0, 4, 0],
        [0, 0, 0, 3, 6, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 3, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: full performance with sparkles
    [
        [0, 4, 0, 3, 3, 3, 0, 4, 0, 0],
        [0, 0, 3, 0, 0, 0, 3, 0, 0, 4],
        [6, 3, 0, 4, 4, 4, 0, 3, 0, 0],
        [0, 3, 6, 6, 6, 6, 6, 3, 0, 0],
        [4, 3, 0, 4, 4, 4, 0, 3, 6, 0],
        [0, 0, 3, 6, 6, 6, 3, 0, 0, 0],
        [0, 4, 0, 3, 4, 3, 0, 4, 0, 0],
        [0, 0, 0, 0, 3, 0, 0, 0, 0, 0],
        [0, 0, 6, 1, 1, 1, 6, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Enchanter: mesmerizing eye emblem
const SPRITE_ENCHANTER: [Sprite; 3] = [
    // Frame 0: base eye
    [
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 1, 6, 6, 1, 1, 1, 0],
        [0, 1, 1, 6, 3, 3, 6, 1, 1, 0],
        [0, 1, 1, 1, 6, 6, 1, 1, 1, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: mesmerize active — glow radiates
    [
        [0, 0, 0, 0, 6, 6, 0, 0, 0, 0],
        [0, 0, 0, 4, 1, 1, 4, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 4, 1, 1, 6, 6, 1, 1, 4, 0],
        [0, 1, 1, 6, 3, 3, 6, 1, 1, 0],
        [0, 4, 1, 1, 6, 6, 1, 1, 4, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 0, 4, 1, 1, 4, 0, 0, 0],
        [0, 0, 0, 0, 6, 6, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: full mesmerize with sparkles
    [
        [0, 6, 0, 0, 6, 6, 0, 0, 6, 0],
        [0, 0, 6, 4, 1, 1, 4, 6, 0, 0],
        [0, 6, 1, 4, 1, 1, 4, 1, 6, 0],
        [6, 4, 1, 1, 6, 6, 1, 1, 4, 6],
        [0, 1, 1, 6, 4, 4, 6, 1, 1, 0],
        [6, 4, 1, 1, 6, 6, 1, 1, 4, 6],
        [0, 6, 1, 4, 1, 1, 4, 1, 6, 0],
        [0, 0, 6, 4, 1, 1, 4, 6, 0, 0],
        [0, 6, 0, 0, 6, 6, 0, 0, 6, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Wizard: arcane flame / starburst emblem
const SPRITE_WIZARD: [Sprite; 3] = [
    // Frame 0: base arcane flame
    [
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 3, 4, 4, 3, 0, 0, 0],
        [0, 0, 0, 4, 6, 6, 4, 0, 0, 0],
        [0, 0, 3, 6, 6, 6, 6, 3, 0, 0],
        [0, 0, 4, 6, 1, 1, 6, 4, 0, 0],
        [0, 0, 1, 4, 1, 1, 4, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 2, 2, 2, 2, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: casting — flame grows
    [
        [0, 0, 0, 3, 3, 3, 3, 0, 0, 0],
        [0, 0, 3, 4, 4, 4, 4, 3, 0, 0],
        [0, 0, 4, 6, 6, 6, 6, 4, 0, 0],
        [0, 3, 6, 6, 6, 6, 6, 6, 3, 0],
        [0, 0, 4, 6, 4, 4, 6, 4, 0, 0],
        [0, 0, 1, 4, 1, 1, 4, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 2, 2, 2, 2, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: full power nuke with sparkles
    [
        [6, 0, 6, 3, 3, 3, 3, 6, 0, 6],
        [0, 6, 3, 4, 4, 4, 4, 3, 6, 0],
        [0, 0, 4, 6, 3, 3, 6, 4, 0, 0],
        [6, 3, 6, 6, 6, 6, 6, 6, 3, 6],
        [0, 0, 4, 6, 4, 4, 6, 4, 0, 0],
        [0, 6, 1, 4, 6, 6, 4, 1, 6, 0],
        [0, 0, 6, 1, 1, 1, 1, 6, 0, 0],
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 6, 0, 2, 2, 2, 2, 0, 6, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Necromancer: skull with death aura emblem
const SPRITE_NECROMANCER: [Sprite; 3] = [
    // Frame 0: base skull
    [
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 1, 4, 4, 4, 4, 1, 0, 0],
        [0, 0, 1, 3, 4, 4, 3, 1, 0, 0],
        [0, 0, 1, 4, 4, 4, 4, 1, 0, 0],
        [0, 0, 0, 1, 2, 2, 1, 0, 0, 0],
        [0, 0, 1, 2, 4, 4, 2, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: death aura — green wisps
    [
        [0, 0, 0, 3, 0, 0, 3, 0, 0, 0],
        [0, 0, 3, 1, 1, 1, 1, 3, 0, 0],
        [0, 0, 1, 4, 4, 4, 4, 1, 0, 0],
        [0, 3, 1, 6, 4, 4, 6, 1, 3, 0],
        [0, 0, 1, 4, 4, 4, 4, 1, 0, 0],
        [0, 0, 0, 1, 2, 2, 1, 0, 0, 0],
        [0, 0, 1, 2, 4, 4, 2, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 3, 3, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: full death power with sparkles
    [
        [0, 3, 0, 3, 6, 6, 3, 0, 3, 0],
        [0, 0, 3, 1, 1, 1, 1, 3, 0, 0],
        [3, 0, 1, 4, 4, 4, 4, 1, 0, 3],
        [0, 3, 1, 6, 4, 4, 6, 1, 3, 0],
        [0, 0, 1, 4, 6, 6, 4, 1, 0, 0],
        [3, 0, 0, 1, 2, 2, 1, 0, 0, 3],
        [0, 0, 1, 2, 4, 4, 2, 1, 0, 0],
        [0, 3, 0, 1, 1, 1, 1, 0, 3, 0],
        [0, 0, 3, 0, 6, 6, 0, 3, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Shaman: tribal spirit mask emblem
const SPRITE_SHAMAN: [Sprite; 3] = [
    // Frame 0: base mask
    [
        [0, 0, 0, 3, 3, 3, 3, 0, 0, 0],
        [0, 0, 3, 3, 3, 3, 3, 3, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 3, 1, 1, 3, 1, 1, 0],
        [0, 1, 1, 1, 1, 1, 1, 1, 1, 0],
        [0, 1, 1, 2, 1, 1, 2, 1, 1, 0],
        [0, 0, 1, 1, 2, 2, 1, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 2, 2, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 1: spirit active — eyes glow
    [
        [0, 0, 6, 3, 3, 3, 3, 6, 0, 0],
        [0, 0, 3, 3, 3, 3, 3, 3, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 6, 1, 1, 6, 1, 1, 0],
        [0, 1, 1, 1, 1, 1, 1, 1, 1, 0],
        [0, 1, 1, 2, 1, 1, 2, 1, 1, 0],
        [0, 0, 1, 1, 6, 6, 1, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 6, 2, 2, 6, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
    // Frame 2: full spirit with sparkles
    [
        [6, 0, 6, 3, 6, 6, 3, 6, 0, 6],
        [0, 6, 3, 3, 3, 3, 3, 3, 6, 0],
        [0, 0, 1, 4, 1, 1, 4, 1, 0, 0],
        [6, 1, 1, 6, 1, 1, 6, 1, 1, 6],
        [0, 1, 1, 1, 1, 1, 1, 1, 1, 0],
        [6, 1, 1, 6, 1, 1, 6, 1, 1, 6],
        [0, 0, 1, 1, 6, 6, 1, 1, 0, 0],
        [0, 6, 0, 1, 1, 1, 1, 0, 6, 0],
        [0, 0, 6, 0, 6, 6, 0, 6, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// Generic fallback: diamond emblem
const SPRITE_GENERIC: [Sprite; 2] = [
    // Frame 0: base diamond
    [
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 1, 3, 3, 1, 1, 1, 0],
        [0, 1, 1, 3, 3, 3, 3, 1, 1, 0],
        [0, 1, 1, 3, 3, 3, 3, 1, 1, 0],
        [0, 1, 1, 1, 3, 3, 1, 1, 1, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
    ],
    // Frame 1: glow
    [
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 1, 6, 6, 1, 1, 1, 0],
        [0, 1, 1, 6, 3, 3, 6, 1, 1, 0],
        [0, 1, 1, 6, 3, 3, 6, 1, 1, 0],
        [0, 1, 1, 1, 6, 6, 1, 1, 1, 0],
        [0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 0, 0, 0],
        [0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_produces_correct_line_count() {
        let lines = render_sprite_lines(&SPRITE_WARRIOR[0], PAL_WARRIOR);
        assert_eq!(lines.len(), SPRITE_RENDER_H as usize);
    }

    #[test]
    fn class_sprite_returns_lines_for_all_classes() {
        let classes = [
            EqClass::Warrior,
            EqClass::Cleric,
            EqClass::Paladin,
            EqClass::Ranger,
            EqClass::ShadowKnight,
            EqClass::Druid,
            EqClass::Monk,
            EqClass::Bard,
            EqClass::Rogue,
            EqClass::Shaman,
            EqClass::Necromancer,
            EqClass::Wizard,
            EqClass::Magician,
            EqClass::Enchanter,
            EqClass::Beastlord,
            EqClass::Berserker,
        ];
        for class in &classes {
            let lines = class_sprite(Some(class), &StandState::Standing, 0);
            assert_eq!(lines.len(), 5, "Failed for {:?}", class);
        }
    }

    #[test]
    fn dead_sprite_animates() {
        let frame0 = class_sprite(Some(&EqClass::Warrior), &StandState::Dead, 0);
        let frame1 = class_sprite(Some(&EqClass::Warrior), &StandState::Dead, 2);
        // Different frames should produce different output
        assert_ne!(format!("{:?}", frame0), format!("{:?}", frame1));
    }

    #[test]
    fn sitting_sprite_works() {
        let lines = class_sprite(Some(&EqClass::Cleric), &StandState::Sitting, 0);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn none_class_uses_generic() {
        let lines = class_sprite(None, &StandState::Standing, 0);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn feigned_sprite_works() {
        let lines = class_sprite(Some(&EqClass::Monk), &StandState::Feigned, 0);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn sprite_constants_correct() {
        assert_eq!(SPRITE_W, 10);
        assert_eq!(SPRITE_H, 10);
        assert_eq!(SPRITE_RENDER_H, 5);
    }

    #[test]
    fn warrior_sprite_has_three_frames() {
        assert_eq!(SPRITE_WARRIOR.len(), 3);
    }

    #[test]
    fn dead_sprite_has_two_frames() {
        assert_eq!(SPRITE_DEAD.len(), 2);
    }

    #[test]
    fn sitting_sprite_has_two_frames() {
        assert_eq!(SPRITE_SITTING.len(), 2);
    }

    #[test]
    fn class_sprite_animation_cycles() {
        // tick / 2 gives the animation frame, mod sprite count (3 frames for warrior)
        let t0 = class_sprite(Some(&EqClass::Warrior), &StandState::Standing, 0);
        let t2 = class_sprite(Some(&EqClass::Warrior), &StandState::Standing, 2);
        let t6 = class_sprite(Some(&EqClass::Warrior), &StandState::Standing, 6);
        // Frame 0 and frame 1 (tick=2) should differ
        assert_ne!(format!("{:?}", t0), format!("{:?}", t2));
        // Frame 0 and frame 3 (tick=6, wraps back to frame 0 with 3 frames) should be same
        assert_eq!(format!("{:?}", t0), format!("{:?}", t6));
    }

    #[test]
    fn class_palette_returns_palette_for_all_classes() {
        let classes = [
            EqClass::Warrior,
            EqClass::Berserker,
            EqClass::Cleric,
            EqClass::Paladin,
            EqClass::ShadowKnight,
            EqClass::Ranger,
            EqClass::Druid,
            EqClass::Beastlord,
            EqClass::Monk,
            EqClass::Rogue,
            EqClass::Bard,
            EqClass::Enchanter,
            EqClass::Wizard,
            EqClass::Magician,
            EqClass::Necromancer,
            EqClass::Shaman,
        ];
        for class in &classes {
            let pal = class_palette(Some(class));
            assert!(!pal.is_empty(), "Palette empty for {:?}", class);
        }
    }

    #[test]
    fn class_palette_none_returns_generic() {
        let pal = class_palette(None);
        assert!(!pal.is_empty());
    }

    #[test]
    fn class_sprites_returns_non_empty_for_all_classes() {
        let classes = [
            EqClass::Warrior,
            EqClass::Cleric,
            EqClass::Ranger,
            EqClass::Monk,
            EqClass::Enchanter,
            EqClass::Wizard,
            EqClass::Necromancer,
        ];
        for class in &classes {
            let sprites = class_sprites(Some(class));
            assert!(!sprites.is_empty(), "No sprites for {:?}", class);
        }
    }

    #[test]
    fn render_sprite_handles_transparent_pixels() {
        // Create a sprite that is all zeros (transparent)
        let sprite: Sprite = [[0; SPRITE_W]; SPRITE_H];
        let lines = render_sprite_lines(&sprite, PAL_GENERIC);
        assert_eq!(lines.len(), SPRITE_RENDER_H as usize);
        // All pixels should be spaces
        for line in &lines {
            for span in line.spans.iter() {
                assert_eq!(span.content.as_ref(), " ");
            }
        }
    }

    #[test]
    fn feigned_sprite_does_not_animate() {
        // Feigned uses frame [0] only, so different ticks should produce same output
        let t0 = class_sprite(Some(&EqClass::Monk), &StandState::Feigned, 0);
        let t5 = class_sprite(Some(&EqClass::Monk), &StandState::Feigned, 10);
        assert_eq!(format!("{:?}", t0), format!("{:?}", t5));
    }
}
