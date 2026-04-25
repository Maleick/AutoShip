//! Shared window management and external HUD integration contracts.
//!
//! This module is the stable data surface for MQ2HUDMove-style integrations:
//! external tools can submit desired window state, TextQuest can resolve anchor
//! positions against known displays, and layout profiles can be persisted as
//! JSON without binding the protocol to a specific renderer.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// A point in virtual desktop coordinates.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowPoint {
    pub x: i32,
    pub y: i32,
}

/// A pixel size for a window or display surface.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl Default for WindowSize {
    fn default() -> Self {
        Self {
            width: 320,
            height: 180,
        }
    }
}

/// Signed offset from an anchor point.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowOffset {
    pub x: i32,
    pub y: i32,
}

/// Resolved virtual desktop rectangle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// One monitor or display region available to TextQuest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisplaySurface {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub origin: WindowPoint,
    pub size: WindowSize,
    #[serde(default = "default_scale_factor_millis")]
    pub scale_factor_millis: u32,
    #[serde(default)]
    pub primary: bool,
}

impl DisplaySurface {
    #[must_use]
    pub fn rect(&self) -> WindowRect {
        WindowRect {
            x: self.origin.x,
            y: self.origin.y,
            width: self.size.width,
            height: self.size.height,
        }
    }
}

fn default_scale_factor_millis() -> u32 {
    1000
}

/// Anchor used when resolving a window against its target monitor.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowAnchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    LeftCenter,
    Center,
    RightCenter,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Z-order preference for coordinated HUD and tool windows.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowZOrder {
    Background,
    #[default]
    Normal,
    Floating,
    TopMost,
}

/// Focus behavior requested by an external HUD or tool.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FocusPolicy {
    Passive,
    #[default]
    Focusable,
    ClickThrough,
}

/// Display mode requested for a window.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    #[default]
    Normal,
    TransparentOverlay,
    ClickThroughOverlay,
}

/// Window category used by runtime integrations and external tools.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    GameClient,
    HudOverlay,
    Tooltip,
    Designer,
    #[default]
    ExternalTool,
}

/// Overlay opacity represented as 0..=1000 alpha permille.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverlayOpacity {
    pub permille: u16,
}

impl OverlayOpacity {
    pub const TRANSPARENT: Self = Self { permille: 0 };
    pub const OPAQUE: Self = Self { permille: 1000 };

    #[must_use]
    pub fn new(permille: u16) -> Self {
        Self {
            permille: permille.min(1000),
        }
    }

    #[must_use]
    pub fn as_alpha_f32(self) -> f32 {
        f32::from(self.permille) / 1000.0
    }
}

impl Default for OverlayOpacity {
    fn default() -> Self {
        Self::OPAQUE
    }
}

/// Desired placement and rendering behavior for one managed window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowPlacement {
    #[serde(default)]
    pub monitor_id: Option<String>,
    #[serde(default)]
    pub anchor: WindowAnchor,
    #[serde(default)]
    pub offset: WindowOffset,
    #[serde(default)]
    pub size: WindowSize,
    #[serde(default)]
    pub z_order: WindowZOrder,
    #[serde(default)]
    pub focus_policy: FocusPolicy,
    #[serde(default)]
    pub opacity: OverlayOpacity,
}

impl Default for WindowPlacement {
    fn default() -> Self {
        Self {
            monitor_id: None,
            anchor: WindowAnchor::TopLeft,
            offset: WindowOffset::default(),
            size: WindowSize::default(),
            z_order: WindowZOrder::Normal,
            focus_policy: FocusPolicy::Focusable,
            opacity: OverlayOpacity::OPAQUE,
        }
    }
}

impl WindowPlacement {
    /// Resolve this placement into virtual desktop coordinates.
    #[must_use]
    pub fn resolved_rect(&self, displays: &[DisplaySurface]) -> Option<WindowRect> {
        let display = select_display(self.monitor_id.as_deref(), displays)?;
        let origin = display.origin;
        let host_width = i64::from(display.size.width);
        let host_height = i64::from(display.size.height);
        let window_width = i64::from(self.size.width);
        let window_height = i64::from(self.size.height);

        let (anchor_x, anchor_y) = match self.anchor {
            WindowAnchor::TopLeft => (0, 0),
            WindowAnchor::TopCenter => ((host_width - window_width) / 2, 0),
            WindowAnchor::TopRight => (host_width - window_width, 0),
            WindowAnchor::LeftCenter => (0, (host_height - window_height) / 2),
            WindowAnchor::Center => (
                (host_width - window_width) / 2,
                (host_height - window_height) / 2,
            ),
            WindowAnchor::RightCenter => {
                (host_width - window_width, (host_height - window_height) / 2)
            }
            WindowAnchor::BottomLeft => (0, host_height - window_height),
            WindowAnchor::BottomCenter => {
                ((host_width - window_width) / 2, host_height - window_height)
            }
            WindowAnchor::BottomRight => (host_width - window_width, host_height - window_height),
        };

        Some(WindowRect {
            x: clamp_i32(i64::from(origin.x) + anchor_x + i64::from(self.offset.x)),
            y: clamp_i32(i64::from(origin.y) + anchor_y + i64::from(self.offset.y)),
            width: self.size.width,
            height: self.size.height,
        })
    }
}

