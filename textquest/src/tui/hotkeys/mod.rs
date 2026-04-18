//! Hotkey registration, conflict detection, and keyboard event routing
//!
//! This module provides a configurable hotkey system for the TUI dashboard, supporting:
//! - Key binding registration with modifiers (shift, alt, ctrl)
//! - Conflict detection between hotkey definitions
//! - Configuration loading/saving from TOML files
//! - Per-character hotkey profiles
//! - Extensible action types (Command, ToggleMode, OpenHelp, SendAssist, Custom)

use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Represents a single key binding with modifiers
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyBinding {
    /// The key code (e.g., 'a', 'F5', etc.)
    pub key: String,
    /// Whether Shift is held
    pub shift: bool,
    /// Whether Alt is held
    pub alt: bool,
    /// Whether Ctrl is held
    pub ctrl: bool,
}

impl KeyBinding {
    /// Create a new key binding
    pub fn new(key: impl Into<String>, shift: bool, alt: bool, ctrl: bool) -> Self {
        Self {
            key: key.into(),
            shift,
            alt,
            ctrl,
        }
    }

    /// Create a simple key binding without modifiers
    pub fn simple(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            shift: false,
            alt: false,
            ctrl: false,
        }
    }

    /// Check if this binding matches a keyboard event
    pub fn matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        let key_match = self.match_key_code(code);
        if !key_match {
            return false;
        }

        let shift_match = self.shift == modifiers.contains(KeyModifiers::SHIFT);
        let alt_match = self.alt == modifiers.contains(KeyModifiers::ALT);
        let ctrl_match = self.ctrl == modifiers.contains(KeyModifiers::CONTROL);

        shift_match && alt_match && ctrl_match
    }

    /// Check if a key code matches the binding's key
    fn match_key_code(&self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char(c) => {
                self.key.to_lowercase() == c.to_lowercase().to_string()
            }
            KeyCode::F(n) => self.key == format!("F{}", n),
            KeyCode::Enter => self.key == "Enter",
            KeyCode::Tab => self.key == "Tab",
            KeyCode::Backspace => self.key == "Backspace",
            KeyCode::Esc => self.key == "Esc",
            KeyCode::Left => self.key == "Left",
            KeyCode::Right => self.key == "Right",
            KeyCode::Up => self.key == "Up",
            KeyCode::Down => self.key == "Down",
            KeyCode::Home => self.key == "Home",
            KeyCode::End => self.key == "End",
            KeyCode::PageUp => self.key == "PageUp",
            KeyCode::PageDown => self.key == "PageDown",
            KeyCode::Delete => self.key == "Delete",
            KeyCode::Insert => self.key == "Insert",
            _ => false,
        }
    }

    /// Get a human-readable string representation
    pub fn to_string_pretty(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(&self.key);
        parts.join("+")
    }
}

/// The action type for a hotkey
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value")]
pub enum HotkeyAction {
    /// Execute a command (e.g., "engage", "disengage")
    Command(String),
    /// Toggle a mode (e.g., "privacy", "help", "alert_panel")
    ToggleMode(String),
    /// Open help
    OpenHelp,
    /// Send an assist command
    SendAssist,
    /// Custom action defined externally
    Custom(String),
}

impl HotkeyAction {
    /// Get a human-readable description of the action
    pub fn description(&self) -> String {
        match self {
            HotkeyAction::Command(cmd) => format!("Command: {}", cmd),
            HotkeyAction::ToggleMode(mode) => format!("Toggle: {}", mode),
            HotkeyAction::OpenHelp => "Open help".to_string(),
            HotkeyAction::SendAssist => "Send assist".to_string(),
            HotkeyAction::Custom(desc) => format!("Custom: {}", desc),
        }
    }
}

/// A hotkey mapping from key binding to action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotkey {
    /// The key binding
    pub binding: KeyBinding,
    /// The action to perform
    pub action: HotkeyAction,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
}

impl Hotkey {
    /// Create a new hotkey
    pub fn new(binding: KeyBinding, action: HotkeyAction) -> Self {
        Self {
            binding,
            action,
            description: None,
        }
    }

    /// Create a new hotkey with a description
    pub fn with_description(
        binding: KeyBinding,
        action: HotkeyAction,
        description: impl Into<String>,
    ) -> Self {
        Self {
            binding,
            action,
            description: Some(description.into()),
        }
    }
}

