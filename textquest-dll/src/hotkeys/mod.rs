//! Hotkey registration and dispatch infrastructure.
//!
//! Provides a thread-safe registry for in-game hotkey bindings. Each hotkey is
//! defined by a [`KeyCombo`] (virtual key + optional modifiers) and a callback
//! executed when the combination is detected.
//!
//! # EQ input integration
//!
//! TODO(#1020): Hook into the EQ input path (CEverQuest::ProcessKeyboard or
//! CDisplay::KeyboardEvent) to feed raw key events into
//! [`HotkeyRegistry::process_key_event`]. The registry itself is fully
//! functional; only the hook installation shim is deferred until the EQ
//! keyboard function offset is confirmed.
//!
//! # Usage
//!
//! ```rust
//! use textquest_dll::hotkeys::{HotkeyRegistry, KeyCombo, VirtualKey, Modifiers};
//!
//! let mut registry = HotkeyRegistry::new();
//! let id = registry.register(
//!     KeyCombo::new(VirtualKey::F1, Modifiers::SHIFT | Modifiers::ALT),
//!     Box::new(|| tracing::info!("Shift+Alt+F1 pressed")),
//! ).expect("combo should not already be registered");
//!
//! registry.unregister(id);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

/// Global hotkey registry instance. Initialized on first access.
///
/// Access via [`global()`].
static GLOBAL_REGISTRY: OnceLock<Mutex<HotkeyRegistry>> = OnceLock::new();

/// Returns a reference to the global [`HotkeyRegistry`] mutex.
///
/// Callers must lock this before reading or mutating the registry.
pub fn global() -> &'static Mutex<HotkeyRegistry> {
    GLOBAL_REGISTRY.get_or_init(|| Mutex::new(HotkeyRegistry::new()))
}

// ── Key definitions ──────────────────────────────────────────────────────────

/// A virtual key code.
///
/// On Windows these map directly to Win32 virtual-key constants (`VK_*`).
/// On non-Windows stub builds the values are equivalent placeholders used only
/// for testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VirtualKey {
    // Function keys
    F1 = 0x70,
    F2 = 0x71,
    F3 = 0x72,
    F4 = 0x73,
    F5 = 0x74,
    F6 = 0x75,
    F7 = 0x76,
    F8 = 0x77,
    F9 = 0x78,
    F10 = 0x79,
    F11 = 0x7A,
    F12 = 0x7B,

    // Alpha keys (A–Z)
    A = b'A',
    B = b'B',
    C = b'C',
    D = b'D',
    E = b'E',
    F = b'F',
    G = b'G',
    H = b'H',
    I = b'I',
    J = b'J',
    K = b'K',
    L = b'L',
    M = b'M',
    N = b'N',
    O = b'O',
    P = b'P',
    Q = b'Q',
    R = b'R',
    S = b'S',
    T = b'T',
    U = b'U',
    V = b'V',
    W = b'W',
    X = b'X',
    Y = b'Y',
    Z = b'Z',

    // Digits (0–9)
    Num0 = b'0',
    Num1 = b'1',
    Num2 = b'2',
    Num3 = b'3',
    Num4 = b'4',
    Num5 = b'5',
    Num6 = b'6',
    Num7 = b'7',
    Num8 = b'8',
    Num9 = b'9',

    // Navigation / editing
    Insert = 0x2D,
    Delete = 0x2E,
    Home = 0x24,
    End = 0x23,
    PageUp = 0x21,
    PageDown = 0x22,
    Left = 0x25,
    Up = 0x26,
    Right = 0x27,
    Down = 0x28,

    // Miscellaneous
    Escape = 0x1B,
    Return = 0x0D,
    Space = 0x20,
    Tab = 0x09,
    Back = 0x08,
    Tilde = 0xC0,
    Minus = 0xBD,
    Equals = 0xBB,
}