fn select_display<'a>(
    monitor_id: Option<&str>,
    displays: &'a [DisplaySurface],
) -> Option<&'a DisplaySurface> {
    if let Some(monitor_id) = monitor_id
        && let Some(display) = displays.iter().find(|display| display.id == monitor_id)
    {
        return Some(display);
    }

    displays
        .iter()
        .find(|display| display.primary)
        .or_else(|| displays.first())
}

fn clamp_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Complete state update for one managed window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowState {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub kind: WindowKind,
    #[serde(default)]
    pub profile_id: String,
    #[serde(default)]
    pub placement: WindowPlacement,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(default)]
    pub display_mode: DisplayMode,
    #[serde(default)]
    pub external_owner: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}

impl WindowState {
    #[must_use]
    pub fn external_hud(id: impl Into<String>, profile_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: String::new(),
            kind: WindowKind::HudOverlay,
            profile_id: profile_id.into(),
            placement: WindowPlacement::default(),
            visible: true,
            display_mode: DisplayMode::TransparentOverlay,
            external_owner: None,
            theme: None,
        }
    }
}

fn default_visible() -> bool {
    true
}

/// Persisted layout for one UI profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowLayoutProfile {
    pub profile_id: String,
    #[serde(default)]
    pub displays: Vec<DisplaySurface>,
    #[serde(default)]
    pub windows: BTreeMap<String, WindowState>,
}

impl WindowLayoutProfile {
    #[must_use]
    pub fn new(profile_id: impl Into<String>) -> Self {
        Self {
            profile_id: profile_id.into(),
            displays: Vec::new(),
            windows: BTreeMap::new(),
        }
    }

    pub fn upsert_window(&mut self, mut state: WindowState) {
        if state.profile_id.is_empty() {
            state.profile_id = self.profile_id.clone();
        }
        self.windows.insert(state.id.clone(), state);
    }

    #[must_use]
    pub fn window(&self, id: &str) -> Option<&WindowState> {
        self.windows.get(id)
    }

    #[must_use]
    pub fn resolved_window_rect(&self, id: &str) -> Option<WindowRect> {
        self.window(id)
            .and_then(|state| state.placement.resolved_rect(&self.displays))
    }
}

/// Storage boundary for layout import/export.
pub trait WindowLayoutPersistence {
    fn load_profile(&self, profile_id: &str) -> Result<Option<WindowLayoutProfile>>;
    fn save_profile(&self, profile: &WindowLayoutProfile) -> Result<()>;
}

/// JSON file-backed profile store.
#[derive(Debug, Clone)]
pub struct JsonWindowLayoutStore {
    root: PathBuf,
}

impl JsonWindowLayoutStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn profile_path(&self, profile_id: &str) -> PathBuf {
        self.root
            .join(format!("{}.json", sanitize_profile_id(profile_id)))
    }
}

impl WindowLayoutPersistence for JsonWindowLayoutStore {
    fn load_profile(&self, profile_id: &str) -> Result<Option<WindowLayoutProfile>> {
        let path = self.profile_path(profile_id);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read window layout profile {}", path.display()))?;
        let profile = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse window layout profile {}", path.display()))?;
        Ok(Some(profile))
    }

    fn save_profile(&self, profile: &WindowLayoutProfile) -> Result<()> {
        fs::create_dir_all(&self.root).with_context(|| {
            format!(
                "failed to create window layout directory {}",
                self.root.display()
            )
        })?;
        let path = self.profile_path(&profile.profile_id);
        let bytes = serde_json::to_vec_pretty(profile)
            .with_context(|| format!("failed to serialize window layout {}", profile.profile_id))?;
        fs::write(&path, bytes)
            .with_context(|| format!("failed to write window layout profile {}", path.display()))
    }
}

fn sanitize_profile_id(profile_id: &str) -> String {
    let sanitized: String = profile_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "default".to_string()
    } else {
        sanitized
    }
}

/// One-method API boundary consumed by external HUD/tool adapters.
pub trait ExternalHudWindowApi {
    fn apply_window_state(&mut self, state: WindowState) -> Result<WindowControlAck>;
}

