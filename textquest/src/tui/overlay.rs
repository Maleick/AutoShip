//! In-game overlay UI scaffolding shared with the TUI state model.
//!
//! The DLL renderer can consume [`OverlayFrame`] without owning gameplay state.

use std::collections::HashMap;

use super::app::App;

/// Renderer boundary for an in-game overlay backend.
///
/// Windows builds are expected to provide a Direct3D 11/12 implementation in
/// the DLL layer. Non-overlay builds can use [`NoopOverlayRenderer`] and keep
/// the existing TUI-only behavior.
pub trait OverlayRenderer {
    /// Render one overlay frame produced from the shared TUI application state.
    fn render_overlay(&mut self, frame: &OverlayFrame) -> OverlayRenderResult;
}

/// Result type returned by overlay renderers.
pub type OverlayRenderResult = Result<OverlayRenderStatus, OverlayRenderError>;

/// Outcome of a renderer frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayRenderStatus {
    /// Overlay rendering is disabled by configuration.
    Disabled,
    /// A frame was accepted by the overlay backend.
    Rendered,
}

/// Renderer setup or frame submission failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayRenderError {
    /// The configured graphics backend is not available in this build.
    BackendUnavailable(OverlayGraphicsBackend),
}

/// Supported in-game graphics backends for the DLL overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OverlayGraphicsBackend {
    Direct3D11,
    Direct3D12,
}

/// Renderer used when the in-game overlay is disabled or unavailable.
#[derive(Debug, Default)]
pub struct NoopOverlayRenderer;

impl OverlayRenderer for NoopOverlayRenderer {
    fn render_overlay(&mut self, frame: &OverlayFrame) -> OverlayRenderResult {
        if frame.enabled {
            Ok(OverlayRenderStatus::Rendered)
        } else {
            Ok(OverlayRenderStatus::Disabled)
        }
    }
}

/// User-selected overlay theme behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum OverlayThemePreference {
    /// Match the active TUI theme.
    #[default]
    FollowTui,
    /// Force dark overlay colors.
    Dark,
    /// Force light overlay colors.
    Light,
}

/// Stable identifiers for the first overlay windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum OverlayWindowKind {
    SpellLoadout,
    Rotation,
    Pull,
    ForceTarget,
    Clicky,
    CampStatus,
}

impl OverlayWindowKind {
    /// Default ordering for overlay chrome and layout persistence.
    pub const ALL: [Self; 6] = [
        Self::SpellLoadout,
        Self::Rotation,
        Self::Pull,
        Self::ForceTarget,
        Self::Clicky,
        Self::CampStatus,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::SpellLoadout => "Spell Loadout",
            Self::Rotation => "Rotation",
            Self::Pull => "Pull",
            Self::ForceTarget => "Force Target",
            Self::Clicky => "Clicky",
            Self::CampStatus => "Camp Status",
        }
    }

    const fn default_rect(self) -> OverlayRect {
        match self {
            Self::SpellLoadout => OverlayRect::new(24, 24, 320, 360),
            Self::Rotation => OverlayRect::new(368, 24, 320, 280),
            Self::Pull => OverlayRect::new(712, 24, 320, 280),
            Self::ForceTarget => OverlayRect::new(24, 408, 320, 260),
            Self::Clicky => OverlayRect::new(368, 328, 320, 220),
            Self::CampStatus => OverlayRect::new(712, 328, 340, 260),
        }
    }
}

/// Pixel rectangle in the game-client backbuffer coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OverlayRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl OverlayRect {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Persistable layout for a single overlay window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OverlayWindowLayout {
    pub rect: OverlayRect,
    pub min_width: u32,
    pub min_height: u32,
    pub open: bool,
    pub minimized: bool,
}

impl OverlayWindowLayout {
    #[must_use]
    pub const fn default_for(kind: OverlayWindowKind) -> Self {
        Self {
            rect: kind.default_rect(),
            min_width: 180,
            min_height: 96,
            open: true,
            minimized: false,
        }
    }

    /// Clamp a resize operation to this window's minimum usable dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.rect.width = width.max(self.min_width);
        self.rect.height = height.max(self.min_height);
    }
}

/// Input capture flags passed to the hook layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OverlayInputCapture {
    pub mouse: bool,
    pub keyboard: bool,
    pub hotkeys: bool,
}