/// Global hotkey configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// Global hotkeys available to all characters
    pub global: Vec<Hotkey>,
    /// Per-character hotkey profiles
    pub character_profiles: HashMap<String, Vec<Hotkey>>,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            global: Vec::new(),
            character_profiles: HashMap::new(),
        }
    }
}

/// Registry for managing hotkeys with conflict detection
pub struct HotkeyRegistry {
    config: HotkeyConfig,
    global_bindings: HashSet<String>,
    character_bindings: HashMap<String, HashSet<String>>,
}

impl HotkeyRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            config: HotkeyConfig::default(),
            global_bindings: HashSet::new(),
            character_bindings: HashMap::new(),
        }
    }

    /// Create a registry from a configuration
    pub fn from_config(config: HotkeyConfig) -> Result<Self> {
        let mut registry = Self::new();
        registry.config = config;

        // Build indices for conflict detection
        for hotkey in &registry.config.global {
            let key_str = hotkey.binding.to_string_pretty();
            registry.global_bindings.insert(key_str);
        }

        for (character, hotkeys) in &registry.config.character_profiles {
            let mut char_bindings = HashSet::new();
            for hotkey in hotkeys {
                let key_str = hotkey.binding.to_string_pretty();
                char_bindings.insert(key_str);
            }
            registry.character_bindings.insert(character.clone(), char_bindings);
        }

        Ok(registry)
    }

    /// Register a global hotkey
    pub fn register_global(&mut self, hotkey: Hotkey) -> Result<()> {
        let key_str = hotkey.binding.to_string_pretty();

        // Check for conflicts
        if self.global_bindings.contains(&key_str) {
            return Err(anyhow!(
                "Global hotkey conflict: '{}' is already registered",
                key_str
            ));
        }

        self.global_bindings.insert(key_str);
        self.config.global.push(hotkey);
        Ok(())
    }

    /// Register a character-specific hotkey
    pub fn register_character(
        &mut self,
        character: impl Into<String>,
        hotkey: Hotkey,
    ) -> Result<()> {
        let character = character.into();
        let key_str = hotkey.binding.to_string_pretty();

        // Check for conflicts with global hotkeys
        if self.global_bindings.contains(&key_str) {
            return Err(anyhow!(
                "Character hotkey '{}' conflicts with a global hotkey",
                key_str
            ));
        }

        // Check for conflicts with existing character hotkeys
        if let Some(char_bindings) = self.character_bindings.get(&character) {
            if char_bindings.contains(&key_str) {
                return Err(anyhow!(
                    "Character hotkey conflict for '{}': '{}' is already registered",
                    character,
                    key_str
                ));
            }
        }

        // Insert the hotkey
        self.config
            .character_profiles
            .entry(character.clone())
            .or_insert_with(Vec::new)
            .push(hotkey);

        self.character_bindings
            .entry(character)
            .or_insert_with(HashSet::new)
            .insert(key_str);

        Ok(())
    }

    /// Unregister a global hotkey
    pub fn unregister_global(&mut self, key: &str) -> Result<()> {
        let key_str = key.to_string();
        if !self.global_bindings.remove(&key_str) {
            return Err(anyhow!("Global hotkey '{}' not found", key));
        }

        self.config.global.retain(|h| h.binding.to_string_pretty() != key_str);
        Ok(())
    }

    /// Unregister a character-specific hotkey
    pub fn unregister_character(&mut self, character: &str, key: &str) -> Result<()> {
        let key_str = key.to_string();

        if let Some(bindings) = self.character_bindings.get_mut(character) {
            if !bindings.remove(&key_str) {
                return Err(anyhow!(
                    "Character hotkey '{}' for '{}' not found",
                    key,
                    character
                ));
            }
        } else {
            return Err(anyhow!("No hotkeys registered for character '{}'", character));
        }

        if let Some(hotkeys) = self.config.character_profiles.get_mut(character) {
            hotkeys.retain(|h| h.binding.to_string_pretty() != key_str);
        }

        Ok(())
    }

    /// Find a conflict if one exists for a new binding
    pub fn find_conflict(&self, binding: &KeyBinding) -> Option<String> {
        let key_str = binding.to_string_pretty();

        if self.global_bindings.contains(&key_str) {
            return Some(format!("Global hotkey conflict: '{}'", key_str));
        }

        for (character, bindings) in &self.character_bindings {
            if bindings.contains(&key_str) {
                return Some(format!(
                    "Character hotkey conflict for '{}': '{}'",
                    character, key_str
                ));
            }
        }

        None
    }

    /// Find the action for a keyboard event
    pub fn get_action(
        &self,
        code: KeyCode,
        modifiers: KeyModifiers,
        character: Option<&str>,
    ) -> Option<HotkeyAction> {
        // Check character-specific hotkeys first
        if let Some(char_name) = character {
            if let Some(hotkeys) = self.config.character_profiles.get(char_name) {
                for hotkey in hotkeys {
                    if hotkey.binding.matches(code, modifiers) {
                        return Some(hotkey.action.clone());
                    }
                }
            }
        }

        // Fall back to global hotkeys
        for hotkey in &self.config.global {
            if hotkey.binding.matches(code, modifiers) {
                return Some(hotkey.action.clone());
            }
        }

        None
    }

    /// Load configuration from a TOML file
    pub fn load_from_config(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(anyhow!("Config file not found: {:?}", path));
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        let config: HotkeyConfig = toml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse config file: {}", e))?;

        *self = Self::from_config(config)?;
        Ok(())
    }

    /// Save configuration to a TOML file
    pub fn save_to_config(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow!("Failed to create config directory: {}", e))?;
        }

        let content = toml::to_string_pretty(&self.config)
            .map_err(|e| anyhow!("Failed to serialize config: {}", e))?;
        std::fs::write(path, content)
            .map_err(|e| anyhow!("Failed to write config file: {}", e))?;

        Ok(())
    }

    /// Get all global hotkeys
    pub fn global_hotkeys(&self) -> &[Hotkey] {
        &self.config.global
    }

    /// Get all hotkeys for a character
    pub fn character_hotkeys(&self, character: &str) -> Option<&[Hotkey]> {
        self.config.character_profiles.get(character).map(|v| v.as_slice())
    }

    /// List all registered character profiles
    pub fn character_profiles(&self) -> Vec<String> {
        self.config.character_profiles.keys().cloned().collect()
    }
}

