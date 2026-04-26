//! Shared hotkey and command registries for the TextQuest orchestrator process.
//!
//! These registries are used by Lua scripts and MQ2-compatible plugins loaded
//! by the orchestrator (not the DLL). They enforce a priority ordering:
//!
//! ```text
//! Priority::BuiltIn  (lowest numeric, highest precedence) → built-in orchestrator commands
//! Priority::Plugin   → MQ2 plugin registrations
//! Priority::Script   → Lua script registrations (lowest precedence)
//! ```
//!
//! When multiple registrations share the same path/combo, the one with the
//! highest precedence (lowest `Priority` value) wins. Scripts/plugins may
//! **override** built-in commands only at `Priority::Script` / `Priority::Plugin`
//! respectively; the built-in stays in the registry but is shadowed.
//!
//! # Unregistration on unload
//!
//! Each entry carries a `source_id` string (script ID or plugin name) so that
//! [`CommandRegistry::unregister_by_source`] and
//! [`HotkeyRegistry::unregister_by_source`] can bulk-remove all bindings
//! belonging to a script or plugin when it unloads.

use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, Mutex},
};

// -- Errors ------------------------------------------------------------------

/// Errors returned by checked command and hotkey registration APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// A slash command path was empty.
    EmptyCommandPath,
    /// Slash command paths must start with `/`.
    InvalidCommandPath(String),
    /// A hotkey combo was empty.
    EmptyHotkey,
    /// A hotkey combo did not contain a primary key.
    MissingHotkeyKey,
    /// A hotkey combo contained more than one primary key.
    MultipleHotkeyKeys(String),
    /// A hotkey combo contained the same modifier more than once.
    DuplicateModifier(String),
    /// A hotkey combo contained an unsupported modifier token.
    UnsupportedModifier(String),
    /// A command or hotkey already exists in the same character scope.
    Conflict {
        kind: &'static str,
        value: String,
        character: Option<String>,
        source_id: String,
    },
    /// The invocation did not provide a required argument.
    MissingRequiredArgument(String),
    /// A quoted argument was not closed.
    UnclosedQuote,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCommandPath => write!(f, "command path is empty"),
            Self::InvalidCommandPath(path) => {
                write!(f, "command path must start with '/': {path}")
            }
            Self::EmptyHotkey => write!(f, "hotkey combo is empty"),
            Self::MissingHotkeyKey => write!(f, "hotkey combo is missing a key"),
            Self::MultipleHotkeyKeys(combo) => {
                write!(f, "hotkey combo has multiple keys: {combo}")
            }
            Self::DuplicateModifier(modifier) => {
                write!(f, "hotkey combo repeats modifier: {modifier}")
            }
            Self::UnsupportedModifier(modifier) => {
                write!(f, "unsupported hotkey modifier: {modifier}")
            }
            Self::Conflict {
                kind,
                value,
                character,
                source_id,
            } => write!(
                f,
                "{kind} '{value}' conflicts with existing registration from '{source_id}'{}",
                character
                    .as_ref()
                    .map(|name| format!(" for character '{name}'"))
                    .unwrap_or_default()
            ),
            Self::MissingRequiredArgument(name) => write!(f, "missing required argument: {name}"),
            Self::UnclosedQuote => write!(f, "quoted argument is missing a closing quote"),
        }
    }
}

impl std::error::Error for RegistryError {}

// -- TOML persistence --------------------------------------------------------

/// Persisted registry configuration loaded from the main TOML config.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RegistryConfig {
    /// Persisted slash command metadata.
    pub commands: Vec<CommandBindingConfig>,
    /// Persisted hotkey-to-command bindings.
    pub hotkeys: Vec<HotkeyBindingConfig>,
}

impl RegistryConfig {
    /// Load registry configuration from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, parsed, or validated.
    pub fn load_toml(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Persist registry configuration as TOML.
    ///
    /// # Errors
    ///
    /// Returns an error if validation, serialization, or writing fails.
    pub fn save_toml(&self, path: &Path) -> anyhow::Result<()> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate duplicate command paths, duplicate hotkeys, and hotkey syntax.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] when any entry is invalid.
    pub fn validate(&self) -> Result<(), RegistryError> {
        let mut commands = HashSet::new();
        for command in &self.commands {
            let path = normalize_command_path(&command.path)?;
            let character = normalize_character(command.character.as_deref());
            if !commands.insert((path.clone(), character.clone())) {
                return Err(RegistryError::Conflict {
                    kind: "command",
                    value: path,
                    character,
                    source_id: command.source_id.clone(),
                });
            }
        }

        let mut hotkeys = HashSet::new();
        for hotkey in &self.hotkeys {
            let combo = HotkeyCombo::parse(&hotkey.combo)?.canonical();
            let character = normalize_character(hotkey.character.as_deref());
            if !hotkeys.insert((combo.clone(), character.clone())) {
                return Err(RegistryError::Conflict {
                    kind: "hotkey",
                    value: combo,
                    character,
                    source_id: hotkey.source_id.clone(),
                });
            }
        }

        Ok(())
    }
}