impl std::fmt::Display for VirtualKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            VirtualKey::F1 => "F1",
            VirtualKey::F2 => "F2",
            VirtualKey::F3 => "F3",
            VirtualKey::F4 => "F4",
            VirtualKey::F5 => "F5",
            VirtualKey::F6 => "F6",
            VirtualKey::F7 => "F7",
            VirtualKey::F8 => "F8",
            VirtualKey::F9 => "F9",
            VirtualKey::F10 => "F10",
            VirtualKey::F11 => "F11",
            VirtualKey::F12 => "F12",
            VirtualKey::A => "A",
            VirtualKey::B => "B",
            VirtualKey::C => "C",
            VirtualKey::D => "D",
            VirtualKey::E => "E",
            VirtualKey::F => "F",
            VirtualKey::G => "G",
            VirtualKey::H => "H",
            VirtualKey::I => "I",
            VirtualKey::J => "J",
            VirtualKey::K => "K",
            VirtualKey::L => "L",
            VirtualKey::M => "M",
            VirtualKey::N => "N",
            VirtualKey::O => "O",
            VirtualKey::P => "P",
            VirtualKey::Q => "Q",
            VirtualKey::R => "R",
            VirtualKey::S => "S",
            VirtualKey::T => "T",
            VirtualKey::U => "U",
            VirtualKey::V => "V",
            VirtualKey::W => "W",
            VirtualKey::X => "X",
            VirtualKey::Y => "Y",
            VirtualKey::Z => "Z",
            VirtualKey::Num0 => "0",
            VirtualKey::Num1 => "1",
            VirtualKey::Num2 => "2",
            VirtualKey::Num3 => "3",
            VirtualKey::Num4 => "4",
            VirtualKey::Num5 => "5",
            VirtualKey::Num6 => "6",
            VirtualKey::Num7 => "7",
            VirtualKey::Num8 => "8",
            VirtualKey::Num9 => "9",
            VirtualKey::Insert => "Insert",
            VirtualKey::Delete => "Delete",
            VirtualKey::Home => "Home",
            VirtualKey::End => "End",
            VirtualKey::PageUp => "PageUp",
            VirtualKey::PageDown => "PageDown",
            VirtualKey::Left => "Left",
            VirtualKey::Up => "Up",
            VirtualKey::Right => "Right",
            VirtualKey::Down => "Down",
            VirtualKey::Escape => "Escape",
            VirtualKey::Return => "Enter",
            VirtualKey::Space => "Space",
            VirtualKey::Tab => "Tab",
            VirtualKey::Back => "Backspace",
            VirtualKey::Tilde => "~",
            VirtualKey::Minus => "-",
            VirtualKey::Equals => "=",
        };
        f.write_str(value)
    }
}