impl OverlayInputCapture {
    #[must_use]
    pub const fn passes_through_eq_input(self) -> bool {
        !self.mouse && !self.keyboard
    }
}

/// A renderer-neutral overlay frame.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayFrame {
    pub enabled: bool,
    pub active_character: Option<String>,
    pub theme: OverlayThemePreference,
    pub tui_theme_name: String,
    pub input_capture: OverlayInputCapture,
    pub windows: Vec<OverlayWindow>,
}

/// View model for one overlay window.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayWindow {
    pub kind: OverlayWindowKind,
    pub title: &'static str,
    pub layout: OverlayWindowLayout,
    pub widgets: Vec<OverlayWidget>,
}

/// Minimal widget vocabulary for the first overlay renderer.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayWidget {
    Text {
        id: &'static str,
        label: String,
    },
    Button {
        id: &'static str,
        label: String,
        enabled: bool,
    },
    TextBox {
        id: &'static str,
        label: String,
        value: String,
        focused: bool,
    },
    Dropdown {
        id: &'static str,
        label: String,
        selected: String,
        options: Vec<String>,
    },
    List {
        id: &'static str,
        items: Vec<OverlayListItem>,
    },
}

/// One row in an overlay list widget.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayListItem {
    pub label: String,
    pub detail: Option<String>,
    pub selected: bool,
    pub enabled: bool,
}

/// Window layout and input policy for the in-game overlay.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OverlayWindowManager {
    pub enabled: bool,
    pub theme: OverlayThemePreference,
    pub default_layouts: HashMap<OverlayWindowKind, OverlayWindowLayout>,
    pub character_layouts: HashMap<String, HashMap<OverlayWindowKind, OverlayWindowLayout>>,
    pub focused_window: Option<OverlayWindowKind>,
    pub keyboard_focus: Option<String>,
    pub hotkeys_enabled: bool,
}

impl Default for OverlayWindowManager {
    fn default() -> Self {
        let default_layouts = OverlayWindowKind::ALL
            .into_iter()
            .map(|kind| (kind, OverlayWindowLayout::default_for(kind)))
            .collect();

        Self {
            enabled: false,
            theme: OverlayThemePreference::FollowTui,
            default_layouts,
            character_layouts: HashMap::new(),
            focused_window: None,
            keyboard_focus: None,
            hotkeys_enabled: true,
        }
    }
}

impl OverlayWindowManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a renderer-neutral frame from the same state used by the TUI.
    #[must_use]
    pub fn frame_from_app(&self, app: &App) -> OverlayFrame {
        let active_character = selected_character_name(app);
        let windows = OverlayWindowKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let layout = self.layout_for(active_character.as_deref(), kind);
                if !layout.open {
                    return None;
                }

                let widgets = if layout.minimized {
                    Vec::new()
                } else {
                    widgets_for_window(kind, app)
                };

                Some(OverlayWindow {
                    kind,
                    title: kind.title(),
                    layout,
                    widgets,
                })
            })
            .collect();

        OverlayFrame {
            enabled: self.enabled,
            active_character,
            theme: self.theme,
            tui_theme_name: format!("{:?}", app.theme_kind),
            input_capture: OverlayInputCapture {
                mouse: self.focused_window.is_some(),
                keyboard: self.keyboard_focus.is_some(),
                hotkeys: self.hotkeys_enabled && self.enabled,
            },
            windows,
        }
    }

    #[must_use]
    pub fn layout_for(
        &self,
        character: Option<&str>,
        kind: OverlayWindowKind,
    ) -> OverlayWindowLayout {
        character
            .and_then(|name| self.character_layouts.get(name))
            .and_then(|layouts| layouts.get(&kind))
            .copied()
            .or_else(|| self.default_layouts.get(&kind).copied())
            .unwrap_or_else(|| OverlayWindowLayout::default_for(kind))
    }
}

fn selected_character_name(app: &App) -> Option<String> {
    app.clients.get(app.selected_client).and_then(|client| {
        if !client.character_name.is_empty() {
            Some(client.character_name.clone())
        } else {
            client
                .local_player
                .as_ref()
                .map(|player| player.displayed_name.clone())
        }
    })
}