/// Persisted slash command metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommandBindingConfig {
    /// Slash command path, such as `/mercs pull`.
    pub path: String,
    /// Operator-facing help text.
    #[serde(default)]
    pub help: String,
    /// Whether the command should be available.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional character scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    /// Source script/plugin/built-in owner.
    #[serde(default)]
    pub source_id: String,
    /// Argument metadata used for validation and help output.
    #[serde(default)]
    pub arguments: Vec<CommandArgumentSpec>,
}

/// Persisted hotkey binding metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HotkeyBindingConfig {
    /// Hotkey combo, such as `Alt+Z`.
    pub combo: String,
    /// Slash command routed when the hotkey fires.
    pub command: String,
    /// Whether the binding should be available.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional character scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    /// Source script/plugin/built-in owner.
    #[serde(default)]
    pub source_id: String,
}

fn default_true() -> bool {
    true
}

fn normalize_character(character: Option<&str>) -> Option<String> {
    character
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn normalize_command_path(path: &str) -> Result<String, RegistryError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(RegistryError::EmptyCommandPath);
    }
    if !trimmed.starts_with('/') {
        return Err(RegistryError::InvalidCommandPath(trimmed.to_string()));
    }
    Ok(trimmed
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase())
}

// ── Priority ──────────────────────────────────────────────────────────────────

/// Execution priority for registered commands and hotkeys.
///
/// Lower numeric value = higher precedence (built-in wins over plugin, plugin
/// over script).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Orchestrator built-in commands. Scripts and plugins may shadow these at
    /// their own priority level, but cannot replace them.
    BuiltIn = 0,
    /// MQ2 plugin registrations.
    Plugin = 1,
    /// Lua script registrations.
    Script = 2,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::BuiltIn => write!(f, "built-in"),
            Priority::Plugin => write!(f, "plugin"),
            Priority::Script => write!(f, "script"),
        }
    }
}

// ── CommandRegistry ──────────────────────────────────────────────────────────

/// Opaque identifier for a registered command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(u64);

impl CommandId {
    /// Recreate an opaque command ID previously returned across the Lua API.
    #[must_use]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Expose the registry ID for script-facing handles.
    #[must_use]
    pub fn as_raw(self) -> u64 {
        self.0
    }
}

/// A single command registration entry.
struct CommandEntry {
    id: CommandId,
    /// Full command path, e.g. `/tq nav goto` or `/mymod help`.
    path: String,
    priority: Priority,
    /// Source script/plugin ID for bulk unregistration on unload.
    source_id: String,
    /// Optional character scope for per-character command registration.
    character: Option<String>,
    /// Operator-facing help text.
    help: Option<String>,
    /// Argument metadata used for validation.
    arguments: Vec<CommandArgumentSpec>,
    /// Disabled commands remain registered but do not dispatch.
    enabled: bool,
    /// Callback invoked with the remainder of the command line after `path`.
    handler: Box<dyn Fn(&str) + Send + Sync>,
}

/// Argument metadata for slash command validation and help output.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommandArgumentSpec {
    /// Argument name shown in help and validation errors.
    pub name: String,
    /// Whether this argument must be present.
    #[serde(default = "default_true")]
    pub required: bool,
}

/// Parsed command invocation arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    /// Original argument tail after the matched command path.
    pub raw: String,
    /// Whitespace-delimited arguments. Double-quoted segments are preserved.
    pub args: Vec<String>,
}

impl CommandInvocation {
    /// Parse a command argument tail into an invocation.
    ///
    /// # Errors
    ///
    /// Returns an error if quotes are not balanced.
    pub fn parse(raw: &str) -> Result<Self, RegistryError> {
        let args = parse_args(raw)?;
        Ok(Self {
            raw: raw.trim().to_string(),
            args,
        })
    }
}

/// Options for checked slash command registration.
#[derive(Debug, Clone)]
pub struct CommandOptions {
    /// Optional character scope for per-character registration.
    pub character: Option<String>,
    /// Operator-facing help text.
    pub help: Option<String>,
    /// Argument metadata used for validation and help.
    pub arguments: Vec<CommandArgumentSpec>,
    /// Whether the command should dispatch.
    pub enabled: bool,
    /// Allow legacy priority shadowing for one path.
    pub allow_shadowing: bool,
}

impl Default for CommandOptions {
    fn default() -> Self {
        Self {
            character: None,
            help: None,
            arguments: Vec::new(),
            enabled: true,
            allow_shadowing: false,
        }
    }
}

impl CommandOptions {
    /// Default options with the command enabled.
    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

/// Help metadata for a registered slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHelp {
    /// Normalized slash command path.
    pub path: String,
    /// Operator-facing help text.
    pub help: Option<String>,
    /// Argument metadata.
    pub arguments: Vec<CommandArgumentSpec>,
    /// Optional character scope.
    pub character: Option<String>,
    /// Whether the command currently dispatches.
    pub enabled: bool,
    /// Source script/plugin/built-in owner.
    pub source_id: String,
}

/// Thread-safe registry mapping slash-command paths to handlers with priority
/// ordering.
///
/// Access via [`CommandRegistry::new`] or share an [`Arc<Mutex<CommandRegistry>>`]
/// between the Lua bindings and the plugin loader.
pub struct CommandRegistry {
    entries: Vec<CommandEntry>,
    next_id: u64,
}