impl FromStr for VirtualKey {
    type Err = ParseKeyComboError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "f1" => Ok(VirtualKey::F1),
            "f2" => Ok(VirtualKey::F2),
            "f3" => Ok(VirtualKey::F3),
            "f4" => Ok(VirtualKey::F4),
            "f5" => Ok(VirtualKey::F5),
            "f6" => Ok(VirtualKey::F6),
            "f7" => Ok(VirtualKey::F7),
            "f8" => Ok(VirtualKey::F8),
            "f9" => Ok(VirtualKey::F9),
            "f10" => Ok(VirtualKey::F10),
            "f11" => Ok(VirtualKey::F11),
            "f12" => Ok(VirtualKey::F12),
            "a" => Ok(VirtualKey::A),
            "b" => Ok(VirtualKey::B),
            "c" => Ok(VirtualKey::C),
            "d" => Ok(VirtualKey::D),
            "e" => Ok(VirtualKey::E),
            "f" => Ok(VirtualKey::F),
            "g" => Ok(VirtualKey::G),
            "h" => Ok(VirtualKey::H),
            "i" => Ok(VirtualKey::I),
            "j" => Ok(VirtualKey::J),
            "k" => Ok(VirtualKey::K),
            "l" => Ok(VirtualKey::L),
            "m" => Ok(VirtualKey::M),
            "n" => Ok(VirtualKey::N),
            "o" => Ok(VirtualKey::O),
            "p" => Ok(VirtualKey::P),
            "q" => Ok(VirtualKey::Q),
            "r" => Ok(VirtualKey::R),
            "s" => Ok(VirtualKey::S),
            "t" => Ok(VirtualKey::T),
            "u" => Ok(VirtualKey::U),
            "v" => Ok(VirtualKey::V),
            "w" => Ok(VirtualKey::W),
            "x" => Ok(VirtualKey::X),
            "y" => Ok(VirtualKey::Y),
            "z" => Ok(VirtualKey::Z),
            "0" | "num0" => Ok(VirtualKey::Num0),
            "1" | "num1" => Ok(VirtualKey::Num1),
            "2" | "num2" => Ok(VirtualKey::Num2),
            "3" | "num3" => Ok(VirtualKey::Num3),
            "4" | "num4" => Ok(VirtualKey::Num4),
            "5" | "num5" => Ok(VirtualKey::Num5),
            "6" | "num6" => Ok(VirtualKey::Num6),
            "7" | "num7" => Ok(VirtualKey::Num7),
            "8" | "num8" => Ok(VirtualKey::Num8),
            "9" | "num9" => Ok(VirtualKey::Num9),
            "insert" | "ins" => Ok(VirtualKey::Insert),
            "delete" | "del" => Ok(VirtualKey::Delete),
            "home" => Ok(VirtualKey::Home),
            "end" => Ok(VirtualKey::End),
            "pageup" | "pgup" => Ok(VirtualKey::PageUp),
            "pagedown" | "pgdn" => Ok(VirtualKey::PageDown),
            "left" => Ok(VirtualKey::Left),
            "up" => Ok(VirtualKey::Up),
            "right" => Ok(VirtualKey::Right),
            "down" => Ok(VirtualKey::Down),
            "escape" | "esc" => Ok(VirtualKey::Escape),
            "return" | "enter" => Ok(VirtualKey::Return),
            "space" => Ok(VirtualKey::Space),
            "tab" => Ok(VirtualKey::Tab),
            "back" | "backspace" => Ok(VirtualKey::Back),
            "tilde" | "`" | "~" => Ok(VirtualKey::Tilde),
            "minus" | "-" => Ok(VirtualKey::Minus),
            "equals" | "=" => Ok(VirtualKey::Equals),
            _ => Err(ParseKeyComboError::UnknownToken(value.trim().to_string())),
        }
    }
}

bitflags::bitflags! {
    /// Keyboard modifier flags.
    ///
    /// Combinations are constructed with the `|` operator:
    /// ```rust
    /// use textquest_dll::hotkeys::Modifiers;
    /// let combo = Modifiers::SHIFT | Modifiers::ALT;
    /// ```
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u8 {
        const SHIFT = 0b001;
        const CTRL  = 0b010;
        const ALT   = 0b100;
    }
}

/// A combination of a virtual key and zero or more modifier keys.
///
/// Two [`KeyCombo`]s are equal iff both their [`VirtualKey`] and [`Modifiers`]
/// match. This is used as the registry key for conflict detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub key: VirtualKey,
    pub modifiers: Modifiers,
}

impl KeyCombo {
    /// Construct a new key combination.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use textquest_dll::hotkeys::{KeyCombo, VirtualKey, Modifiers};
    ///
    /// let combo = KeyCombo::new(VirtualKey::F5, Modifiers::CTRL | Modifiers::SHIFT);
    /// ```
    pub const fn new(key: VirtualKey, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// Construct a bare (no-modifier) combo.
    pub const fn bare(key: VirtualKey) -> Self {
        Self {
            key,
            modifiers: Modifiers::empty(),
        }
    }

    /// Returns `true` if the combination involves all three standard modifiers
    /// (Shift + Alt + Ctrl).
    pub fn is_triple_modifier(&self) -> bool {
        self.modifiers
            .contains(Modifiers::SHIFT | Modifiers::ALT | Modifiers::CTRL)
    }
}

impl std::fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.modifiers.contains(Modifiers::CTRL) {
            f.write_str("Ctrl+")?;
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            f.write_str("Shift+")?;
        }
        if self.modifiers.contains(Modifiers::ALT) {
            f.write_str("Alt+")?;
        }
        write!(f, "{}", self.key)
    }
}