impl Default for HotkeyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_binding_simple() {
        let binding = KeyBinding::simple("a");
        assert_eq!(binding.key, "a");
        assert!(!binding.shift);
        assert!(!binding.alt);
        assert!(!binding.ctrl);
    }

    #[test]
    fn key_binding_with_modifiers() {
        let binding = KeyBinding::new("F5", true, true, false);
        assert_eq!(binding.key, "F5");
        assert!(binding.shift);
        assert!(binding.alt);
        assert!(!binding.ctrl);
    }

    #[test]
    fn key_binding_matches_char() {
        let binding = KeyBinding::simple("a");
        let modifiers = KeyModifiers::NONE;
        assert!(binding.matches(KeyCode::Char('a'), modifiers));
        assert!(binding.matches(KeyCode::Char('A'), modifiers));
    }

    #[test]
    fn key_binding_matches_with_shift() {
        let binding = KeyBinding::new("a", true, false, false);
        let with_shift = KeyModifiers::SHIFT;
        let without_shift = KeyModifiers::NONE;

        assert!(binding.matches(KeyCode::Char('a'), with_shift));
        assert!(!binding.matches(KeyCode::Char('a'), without_shift));
    }

    #[test]
    fn key_binding_matches_function_key() {
        let binding = KeyBinding::simple("F5");
        assert!(binding.matches(KeyCode::F(5), KeyModifiers::NONE));
        assert!(!binding.matches(KeyCode::F(4), KeyModifiers::NONE));
    }

    #[test]
    fn key_binding_to_string_pretty() {
        let binding = KeyBinding::new("a", true, true, false);
        let pretty = binding.to_string_pretty();
        assert!(pretty.contains("Ctrl"));
        assert!(pretty.contains("Alt"));
        assert!(pretty.contains("Shift"));
        assert!(pretty.contains("a"));
    }

    #[test]
    fn hotkey_action_description() {
        assert_eq!(
            HotkeyAction::Command("engage".to_string()).description(),
            "Command: engage"
        );
        assert_eq!(
            HotkeyAction::ToggleMode("help".to_string()).description(),
            "Toggle: help"
        );
        assert_eq!(HotkeyAction::OpenHelp.description(), "Open help");
    }

    #[test]
    fn registry_register_global() {
        let mut registry = HotkeyRegistry::new();
        let binding = KeyBinding::simple("a");
        let action = HotkeyAction::Command("engage".to_string());
        let hotkey = Hotkey::new(binding, action);

        registry.register_global(hotkey).unwrap();
        assert_eq!(registry.global_hotkeys().len(), 1);
    }

    #[test]
    fn registry_conflict_detection_global() {
        let mut registry = HotkeyRegistry::new();
        let binding = KeyBinding::simple("a");
        let action = HotkeyAction::Command("engage".to_string());
        let hotkey1 = Hotkey::new(binding.clone(), action.clone());
        let hotkey2 = Hotkey::new(binding, action);

        registry.register_global(hotkey1).unwrap();
        let result = registry.register_global(hotkey2);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("conflict"));
    }

    #[test]
    fn registry_conflict_detection_find() {
        let mut registry = HotkeyRegistry::new();
        let binding = KeyBinding::simple("a");
        let action = HotkeyAction::Command("engage".to_string());
        let hotkey = Hotkey::new(binding.clone(), action);

        registry.register_global(hotkey).unwrap();
        let conflict = registry.find_conflict(&binding);

        assert!(conflict.is_some());
        assert!(conflict.unwrap().contains("Global hotkey conflict"));
    }

    #[test]
    fn registry_character_specific() {
        let mut registry = HotkeyRegistry::new();
        let binding = KeyBinding::simple("b");
        let action = HotkeyAction::ToggleMode("help".to_string());
        let hotkey = Hotkey::new(binding, action);

        registry.register_character("Warrior", hotkey).unwrap();
        assert_eq!(registry.character_hotkeys("Warrior").unwrap().len(), 1);
    }

    #[test]
    fn registry_character_cannot_override_global() {
        let mut registry = HotkeyRegistry::new();

        // Register a global hotkey
        let binding = KeyBinding::simple("a");
        let action = HotkeyAction::Command("engage".to_string());
        let global_hotkey = Hotkey::new(binding.clone(), action);
        registry.register_global(global_hotkey).unwrap();

        // Try to register the same binding for a character
        let char_hotkey = Hotkey::new(binding, HotkeyAction::ToggleMode("help".to_string()));
        let result = registry.register_character("Wizard", char_hotkey);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("conflict"));
    }

    #[test]
    fn registry_get_action_global() {
        let mut registry = HotkeyRegistry::new();
        let binding = KeyBinding::simple("a");
        let action = HotkeyAction::Command("engage".to_string());
        let hotkey = Hotkey::new(binding, action.clone());

        registry.register_global(hotkey).unwrap();
        let found = registry.get_action(KeyCode::Char('a'), KeyModifiers::NONE, None);

        assert_eq!(found, Some(action));
    }

    #[test]
    fn registry_get_action_character_precedence() {
        let mut registry = HotkeyRegistry::new();

        // Register global hotkey for 'a'
        let binding = KeyBinding::simple("a");
        let global_action = HotkeyAction::Command("global_action".to_string());
        let global_hotkey = Hotkey::new(binding.clone(), global_action);
        registry.register_global(global_hotkey).unwrap();

        // Register character-specific hotkey for 'a'
        let char_action = HotkeyAction::Command("char_action".to_string());
        let char_hotkey = Hotkey::new(binding, char_action.clone());
        registry.register_character("Wizard", char_hotkey).unwrap();

        // Character-specific should take precedence
        let found = registry.get_action(KeyCode::Char('a'), KeyModifiers::NONE, Some("Wizard"));
        assert_eq!(found, Some(char_action));
    }

    #[test]
    fn registry_unregister_global() {
        let mut registry = HotkeyRegistry::new();
        let binding = KeyBinding::simple("a");
        let action = HotkeyAction::Command("engage".to_string());
        let hotkey = Hotkey::new(binding, action);

        registry.register_global(hotkey).unwrap();
        assert_eq!(registry.global_hotkeys().len(), 1);

        registry.unregister_global("Ctrl+a").ok();
        // Binding is "a", not "Ctrl+a"
        registry.unregister_global("a").unwrap();
        assert_eq!(registry.global_hotkeys().len(), 0);
    }

    #[test]
    fn registry_character_profiles_list() {
        let mut registry = HotkeyRegistry::new();
        let binding = KeyBinding::simple("a");
        let action = HotkeyAction::ToggleMode("help".to_string());

        let hotkey1 = Hotkey::new(binding.clone(), action.clone());
        let hotkey2 = Hotkey::new(binding, action);

        registry.register_character("Warrior", hotkey1).unwrap();
        registry.register_character("Wizard", hotkey2).unwrap();

        let profiles = registry.character_profiles();
        assert_eq!(profiles.len(), 2);
        assert!(profiles.contains(&"Warrior".to_string()));
        assert!(profiles.contains(&"Wizard".to_string()));
    }
}