/// Acknowledgement returned after a window state update is accepted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowControlAck {
    pub profile_id: String,
    pub window_id: String,
    pub resolved_rect: Option<WindowRect>,
    pub persisted: bool,
}

/// File-backed controller suitable for SDKs, web handlers, and runtime adapters.
pub struct PersistentWindowManager<S> {
    store: S,
    profile: WindowLayoutProfile,
}

impl<S: WindowLayoutPersistence> PersistentWindowManager<S> {
    pub fn load_or_new(
        store: S,
        profile_id: impl Into<String>,
        displays: Vec<DisplaySurface>,
    ) -> Result<Self> {
        let profile_id = profile_id.into();
        let mut profile = store
            .load_profile(&profile_id)?
            .unwrap_or_else(|| WindowLayoutProfile::new(profile_id));
        if !displays.is_empty() {
            profile.displays = displays;
        }
        Ok(Self { store, profile })
    }

    #[must_use]
    pub fn profile(&self) -> &WindowLayoutProfile {
        &self.profile
    }

    #[must_use]
    pub fn into_inner(self) -> (S, WindowLayoutProfile) {
        (self.store, self.profile)
    }
}

impl<S: WindowLayoutPersistence> ExternalHudWindowApi for PersistentWindowManager<S> {
    fn apply_window_state(&mut self, mut state: WindowState) -> Result<WindowControlAck> {
        if !state.profile_id.is_empty() && state.profile_id != self.profile.profile_id {
            bail!(
                "window state profile '{}' does not match active profile '{}'",
                state.profile_id,
                self.profile.profile_id
            );
        }

        state.profile_id = self.profile.profile_id.clone();
        let window_id = state.id.clone();
        self.profile.upsert_window(state);
        let resolved_rect = self.profile.resolved_window_rect(&window_id);
        self.store.save_profile(&self.profile)?;

        Ok(WindowControlAck {
            profile_id: self.profile.profile_id.clone(),
            window_id,
            resolved_rect,
            persisted: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: &str, x: i32, y: i32, width: u32, height: u32, primary: bool) -> DisplaySurface {
        DisplaySurface {
            id: id.to_string(),
            name: None,
            origin: WindowPoint { x, y },
            size: WindowSize { width, height },
            scale_factor_millis: 1000,
            primary,
        }
    }

    #[test]
    fn anchored_rect_uses_selected_monitor() {
        let placement = WindowPlacement {
            monitor_id: Some("right".into()),
            anchor: WindowAnchor::BottomRight,
            offset: WindowOffset { x: -16, y: -24 },
            size: WindowSize {
                width: 320,
                height: 180,
            },
            z_order: WindowZOrder::TopMost,
            focus_policy: FocusPolicy::ClickThrough,
            opacity: OverlayOpacity::new(650),
        };
        let displays = vec![
            display("primary", 0, 0, 1920, 1080, true),
            display("right", 1920, 0, 2560, 1440, false),
        ];

        assert_eq!(
            placement.resolved_rect(&displays),
            Some(WindowRect {
                x: 4144,
                y: 1236,
                width: 320,
                height: 180,
            })
        );
        assert_eq!(placement.opacity.as_alpha_f32(), 0.65);
    }

    #[test]
    fn json_store_round_trips_profile() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = JsonWindowLayoutStore::new(temp.path().to_path_buf());
        let mut profile = WindowLayoutProfile::new("raid/ui");
        profile
            .displays
            .push(display("primary", 0, 0, 1920, 1080, true));
        profile.upsert_window(WindowState::external_hud("hud-main", "raid/ui"));

        store.save_profile(&profile).expect("save profile");
        let loaded = store
            .load_profile("raid/ui")
            .expect("load profile")
            .expect("profile exists");

        assert_eq!(loaded, profile);
        assert!(store.profile_path("raid/ui").ends_with("raid_ui.json"));
    }

    #[test]
    fn manager_applies_window_state_and_persists_layout() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let store = JsonWindowLayoutStore::new(root.clone());
        let displays = vec![display("primary", 10, 20, 1280, 720, true)];
        let mut manager =
            PersistentWindowManager::load_or_new(store, "default", displays).expect("manager");
        let mut state = WindowState::external_hud("tooltip", "");
        state.placement.offset = WindowOffset { x: 32, y: 40 };

        let ack = manager.apply_window_state(state).expect("apply state");

        assert_eq!(ack.window_id, "tooltip");
        assert_eq!(
            ack.resolved_rect,
            Some(WindowRect {
                x: 42,
                y: 60,
                width: 320,
                height: 180,
            })
        );
        assert!(
            JsonWindowLayoutStore::new(root)
                .load_profile("default")
                .expect("load profile")
                .expect("profile exists")
                .window("tooltip")
                .is_some()
        );
    }
}