impl FromStr for KeyCombo {
    type Err = ParseKeyComboError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value.is_empty() {
            return Err(ParseKeyComboError::Empty);
        }

        let mut modifiers = Modifiers::empty();
        let mut key = None;

        for raw_part in value.split('+') {
            let part = raw_part.trim();
            if part.is_empty() {
                return Err(ParseKeyComboError::UnknownToken(raw_part.to_string()));
            }

            match part.to_ascii_lowercase().as_str() {
                "shift" => modifiers |= Modifiers::SHIFT,
                "ctrl" | "control" => modifiers |= Modifiers::CTRL,
                "alt" => modifiers |= Modifiers::ALT,
                _ => {
                    let parsed_key = VirtualKey::from_str(part)?;
                    if key.replace(parsed_key).is_some() {
                        return Err(ParseKeyComboError::DuplicateKey);
                    }
                }
            }
        }

        let key = key.ok_or(ParseKeyComboError::MissingKey)?;
        Ok(KeyCombo::new(key, modifiers))
    }
}

/// Error returned when parsing an EQ-style hotkey string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseKeyComboError {
    #[error("hotkey combo cannot be empty")]
    Empty,
    #[error("hotkey combo must include one non-modifier key")]
    MissingKey,
    #[error("hotkey combo includes more than one non-modifier key")]
    DuplicateKey,
    #[error("unknown hotkey token '{0}'")]
    UnknownToken(String),
}

/// Persisted hotkey binding from config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyBinding {
    /// EQ-style combo, for example `Alt+Z` or `Shift+Ctrl+F1`.
    pub combo: String,
    /// Slash command fired when the combo is pressed.
    pub command: String,
}

impl HotkeyBinding {
    pub fn new(combo: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            combo: combo.into(),
            command: command.into(),
        }
    }

    pub fn key_combo(&self) -> Result<KeyCombo, ParseKeyComboError> {
        self.combo.parse()
    }
}

/// Hotkey config section persisted as TOML.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyConfig {
    #[serde(default)]
    pub bindings: Vec<HotkeyBinding>,
}

/// Error returned when loading or saving hotkey bindings.
#[derive(Debug, thiserror::Error)]
pub enum HotkeyConfigError {
    #[error("failed to read or write hotkey config: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse hotkey config: {0}")]
    Deserialize(#[from] toml::de::Error),
    #[error("failed to serialize hotkey config: {0}")]
    Serialize(#[from] toml::ser::Error),
}

pub fn load_bindings_from_config(
    path: impl AsRef<Path>,
) -> Result<HotkeyConfig, HotkeyConfigError> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(HotkeyConfig::default());
    }

    let contents = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}

pub fn save_bindings_to_config(
    path: impl AsRef<Path>,
    config: &HotkeyConfig,
) -> Result<(), HotkeyConfigError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }

    let contents = toml::to_string_pretty(config)?;
    std::fs::write(path, contents)?;
    Ok(())
}

// ── Registry ─────────────────────────────────────────────────────────────────

/// Opaque identifier returned by [`HotkeyRegistry::register`].
///
/// Pass to [`HotkeyRegistry::unregister`] to remove the hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HotkeyId(u32);

/// Error returned when a registration attempt conflicts with an existing entry.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("key combo {combo} is already registered (id {existing_id:?})")]
    Conflict {
        combo: KeyCombo,
        existing_id: HotkeyId,
    },
}