impl CommandRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Register a command handler.
    ///
    /// `path` is the full slash-command prefix (e.g. `"/mymod help"`).
    /// `source_id` identifies the registrant for bulk cleanup.
    ///
    /// Returns the assigned [`CommandId`].  Multiple registrations for the
    /// same `path` at different priorities are allowed — dispatch picks the
    /// highest-precedence (lowest `Priority` value) entry.
    pub fn register(
        &mut self,
        path: impl Into<String>,
        priority: Priority,
        source_id: impl Into<String>,
        handler: Box<dyn Fn(&str) + Send + Sync>,
    ) -> CommandId {
        let mut options = CommandOptions::enabled();
        options.allow_shadowing = true;
        match self.register_with_options(path, priority, source_id, options, handler) {
            Ok(id) => id,
            Err(error) => {
                tracing::warn!(%error, "legacy command registration rejected invalid input");
                CommandId(0)
            }
        }
    }

    /// Register a command with duplicate conflict detection enabled.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths or duplicate path/character scopes.
    pub fn try_register(
        &mut self,
        path: impl Into<String>,
        priority: Priority,
        source_id: impl Into<String>,
        handler: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Result<CommandId, RegistryError> {
        self.register_with_options(
            path,
            priority,
            source_id,
            CommandOptions::enabled(),
            handler,
        )
    }

    /// Register a command with metadata, help text, enablement, and character
    /// scope.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths or duplicate path/character scopes.
    pub fn register_with_options(
        &mut self,
        path: impl Into<String>,
        priority: Priority,
        source_id: impl Into<String>,
        options: CommandOptions,
        handler: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Result<CommandId, RegistryError> {
        let id = CommandId(self.next_id);
        let path = normalize_command_path(&path.into())?;
        let source_id = source_id.into();
        let character = normalize_character(options.character.as_deref());
        if !options.allow_shadowing
            && let Some(existing) = self
                .entries
                .iter()
                .find(|entry| entry.path == path && entry.character == character)
        {
            return Err(RegistryError::Conflict {
                kind: "command",
                value: path,
                character,
                source_id: existing.source_id.clone(),
            });
        }

        self.next_id = self.next_id.wrapping_add(1).max(1);
        tracing::debug!(
            %path, %priority, %source_id, ?id,
            "command registered"
        );
        self.entries.push(CommandEntry {
            id,
            path,
            priority,
            source_id,
            character,
            help: options.help,
            arguments: options.arguments,
            enabled: options.enabled,
            handler,
        });
        Ok(id)
    }

    /// Unregister a specific command by its [`CommandId`].
    ///
    /// Returns `true` if the entry was found and removed.
    pub fn unregister(&mut self, id: CommandId) -> bool {
        let len_before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        let removed = self.entries.len() < len_before;
        if removed {
            tracing::debug!(?id, "command unregistered");
        } else {
            tracing::warn!(?id, "unregister_command: id not found");
        }
        removed
    }

    /// Unregister all commands registered by `source_id`.
    ///
    /// Returns the number of entries removed.
    pub fn unregister_by_source(&mut self, source_id: &str) -> usize {
        let len_before = self.entries.len();
        self.entries.retain(|e| e.source_id != source_id);
        let removed = len_before - self.entries.len();
        tracing::debug!(%source_id, removed, "commands unregistered by source");
        removed
    }

    /// Dispatch `command_line` against the registry.
    ///
    /// `command_line` should be the full slash command including the leading
    /// `/`, e.g. `"/tq nav goto 100 200 0"`.
    ///
    /// The handler of the **highest-precedence** (lowest `Priority`) entry
    /// whose `path` is a prefix of `command_line` is invoked with the
    /// remaining tail (trimmed).
    ///
    /// Returns `true` if a handler was found and called.
    pub fn dispatch(&self, command_line: &str) -> bool {
        self.dispatch_for_character(command_line, None)
    }

    /// Dispatch a slash command for an optional character scope.
    ///
    /// Character-specific entries win over global entries, then priority
    /// determines precedence.
    pub fn dispatch_for_character(&self, command_line: &str, character: Option<&str>) -> bool {
        let character = normalize_character(character);
        let matching: Vec<(&CommandEntry, String, usize)> = self
            .entries
            .iter()
            .filter(|entry| entry.enabled)
            .filter_map(|entry| {
                let scope_rank = match (entry.character.as_deref(), character.as_deref()) {
                    (Some(entry_character), Some(requested)) if entry_character == requested => 0,
                    (None, _) => 1,
                    _ => return None,
                };
                command_tail(command_line, &entry.path).map(|tail| (entry, tail, scope_rank))
            })
            .collect();

        if matching.is_empty() {
            return false;
        }

        let (best, tail, _) = matching
            .into_iter()
            .min_by_key(|(entry, _, scope_rank)| (*scope_rank, entry.priority))
            .expect("matching is non-empty");

        let invocation = match CommandInvocation::parse(&tail) {
            Ok(invocation) => invocation,
            Err(error) => {
                tracing::warn!(%error, path = %best.path, "command argument parse failed");
                return false;
            }
        };
        if let Err(error) = validate_arguments(&best.arguments, &invocation) {
            tracing::warn!(%error, path = %best.path, "command argument validation failed");
            return false;
        }

        tracing::debug!(
            path = %best.path,
            priority = %best.priority,
            tail = %invocation.raw,
            "command dispatched"
        );
        (best.handler)(&invocation.raw);
        true
    }

    /// Return help metadata for a command path in an optional character scope.
    pub fn help_for(&self, path: &str, character: Option<&str>) -> Option<CommandHelp> {
        let path = normalize_command_path(path).ok()?;
        let character = normalize_character(character);
        self.entries
            .iter()
            .filter(|entry| entry.path == path)
            .filter_map(|entry| {
                let scope_rank = match (entry.character.as_deref(), character.as_deref()) {
                    (Some(entry_character), Some(requested)) if entry_character == requested => 0,
                    (None, _) => 1,
                    _ => return None,
                };
                Some((entry, scope_rank))
            })
            .min_by_key(|(entry, scope_rank)| (*scope_rank, entry.priority))
            .map(|(entry, _)| CommandHelp {
                path: entry.path.clone(),
                help: entry.help.clone(),
                arguments: entry.arguments.clone(),
                character: entry.character.clone(),
                enabled: entry.enabled,
                source_id: entry.source_id.clone(),
            })
    }

    /// Resolve built-in namespace help lines such as `/mercs help pull`.
    pub fn help_for_command_line(
        &self,
        command_line: &str,
        character: Option<&str>,
    ) -> Option<CommandHelp> {
        let tokens = split_command_tokens(command_line);
        if tokens.len() < 3
            || !tokens[0].starts_with('/')
            || !tokens[1].eq_ignore_ascii_case("help")
        {
            return None;
        }
        let path = std::iter::once(tokens[0].as_str())
            .chain(tokens[2..].iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        self.help_for(&path, character)
    }

    /// Enable or disable a registered command by ID.
    ///
    /// Returns `true` when the ID was found.
    pub fn set_enabled(&mut self, id: CommandId, enabled: bool) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.enabled = enabled;
            return true;
        }
        false
    }

    /// Number of registered commands.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no commands are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── HotkeyRegistry ───────────────────────────────────────────────────────────

/// Parsed hotkey combo with supported Ctrl/Alt/Shift modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeyCombo {
    /// Ctrl modifier.
    pub ctrl: bool,
    /// Alt modifier.
    pub alt: bool,
    /// Shift modifier.
    pub shift: bool,
    /// Primary key.
    pub key: String,
}

