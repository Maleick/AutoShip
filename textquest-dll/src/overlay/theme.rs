/// Visual theme selection for the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// RGBA float colors for a given theme.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThemeColors {
    /// Window background fill.
    pub background: [f32; 4],
    /// Primary text color.
    pub foreground: [f32; 4],
    /// Draggable header fill.
    pub header: [f32; 4],
    /// Window border.
    pub border: [f32; 4],
    /// Button idle fill.
    pub button: [f32; 4],
    /// Button hovered fill.
    pub button_hover: [f32; 4],
    /// Button pressed fill.
    pub button_active: [f32; 4],
}

impl Theme {
    pub fn colors(self) -> ThemeColors {
        match self {
            Theme::Dark => ThemeColors {
                background: [0.08, 0.08, 0.10, 0.92],
                foreground: [0.90, 0.90, 0.92, 1.00],
                header: [0.15, 0.15, 0.20, 1.00],
                border: [0.30, 0.30, 0.40, 1.00],
                button: [0.22, 0.22, 0.30, 1.00],
                button_hover: [0.30, 0.30, 0.42, 1.00],
                button_active: [0.40, 0.40, 0.60, 1.00],
            },
            Theme::Light => ThemeColors {
                background: [0.94, 0.94, 0.96, 0.95],
                foreground: [0.10, 0.10, 0.12, 1.00],
                header: [0.80, 0.80, 0.88, 1.00],
                border: [0.60, 0.60, 0.70, 1.00],
                button: [0.82, 0.82, 0.90, 1.00],
                button_hover: [0.72, 0.72, 0.84, 1.00],
                button_active: [0.62, 0.62, 0.78, 1.00],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_dark() {
        assert_eq!(Theme::default(), Theme::Dark);
    }

    #[test]
    fn colors_alpha_nonzero() {
        for theme in [Theme::Dark, Theme::Light] {
            let c = theme.colors();
            assert!(c.background[3] > 0.0);
            assert!(c.foreground[3] > 0.0);
        }
    }

    #[test]
    fn dark_light_differ() {
        let dark = Theme::Dark.colors();
        let light = Theme::Light.colors();
        assert_ne!(dark.background, light.background);
    }

    #[test]
    fn theme_roundtrip_serde() {
        let json = serde_json::to_string(&Theme::Light).unwrap();
        let parsed: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Theme::Light);
    }
}