/// Error returned when registering a persisted hotkey command binding.
#[derive(Debug, thiserror::Error)]
pub enum CommandHotkeyError {
    #[error(transparent)]
    Parse(#[from] ParseKeyComboError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

/// A registered hotkey entry kept inside the registry.
struct HotkeyEntry {
    id: HotkeyId,
    combo: KeyCombo,
    callback: Box<dyn Fn() + Send + Sync>,
}

/// Thread-safe registry of hotkey → callback bindings.
///
/// The registry enforces that each [`KeyCombo`] is registered at most once;
/// attempting to register a duplicate returns [`RegistryError::Conflict`].
///
/// # Thread safety
///
/// The registry is not internally synchronized, so shared access or mutation
/// across threads must be protected with external synchronization such as a
/// [`Mutex`] or `RwLock`. Use the provided [`global()`] [`Mutex`] for
/// cross-thread access.
pub struct HotkeyRegistry {
    /// Map combo → entry for O(1) conflict detection.
    by_combo: HashMap<KeyCombo, HotkeyId>,
    /// Map id → entry for O(1) lookup by id.
    by_id: HashMap<HotkeyId, HotkeyEntry>,
    /// Monotonically increasing ID counter.
    next_id: u32,
}

impl HotkeyRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            by_combo: HashMap::new(),
            by_id: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a hotkey.
    ///
    /// Returns the assigned [`HotkeyId`] on success, or
    /// [`RegistryError::Conflict`] if `combo` is already taken.
    ///
    /// # Arguments
    ///
    /// * `combo` — the key combination to bind
    /// * `callback` — closure executed (on the calling thread) when the combo
    ///   is fired via [`process_key_event`][Self::process_key_event]
    pub fn register(
        &mut self,
        combo: KeyCombo,
        callback: Box<dyn Fn() + Send + Sync>,
    ) -> Result<HotkeyId, RegistryError> {
        if let Some(&existing_id) = self.by_combo.get(&combo) {
            tracing::warn!(
                combo = %combo,
                ?existing_id,
                "hotkey registration conflict — combo already registered"
            );
            return Err(RegistryError::Conflict { combo, existing_id });
        }

        let id = HotkeyId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(1); // never emit id 0

        tracing::debug!(combo = %combo, ?id, "hotkey registered");

        self.by_combo.insert(combo, id);
        self.by_id.insert(
            id,
            HotkeyEntry {
                id,
                combo,
                callback,
            },
        );

        Ok(id)
    }

    /// Register an EQ-style hotkey that dispatches a slash command.
    pub fn register_command_binding(
        &mut self,
        combo_text: &str,
        command: impl Into<String>,
    ) -> Result<HotkeyId, CommandHotkeyError> {
        let combo = combo_text.parse::<KeyCombo>()?;
        let command = command.into();
        let id = self.register(
            combo,
            Box::new(move || {
                let result = crate::commands::dispatch_global_command(&command);
                tracing::debug!(command = %command, ?result, "hotkey command dispatched");
            }),
        )?;
        Ok(id)
    }

    /// Apply persisted command bindings to this registry.
    pub fn apply_bindings(
        &mut self,
        config: &HotkeyConfig,
    ) -> Result<Vec<HotkeyId>, CommandHotkeyError> {
        let mut ids = Vec::with_capacity(config.bindings.len());
        for binding in &config.bindings {
            ids.push(self.register_command_binding(&binding.combo, binding.command.clone())?);
        }
        Ok(ids)
    }

    /// Unregister a hotkey by its [`HotkeyId`].
    ///
    /// Returns `true` if the id was present and removed, `false` otherwise.
    pub fn unregister(&mut self, id: HotkeyId) -> bool {
        if let Some(entry) = self.by_id.remove(&id) {
            self.by_combo.remove(&entry.combo);
            tracing::debug!(combo = %entry.combo, ?id, "hotkey unregistered");
            true
        } else {
            tracing::warn!(?id, "unregister called for unknown hotkey id");
            false
        }
    }

    /// Returns `true` if `combo` is currently bound.
    pub fn is_registered(&self, combo: KeyCombo) -> bool {
        self.by_combo.contains_key(&combo)
    }

    /// Returns the [`HotkeyId`] for `combo` if it is registered.
    pub fn id_for(&self, combo: KeyCombo) -> Option<HotkeyId> {
        self.by_combo.get(&combo).copied()
    }

    /// Returns the number of registered hotkeys.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Returns `true` if no hotkeys are registered.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Process a raw key event.
    ///
    /// Looks up `combo` in the registry and, if found, invokes the associated
    /// callback synchronously on the calling thread.
    ///
    /// This is the integration point for the EQ keyboard hook. Currently called
    /// only from unit tests; the real EQ hook integration is deferred pending
    /// offset confirmation (see module-level TODO).
    ///
    /// Returns `true` if a hotkey was fired, `false` otherwise.
    pub fn process_key_event(&self, combo: KeyCombo) -> bool {
        if let Some(&id) = self.by_combo.get(&combo) {
            if let Some(entry) = self.by_id.get(&id) {
                tracing::debug!(combo = %combo, ?id, "hotkey fired");
                (entry.callback)();
                return true;
            }
        }
        false
    }

    /// Iterate over all registered combos (order unspecified).
    pub fn combos(&self) -> impl Iterator<Item = KeyCombo> + '_ {
        self.by_combo.keys().copied()
    }
}