impl HotkeyCombo {
    /// Parse a user-facing hotkey string such as `Alt+Z` or `Ctrl+Shift+F5`.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] for empty combos, duplicate modifiers, or
    /// missing/ambiguous primary keys.
    pub fn parse(combo: &str) -> Result<Self, RegistryError> {
        let trimmed = combo.trim();
        if trimmed.is_empty() {
            return Err(RegistryError::EmptyHotkey);
        }

        let mut parsed = Self {
            ctrl: false,
            alt: false,
            shift: false,
            key: String::new(),
        };

        for part in trimmed.split('+') {
            let token = part.trim();
            if token.is_empty() {
                continue;
            }
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => {
                    if parsed.ctrl {
                        return Err(RegistryError::DuplicateModifier("ctrl".to_string()));
                    }
                    parsed.ctrl = true;
                }
                "alt" => {
                    if parsed.alt {
                        return Err(RegistryError::DuplicateModifier("alt".to_string()));
                    }
                    parsed.alt = true;
                }
                "shift" => {
                    if parsed.shift {
                        return Err(RegistryError::DuplicateModifier("shift".to_string()));
                    }
                    parsed.shift = true;
                }
                key if matches!(key, "meta" | "cmd" | "super" | "win") => {
                    return Err(RegistryError::UnsupportedModifier(token.to_string()));
                }
                key => {
                    if !parsed.key.is_empty() {
                        return Err(RegistryError::MultipleHotkeyKeys(trimmed.to_string()));
                    }
                    parsed.key = key.to_ascii_uppercase();
                }
            }
        }

        if parsed.key.is_empty() {
            return Err(RegistryError::MissingHotkeyKey);
        }