fn widgets_for_window(kind: OverlayWindowKind, app: &App) -> Vec<OverlayWidget> {
    match kind {
        OverlayWindowKind::SpellLoadout => spell_loadout_widgets(app),
        OverlayWindowKind::Rotation => rotation_widgets(app),
        OverlayWindowKind::Pull => pull_widgets(app),
        OverlayWindowKind::ForceTarget => force_target_widgets(app),
        OverlayWindowKind::Clicky => clicky_widgets(app),
        OverlayWindowKind::CampStatus => camp_status_widgets(app),
    }
}

fn spell_loadout_widgets(app: &App) -> Vec<OverlayWidget> {
    let items = app
        .spell_loadout_state
        .slots
        .iter()
        .enumerate()
        .map(|(index, slot)| OverlayListItem {
            label: format!("Gem {}", index + 1),
            detail: Some(format!("{} ms recast", slot.recast_ms)),
            selected: index == app.spell_loadout_state.selected,
            enabled: slot.enabled,
        })
        .collect();

    vec![OverlayWidget::List {
        id: "spell-loadout-slots",
        items,
    }]
}

fn rotation_widgets(app: &App) -> Vec<OverlayWidget> {
    let items = app
        .rotation_window_state
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| OverlayListItem {
            label: entry.name.clone(),
            detail: Some(format!("{} casts", entry.casts)),
            selected: index == app.rotation_window_state.selected,
            enabled: entry.enabled,
        })
        .collect();

    vec![OverlayWidget::List {
        id: "rotation-entries",
        items,
    }]
}

fn pull_widgets(app: &App) -> Vec<OverlayWidget> {
    let mut widgets = vec![OverlayWidget::Button {
        id: "pull-toggle",
        label: String::from("Auto Pull"),
        enabled: app.pull_window_state.auto_pull_enabled,
    }];

    let items = app
        .pull_window_state
        .targets
        .iter()
        .enumerate()
        .map(|(index, target)| OverlayListItem {
            label: target.name.clone(),
            detail: target.done.then(|| String::from("done")),
            selected: index == app.pull_window_state.selected,
            enabled: !target.done,
        })
        .collect();

    widgets.push(OverlayWidget::List {
        id: "pull-targets",
        items,
    });
    widgets
}

fn force_target_widgets(app: &App) -> Vec<OverlayWidget> {
    let items = app
        .force_target_state
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| OverlayListItem {
            label: entry.name.clone(),
            detail: Some(format!("#{}", entry.spawn_id)),
            selected: index == app.force_target_state.selected,
            enabled: true,
        })
        .collect();

    vec![
        OverlayWidget::TextBox {
            id: "force-target-filter",
            label: String::from("Target"),
            value: String::new(),
            focused: app.overlay_manager.keyboard_focus.as_deref() == Some("force-target-filter"),
        },
        OverlayWidget::List {
            id: "force-targets",
            items,
        },
    ]
}

fn clicky_widgets(app: &App) -> Vec<OverlayWidget> {
    let items = app
        .clicky_window_state
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| OverlayListItem {
            label: format!("Clicky {}", index + 1),
            detail: None,
            selected: index == app.clicky_window_state.selected,
            enabled: item.enabled,
        })
        .collect();

    vec![OverlayWidget::List {
        id: "clicky-items",
        items,
    }]
}

fn camp_status_widgets(app: &App) -> Vec<OverlayWidget> {
    let mut widgets = vec![
        OverlayWidget::Text {
            id: "camp-phase",
            label: format!("Phase: {:?}", app.camp_status_state.phase),
        },
        OverlayWidget::Text {
            id: "camp-pull-target",
            label: format!("Pull: {}", app.camp_status_state.current_pull_target),
        },
    ];

    let items = app
        .camp_status_state
        .members
        .iter()
        .map(|member| {
            let mana = member
                .mana_pct
                .map(|pct| format!("{pct:.0}% mana"))
                .unwrap_or_else(|| String::from("mana unknown"));
            OverlayListItem {
                label: member.name.clone(),
                detail: Some(format!("{:.0} hp, {mana}", member.hp_points)),
                selected: false,
                enabled: !member.is_dead,
            }
        })
        .collect();

    widgets.push(OverlayWidget::List {
        id: "camp-members",
        items,
    });
    widgets
}