/// Register a command hotkey in the global hotkey registry.
pub fn register_command_hotkey(
    combo_text: &str,
    command: impl Into<String>,
) -> Result<HotkeyId, CommandHotkeyError> {
    let mut registry = global().lock().expect("global hotkey registry poisoned");
    registry.register_command_binding(combo_text, command)
}

impl Default for HotkeyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    };

    fn make_counter() -> (Arc<AtomicU32>, Box<dyn Fn() + Send + Sync>) {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let cb: Box<dyn Fn() + Send + Sync> = Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        });
        (counter, cb)
    }

    // ── Registration ──────────────────────────────────────────────────────────

    #[test]
    fn register_returns_unique_ids() {
        let mut reg = HotkeyRegistry::new();
        let combo1 = KeyCombo::new(VirtualKey::F1, Modifiers::empty());
        let combo2 = KeyCombo::new(VirtualKey::F2, Modifiers::empty());

        let id1 = reg.register(combo1, Box::new(|| {})).unwrap();
        let id2 = reg.register(combo2, Box::new(|| {})).unwrap();

        assert_ne!(id1, id2, "each registration should yield a distinct id");
    }

    #[test]
    fn register_updates_len() {
        let mut reg = HotkeyRegistry::new();
        assert_eq!(reg.len(), 0);
        assert!(reg.is_empty());

        reg.register(
            KeyCombo::new(VirtualKey::F1, Modifiers::empty()),
            Box::new(|| {}),
        )
        .unwrap();
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
    }

    #[test]
    fn register_marks_combo_as_occupied() {
        let mut reg = HotkeyRegistry::new();
        let combo = KeyCombo::new(VirtualKey::A, Modifiers::CTRL);

        assert!(!reg.is_registered(combo));
        reg.register(combo, Box::new(|| {})).unwrap();
        assert!(reg.is_registered(combo));
    }

    // ── Conflict detection ────────────────────────────────────────────────────

    #[test]
    fn duplicate_registration_returns_conflict_error() {
        let mut reg = HotkeyRegistry::new();
        let combo = KeyCombo::new(VirtualKey::F5, Modifiers::SHIFT);

        let id = reg.register(combo, Box::new(|| {})).unwrap();
        let err = reg
            .register(combo, Box::new(|| {}))
            .expect_err("duplicate combo must fail");

        match err {
            RegistryError::Conflict { existing_id, .. } => {
                assert_eq!(existing_id, id, "conflict must reference the first id");
            }
        }
    }

    #[test]
    fn different_modifiers_do_not_conflict() {
        let mut reg = HotkeyRegistry::new();
        let a = KeyCombo::new(VirtualKey::F1, Modifiers::empty());
        let b = KeyCombo::new(VirtualKey::F1, Modifiers::SHIFT);
        let c = KeyCombo::new(VirtualKey::F1, Modifiers::CTRL);
        let d = KeyCombo::new(VirtualKey::F1, Modifiers::ALT);
        let e = KeyCombo::new(VirtualKey::F1, Modifiers::SHIFT | Modifiers::ALT);
        let f = KeyCombo::new(VirtualKey::F1, Modifiers::SHIFT | Modifiers::CTRL);
        let g = KeyCombo::new(
            VirtualKey::F1,
            Modifiers::SHIFT | Modifiers::ALT | Modifiers::CTRL,
        );

        for combo in [a, b, c, d, e, f, g] {
            reg.register(combo, Box::new(|| {}))
                .unwrap_or_else(|_| panic!("combo {combo} should register cleanly"));
        }
        assert_eq!(reg.len(), 7);
    }

    // ── Unregistration ────────────────────────────────────────────────────────

    #[test]
    fn unregister_removes_combo() {
        let mut reg = HotkeyRegistry::new();
        let combo = KeyCombo::new(VirtualKey::F3, Modifiers::empty());

        let id = reg.register(combo, Box::new(|| {})).unwrap();
        assert!(reg.unregister(id));
        assert!(!reg.is_registered(combo));
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn unregister_allows_re_registration() {
        let mut reg = HotkeyRegistry::new();
        let combo = KeyCombo::new(VirtualKey::B, Modifiers::ALT);

        let id1 = reg.register(combo, Box::new(|| {})).unwrap();
        reg.unregister(id1);

        // Should succeed — the slot is free again.
        let id2 = reg
            .register(combo, Box::new(|| {}))
            .expect("re-registration after unregister must succeed");

        assert_ne!(id1, id2, "re-registration should produce a fresh id");
    }

    #[test]
    fn unregister_unknown_id_returns_false() {
        let mut reg = HotkeyRegistry::new();
        assert!(!reg.unregister(HotkeyId(9999)));
    }

    // ── Callback execution ────────────────────────────────────────────────────

    #[test]
    fn process_key_event_fires_callback() {
        let mut reg = HotkeyRegistry::new();
        let combo = KeyCombo::new(VirtualKey::F10, Modifiers::CTRL);
        let (counter, cb) = make_counter();

        reg.register(combo, cb).unwrap();
        assert!(reg.process_key_event(combo));
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "callback must fire once"
        );
    }

    #[test]
    fn process_key_event_fires_callback_multiple_times() {
        let mut reg = HotkeyRegistry::new();
        let combo = KeyCombo::bare(VirtualKey::F12);
        let (counter, cb) = make_counter();

        reg.register(combo, cb).unwrap();
        for _ in 0..5 {
            assert!(reg.process_key_event(combo));
        }
        assert_eq!(counter.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn process_key_event_returns_false_for_unregistered_combo() {
        let reg = HotkeyRegistry::new();
        let combo = KeyCombo::bare(VirtualKey::Escape);
        assert!(!reg.process_key_event(combo));
    }

    #[test]
    fn process_key_event_does_not_fire_after_unregister() {
        let mut reg = HotkeyRegistry::new();
        let combo = KeyCombo::new(VirtualKey::Z, Modifiers::SHIFT | Modifiers::CTRL);
        let (counter, cb) = make_counter();

        let id = reg.register(combo, cb).unwrap();
        assert!(reg.process_key_event(combo));
        reg.unregister(id);
        assert!(!reg.process_key_event(combo));
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "no extra fires after unregister"
        );
    }

    // ── Triple-modifier combo ─────────────────────────────────────────────────

    #[test]
    fn triple_modifier_combo_detected() {
        let combo = KeyCombo::new(
            VirtualKey::F1,
            Modifiers::SHIFT | Modifiers::ALT | Modifiers::CTRL,
        );
        assert!(combo.is_triple_modifier());
    }

    #[test]
    fn partial_modifier_not_triple() {
        let combos = [
            KeyCombo::new(VirtualKey::F1, Modifiers::SHIFT | Modifiers::ALT),
            KeyCombo::new(VirtualKey::F1, Modifiers::CTRL),
            KeyCombo::new(VirtualKey::F1, Modifiers::empty()),
        ];
        for combo in combos {
            assert!(!combo.is_triple_modifier(), "{combo} should not be triple");
        }
    }

    #[test]
    fn triple_modifier_combo_registers_and_fires() {
        let mut reg = HotkeyRegistry::new();
        let combo = KeyCombo::new(
            VirtualKey::Home,
            Modifiers::SHIFT | Modifiers::ALT | Modifiers::CTRL,
        );
        let (counter, cb) = make_counter();

        reg.register(combo, cb).unwrap();
        assert!(reg.process_key_event(combo));
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    // ── Global registry ───────────────────────────────────────────────────────

    #[test]
    fn global_registry_is_accessible() {
        // Just confirm we can lock it without panicking.
        let guard = global().lock().unwrap();
        let _ = guard.len();
    }

    // ── Display / helpers ─────────────────────────────────────────────────────

    #[test]
    fn key_combo_display_formats_correctly() {
        let bare = KeyCombo::bare(VirtualKey::F1);
        assert_eq!(bare.to_string(), "F1");

        let shift_f1 = KeyCombo::new(VirtualKey::F1, Modifiers::SHIFT);
        assert_eq!(shift_f1.to_string(), "Shift+F1");

        let ctrl_alt_del = KeyCombo::new(VirtualKey::Delete, Modifiers::CTRL | Modifiers::ALT);
        assert_eq!(ctrl_alt_del.to_string(), "Ctrl+Alt+Delete");

        let all = KeyCombo::new(
            VirtualKey::F10,
            Modifiers::SHIFT | Modifiers::ALT | Modifiers::CTRL,
        );
        assert_eq!(all.to_string(), "Ctrl+Shift+Alt+F10");
    }

    #[test]
    fn parses_eq_style_hotkey_combo() {
        let combo: KeyCombo = "Shift+Alt+Ctrl+Z".parse().unwrap();
        assert_eq!(
            combo,
            KeyCombo::new(
                VirtualKey::Z,
                Modifiers::SHIFT | Modifiers::ALT | Modifiers::CTRL,
            )
        );
    }

    #[test]
    fn hotkey_config_round_trips_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hotkeys.toml");
        let config = HotkeyConfig {
            bindings: vec![HotkeyBinding::new("Alt+Z", "/mercs pull npc_name")],
        };

        save_bindings_to_config(&path, &config).unwrap();
        assert_eq!(load_bindings_from_config(&path).unwrap(), config);
    }

    #[test]
    fn command_binding_dispatches_registered_command() {
        let fired = Arc::new(AtomicU32::new(0));
        let fired_for_command = fired.clone();
        let path = "/issue793_hotkey_probe";
        let _ = crate::commands::register_script_command(path, move |_| {
            fired_for_command.fetch_add(1, Ordering::Relaxed);
            crate::commands::CommandResult::Ok
        });

        let combo: KeyCombo = "Alt+Z".parse().unwrap();
        let mut reg = HotkeyRegistry::new();
        reg.register_command_binding("Alt+Z", path).unwrap();

        assert!(reg.process_key_event(combo));
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn id_for_returns_none_when_not_registered() {
        let reg = HotkeyRegistry::new();
        assert!(reg.id_for(KeyCombo::bare(VirtualKey::A)).is_none());
    }

    #[test]
    fn id_for_returns_correct_id() {
        let mut reg = HotkeyRegistry::new();
        let combo = KeyCombo::new(VirtualKey::M, Modifiers::ALT);
        let id = reg.register(combo, Box::new(|| {})).unwrap();
        assert_eq!(reg.id_for(combo), Some(id));
    }

    #[test]
    fn combos_iterator_covers_all_registered() {
        let mut reg = HotkeyRegistry::new();
        let c1 = KeyCombo::bare(VirtualKey::F1);
        let c2 = KeyCombo::bare(VirtualKey::F2);
        let c3 = KeyCombo::bare(VirtualKey::F3);

        reg.register(c1, Box::new(|| {})).unwrap();
        reg.register(c2, Box::new(|| {})).unwrap();
        reg.register(c3, Box::new(|| {})).unwrap();

        let mut seen: Vec<KeyCombo> = reg.combos().collect();
        seen.sort_by_key(|c| c.key as u8);

        assert_eq!(seen, vec![c1, c2, c3]);
    }
}