        Ok(parsed)
    }

    /// Render the combo in canonical `Ctrl+Alt+Shift+Key` order.
    #[must_use]
    pub fn canonical(&self) -> String {
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

impl std::str::FromStr for HotkeyCombo {
    type Err = RegistryError;

    fn from_str(combo: &str) -> Result<Self, Self::Err> {
        Self::parse(combo)
    }
}

/// Opaque identifier for a registered script/plugin hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptHotkeyId(u64);

impl ScriptHotkeyId {
    /// Recreate an opaque hotkey ID previously returned across the Lua API.
    #[must_use]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Expose the registry ID for script-facing handles.
    #[must_use]
    pub fn as_raw(self) -> u64 {
        self.0
    }
}

/// A single hotkey registration entry.
struct HotkeyEntry {
    id: ScriptHotkeyId,
    /// Human-readable combo string, e.g. `"ctrl+shift+f5"`.
    combo: String,
    priority: Priority,
    source_id: String,
    /// Optional character scope for per-character bindings.
    character: Option<String>,
    /// Disabled hotkeys remain registered but do not fire.
    enabled: bool,
    callback: Box<dyn Fn() + Send + Sync>,
}

/// Options for checked hotkey registration.
#[derive(Debug, Clone)]
pub struct HotkeyOptions {
    /// Optional character scope for per-character registration.
    pub character: Option<String>,
    /// Whether the hotkey should fire.
    pub enabled: bool,
    /// Allow legacy priority shadowing for one combo.
    pub allow_shadowing: bool,
}

impl Default for HotkeyOptions {
    fn default() -> Self {
        Self {
            character: None,
            enabled: true,
            allow_shadowing: false,
        }
    }
}

/// Thread-safe registry mapping key-combo strings to callbacks with priority
/// ordering.
///
/// Combo strings are normalised to lowercase on registration so that
/// `"Ctrl+F5"` and `"ctrl+f5"` resolve to the same entry.
pub struct ScriptHotkeyRegistry {
    entries: Vec<HotkeyEntry>,
    next_id: u64,
}

impl ScriptHotkeyRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Register a hotkey.
    ///
    /// `combo` is a human-readable key combination string (e.g. `"ctrl+f5"`).
    /// `source_id` identifies the registrant for bulk cleanup on unload.
    ///
    /// Returns the assigned [`ScriptHotkeyId`].  Multiple registrations for
    /// the same normalised combo are allowed at different priorities.
    pub fn register(
        &mut self,
        combo: impl Into<String>,
        priority: Priority,
        source_id: impl Into<String>,
        callback: Box<dyn Fn() + Send + Sync>,
    ) -> ScriptHotkeyId {
        let options = HotkeyOptions {
            allow_shadowing: true,
            ..HotkeyOptions::default()
        };
        match self.register_with_options(combo, priority, source_id, options, callback) {
            Ok(id) => id,
            Err(error) => {
                tracing::warn!(%error, "legacy hotkey registration rejected invalid input");
                ScriptHotkeyId(0)
            }
        }
    }

    /// Register a hotkey with duplicate conflict detection enabled.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid combos or duplicate combo/character scopes.
    pub fn try_register(
        &mut self,
        combo: impl Into<String>,
        priority: Priority,
        source_id: impl Into<String>,
        callback: Box<dyn Fn() + Send + Sync>,
    ) -> Result<ScriptHotkeyId, RegistryError> {
        self.register_with_options(
            combo,
            priority,
            source_id,
            HotkeyOptions::default(),
            callback,
        )
    }

    /// Register a hotkey with enablement and character scope metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid combos or duplicate combo/character scopes.
    pub fn register_with_options(
        &mut self,
        combo: impl Into<String>,
        priority: Priority,
        source_id: impl Into<String>,
        options: HotkeyOptions,
        callback: Box<dyn Fn() + Send + Sync>,
    ) -> Result<ScriptHotkeyId, RegistryError> {
        let id = ScriptHotkeyId(self.next_id);
        let combo = HotkeyCombo::parse(&combo.into())?.canonical();
        let source_id = source_id.into();
        let character = normalize_character(options.character.as_deref());
        if !options.allow_shadowing
            && let Some(existing) = self
                .entries
                .iter()
                .find(|entry| entry.combo == combo && entry.character == character)
        {
            return Err(RegistryError::Conflict {
                kind: "hotkey",
                value: combo,
                character,
                source_id: existing.source_id.clone(),
            });
        }

        self.next_id = self.next_id.wrapping_add(1).max(1);
        tracing::debug!(
            %combo, %priority, %source_id, ?id,
            "hotkey registered (script/plugin)"
        );
        self.entries.push(HotkeyEntry {
            id,
            combo,
            priority,
            source_id,
            character,
            enabled: options.enabled,
            callback,
        });
        Ok(id)
    }

    /// Unregister a specific hotkey by its [`ScriptHotkeyId`].
    ///
    /// Returns `true` if the entry was found and removed.
    pub fn unregister(&mut self, id: ScriptHotkeyId) -> bool {
        let len_before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        let removed = self.entries.len() < len_before;
        if removed {
            tracing::debug!(?id, "hotkey unregistered");
        } else {
            tracing::warn!(?id, "unregister_hotkey: id not found");
        }
        removed
    }

    /// Unregister all hotkeys registered by `source_id`.
    ///
    /// Returns the number of entries removed.
    pub fn unregister_by_source(&mut self, source_id: &str) -> usize {
        let len_before = self.entries.len();
        self.entries.retain(|e| e.source_id != source_id);
        let removed = len_before - self.entries.len();
        tracing::debug!(%source_id, removed, "hotkeys unregistered by source");
        removed
    }

    /// Fire the callback for the highest-precedence handler registered for
    /// `combo` (case-insensitive).
    ///
    /// Returns `true` if a handler was found and called.
    pub fn fire(&self, combo: &str) -> bool {
        self.fire_for_character(combo, None)
    }

    /// Fire a hotkey for an optional character scope.
    ///
    /// Character-specific bindings win over global bindings, then priority
    /// determines precedence.
    pub fn fire_for_character(&self, combo: &str, character: Option<&str>) -> bool {
        let combo = match HotkeyCombo::parse(combo) {
            Ok(combo) => combo.canonical(),
            Err(error) => {
                tracing::warn!(%error, "hotkey parse failed");
                return false;
            }
        };
        let character = normalize_character(character);
        let matching: Vec<(&HotkeyEntry, usize)> = self
            .entries
            .iter()
            .filter(|entry| entry.enabled)
            .filter_map(|entry| {
                if entry.combo != combo {
                    return None;
                }
                let scope_rank = match (entry.character.as_deref(), character.as_deref()) {
                    (Some(entry_character), Some(requested)) if entry_character == requested => 0,
                    (None, _) => 1,
                    _ => return None,
                };
                Some((entry, scope_rank))
            })
            .collect();

        if matching.is_empty() {
            return false;
        }

        let (best, _) = matching
            .into_iter()
            .min_by_key(|(entry, scope_rank)| (*scope_rank, entry.priority))
            .expect("matching is non-empty");

        tracing::debug!(
            combo = %best.combo,
            priority = %best.priority,
            "hotkey fired (script/plugin)"
        );
        (best.callback)();
        true
    }

    /// Enable or disable a registered hotkey by ID.
    ///
    /// Returns `true` when the ID was found.
    pub fn set_enabled(&mut self, id: ScriptHotkeyId, enabled: bool) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.enabled = enabled;
            return true;
        }
        false
    }

    /// Number of registered hotkeys.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no hotkeys are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ScriptHotkeyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Issue #990 API name for the hotkey registry.
pub type HotKeyRegistry = ScriptHotkeyRegistry;

fn split_command_tokens(command_line: &str) -> Vec<String> {
    command_line
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

fn command_tail(command_line: &str, path: &str) -> Option<String> {
    let command_tokens = split_command_tokens(command_line);
    let path_tokens = split_command_tokens(path);
    if command_tokens.len() < path_tokens.len() {
        return None;
    }
    let matches = path_tokens
        .iter()
        .zip(command_tokens.iter())
        .all(|(expected, actual)| expected.eq_ignore_ascii_case(actual));
    matches.then(|| command_tokens[path_tokens.len()..].join(" "))
}

fn parse_args(raw: &str) -> Result<Vec<String>, RegistryError> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape = false;

    for ch in raw.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        match ch {
            '\\' if in_quotes => escape = true,
            '"' => in_quotes = !in_quotes,
            ch if ch.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if in_quotes {
        return Err(RegistryError::UnclosedQuote);
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
}

fn validate_arguments(
    specs: &[CommandArgumentSpec],
    invocation: &CommandInvocation,
) -> Result<(), RegistryError> {
    let required_count = specs.iter().filter(|spec| spec.required).count();
    if invocation.args.len() >= required_count {
        return Ok(());
    }
    let missing = specs
        .iter()
        .filter(|spec| spec.required)
        .nth(invocation.args.len())
        .map(|spec| spec.name.clone())
        .unwrap_or_else(|| "argument".to_string());
    Err(RegistryError::MissingRequiredArgument(missing))
}

// ── Shared state alias ────────────────────────────────────────────────────────

/// A thread-safe, reference-counted command registry suitable for sharing
/// between the Lua VM, plugin loader, and other subsystems.
pub type SharedCommandRegistry = Arc<Mutex<CommandRegistry>>;

/// A thread-safe, reference-counted hotkey registry suitable for sharing
/// between the Lua VM, plugin loader, and other subsystems.
pub type SharedHotkeyRegistry = Arc<Mutex<ScriptHotkeyRegistry>>;

/// Construct a new pair of shared registries.
pub fn new_shared() -> (SharedCommandRegistry, SharedHotkeyRegistry) {
    (
        Arc::new(Mutex::new(CommandRegistry::new())),
        Arc::new(Mutex::new(ScriptHotkeyRegistry::new())),
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // ── CommandRegistry ───────────────────────────────────────────────────────

    #[test]
    fn command_register_and_dispatch() {
        let mut reg = CommandRegistry::new();
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        reg.register(
            "/test",
            Priority::Script,
            "my_script",
            Box::new(move |_| {
                f.fetch_add(1, Ordering::Relaxed);
            }),
        );

        assert!(reg.dispatch("/test"));
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn command_dispatch_passes_tail() {
        let mut reg = CommandRegistry::new();
        let received = Arc::new(Mutex::new(String::new()));
        let r = received.clone();
        reg.register(
            "/nav goto",
            Priority::Script,
            "s",
            Box::new(move |tail| {
                *r.lock().unwrap() = tail.to_string();
            }),
        );

        assert!(reg.dispatch("/nav goto 100 200 0"));
        assert_eq!(*received.lock().unwrap(), "100 200 0");
    }

    #[test]
    fn command_invocation_parses_quoted_tail_arguments() {
        let parsed = CommandInvocation::parse(r#""assist \"Rathyl\" now""#)
            .expect("quoted args should parse");
        assert_eq!(parsed.args, vec!["assist \"Rathyl\" now".to_string()]);
        assert_eq!(parsed.raw, r#""assist \"Rathyl\" now""#);
    }

    #[test]
    fn command_invocation_rejects_unclosed_quote() {
        let error =
            CommandInvocation::parse(r#""assist now"#).expect_err("unclosed quotes should fail");
        assert!(matches!(error, RegistryError::UnclosedQuote));
    }

    #[test]
    fn integration_execute_slash_command_via_dispatcher() {
        let mut reg = CommandRegistry::new();
        let called = Arc::new(Mutex::new(String::new()));
        let sink = called.clone();
        reg.register(
            "/heal",
            Priority::Script,
            "combat_script",
            Box::new(move |args| {
                *sink.lock().unwrap() = args.to_string();
            }),
        );

        assert!(reg.dispatch("/heal \"Cleric Heal\""));
        assert_eq!(*called.lock().unwrap(), "Cleric Heal");
    }

    #[test]
    fn command_dispatch_returns_false_for_unregistered() {
        let reg = CommandRegistry::new();
        assert!(!reg.dispatch("/unknown"));
    }

    #[test]
    fn legacy_command_register_rejects_invalid_input_without_panicking() {
        let mut reg = CommandRegistry::new();
        let id = reg.register("invalid", Priority::Script, "my_script", Box::new(|_| {}));

        assert_eq!(id.as_raw(), 0);
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn command_priority_resolution_builtin_wins() {
        let mut reg = CommandRegistry::new();
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));

        let o1 = order.clone();
        reg.register(
            "/cmd",
            Priority::Script,
            "script",
            Box::new(move |_| o1.lock().unwrap().push("script")),
        );

        let o2 = order.clone();
        reg.register(
            "/cmd",
            Priority::BuiltIn,
            "builtin",
            Box::new(move |_| o2.lock().unwrap().push("builtin")),
        );

        let o3 = order.clone();
        reg.register(
            "/cmd",
            Priority::Plugin,
            "plugin",
            Box::new(move |_| o3.lock().unwrap().push("plugin")),
        );

        assert!(reg.dispatch("/cmd"));
        let result = order.lock().unwrap().clone();
        // Only the built-in handler should have fired.
        assert_eq!(result, vec!["builtin"]);
    }

    #[test]
    fn command_unregister_by_id() {
        let mut reg = CommandRegistry::new();
        let id = reg.register("/x", Priority::Script, "s", Box::new(|_| {}));
        assert!(reg.dispatch("/x"));
        assert!(reg.unregister(id));
        assert!(!reg.dispatch("/x"));
    }

    #[test]
    fn command_unregister_by_source() {
        let mut reg = CommandRegistry::new();
        reg.register("/a", Priority::Script, "my_script", Box::new(|_| {}));
        reg.register("/b", Priority::Script, "my_script", Box::new(|_| {}));
        reg.register("/c", Priority::Plugin, "other", Box::new(|_| {}));
        assert_eq!(reg.len(), 3);

        let removed = reg.unregister_by_source("my_script");
        assert_eq!(removed, 2);
        assert_eq!(reg.len(), 1); // "other" remains
        assert!(!reg.dispatch("/a"));
        assert!(!reg.dispatch("/b"));
        assert!(reg.dispatch("/c"));
    }

    #[test]
    fn command_checked_registration_rejects_duplicate_path() {
        let mut reg = CommandRegistry::new();
        assert!(
            reg.try_register(
                "/mercs pull",
                Priority::Script,
                "script_a",
                Box::new(|_| {})
            )
            .is_ok()
        );

        let error = reg
            .try_register(
                "/mercs pull",
                Priority::Script,
                "script_b",
                Box::new(|_| {}),
            )
            .expect_err("duplicate command path should fail");

        assert!(matches!(
            error,
            RegistryError::Conflict {
                kind: "command",
                ..
            }
        ));
    }

    #[test]
    fn command_help_and_required_args_are_enforced() {
        let mut reg = CommandRegistry::new();
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let options = CommandOptions {
            help: Some("Pull a named target".to_string()),
            arguments: vec![CommandArgumentSpec {
                name: "target".to_string(),
                required: true,
            }],
            ..CommandOptions::default()
        };
        reg.register_with_options(
            "/mercs pull",
            Priority::Script,
            "script",
            options,
            Box::new(move |_| {
                f.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();

        assert!(!reg.dispatch("/mercs pull"));
        assert!(reg.dispatch("/mercs pull orc_centurion"));
        assert_eq!(fired.load(Ordering::Relaxed), 1);
        assert_eq!(
            reg.help_for_command_line("/mercs help pull", None)
                .unwrap()
                .help,
            Some("Pull a named target".to_string())
        );
    }

    #[test]
    fn command_disable_prevents_dispatch() {
        let mut reg = CommandRegistry::new();
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let id = reg
            .try_register(
                "/x",
                Priority::Script,
                "script",
                Box::new(move |_| {
                    f.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap();

        assert!(reg.set_enabled(id, false));
        assert!(!reg.dispatch("/x"));
        assert_eq!(fired.load(Ordering::Relaxed), 0);
    }

    // ── ScriptHotkeyRegistry ──────────────────────────────────────────────────

    #[test]
    fn hotkey_register_and_fire() {
        let mut reg = ScriptHotkeyRegistry::new();
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        reg.register(
            "ctrl+f5",
            Priority::Script,
            "s",
            Box::new(move || {
                f.fetch_add(1, Ordering::Relaxed);
            }),
        );

        assert!(reg.fire("ctrl+f5"));
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn integration_register_hotkey_and_press_key() {
        let mut reg = ScriptHotkeyRegistry::new();
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        reg.register(
            "Shift+Ctrl+f12",
            Priority::Script,
            "s",
            Box::new(move || {
                f.fetch_add(1, Ordering::Relaxed);
            }),
        );

        assert!(reg.fire("shift+ctrl+F12"));
        assert_eq!(fired.load(Ordering::Relaxed), 1);
        assert!(!reg.fire("shift+ctrl+F11"));
    }

    #[test]
    fn hotkey_fire_case_insensitive() {
        let mut reg = ScriptHotkeyRegistry::new();
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        reg.register(
            "Ctrl+F5",
            Priority::Script,
            "s",
            Box::new(move || {
                f.fetch_add(1, Ordering::Relaxed);
            }),
        );

        assert!(reg.fire("ctrl+f5"));
        assert!(reg.fire("CTRL+F5"));
        assert_eq!(fired.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn hotkey_fire_returns_false_for_unregistered() {
        let reg = ScriptHotkeyRegistry::new();
        assert!(!reg.fire("ctrl+f1"));
    }

    #[test]
    fn legacy_hotkey_register_rejects_invalid_input_without_panicking() {
        let mut reg = ScriptHotkeyRegistry::new();
        let id = reg.register("", Priority::Script, "my_script", Box::new(|| {}));

        assert_eq!(id.as_raw(), 0);
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn hotkey_priority_resolution_plugin_beats_script() {
        let mut reg = ScriptHotkeyRegistry::new();
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));

        let o1 = order.clone();
        reg.register(
            "alt+z",
            Priority::Script,
            "s",
            Box::new(move || o1.lock().unwrap().push("script")),
        );

        let o2 = order.clone();
        reg.register(
            "alt+z",
            Priority::Plugin,
            "p",
            Box::new(move || o2.lock().unwrap().push("plugin")),
        );

        assert!(reg.fire("alt+z"));
        let result = order.lock().unwrap().clone();
        assert_eq!(result, vec!["plugin"]);
    }

    #[test]
    fn hotkey_unregister_by_id() {
        let mut reg = ScriptHotkeyRegistry::new();
        let id = reg.register("f1", Priority::Script, "s", Box::new(|| {}));
        assert!(reg.fire("f1"));
        assert!(reg.unregister(id));
        assert!(!reg.fire("f1"));
    }

    #[test]
    fn hotkey_unregister_by_source() {
        let mut reg = ScriptHotkeyRegistry::new();
        reg.register("f1", Priority::Script, "my_script", Box::new(|| {}));
        reg.register("f2", Priority::Script, "my_script", Box::new(|| {}));
        reg.register("f3", Priority::Plugin, "plugin_a", Box::new(|| {}));
        assert_eq!(reg.len(), 3);

        let removed = reg.unregister_by_source("my_script");
        assert_eq!(removed, 2);
        assert_eq!(reg.len(), 1);
        assert!(!reg.fire("f1"));
        assert!(!reg.fire("f2"));
        assert!(reg.fire("f3"));
    }

    #[test]
    fn hotkey_checked_registration_rejects_duplicate_combo() {
        let mut reg = ScriptHotkeyRegistry::new();
        assert!(
            reg.try_register("Alt+Z", Priority::Script, "script_a", Box::new(|| {}))
                .is_ok()
        );

        let error = reg
            .try_register("alt+z", Priority::Script, "script_b", Box::new(|| {}))
            .expect_err("duplicate hotkey should fail");

        assert!(matches!(
            error,
            RegistryError::Conflict { kind: "hotkey", .. }
        ));
    }

    #[test]
    fn hotkey_per_character_bindings_do_not_conflict() {
        let mut reg = ScriptHotkeyRegistry::new();
        let a = Arc::new(AtomicU32::new(0));
        let b = Arc::new(AtomicU32::new(0));
        let a_hit = a.clone();
        let b_hit = b.clone();

        reg.register_with_options(
            "Ctrl+Shift+F5",
            Priority::Script,
            "script_a",
            HotkeyOptions {
                character: Some("Aerin".to_string()),
                ..HotkeyOptions::default()
            },
            Box::new(move || {
                a_hit.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();
        reg.register_with_options(
            "ctrl+shift+f5",
            Priority::Script,
            "script_b",
            HotkeyOptions {
                character: Some("Borin".to_string()),
                ..HotkeyOptions::default()
            },
            Box::new(move || {
                b_hit.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();

        assert!(reg.fire_for_character("CTRL+SHIFT+F5", Some("Aerin")));
        assert!(reg.fire_for_character("ctrl+shift+f5", Some("Borin")));
        assert_eq!(a.load(Ordering::Relaxed), 1);
        assert_eq!(b.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn registry_config_validates_hotkey_syntax_and_conflicts() {
        let config = RegistryConfig {
            hotkeys: vec![
                HotkeyBindingConfig {
                    combo: "Alt+Z".to_string(),
                    command: "/mercs pull".to_string(),
                    enabled: true,
                    character: None,
                    source_id: "script".to_string(),
                },
                HotkeyBindingConfig {
                    combo: "alt+z".to_string(),
                    command: "/mercs stop".to_string(),
                    enabled: true,
                    character: None,
                    source_id: "script".to_string(),
                },
            ],
            ..RegistryConfig::default()
        };

        assert!(matches!(
            config.validate(),
            Err(RegistryError::Conflict { kind: "hotkey", .. })
        ));
    }

    // ── new_shared ────────────────────────────────────────────────────────────

    #[test]
    fn new_shared_returns_empty_registries() {
        let (cmd, hk) = new_shared();
        assert!(cmd.lock().unwrap().is_empty());
        assert!(hk.lock().unwrap().is_empty());
    }
}
