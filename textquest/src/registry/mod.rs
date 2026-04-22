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

use std::sync::{Arc, Mutex};

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

/// A single command registration entry.
struct CommandEntry {
    id: CommandId,
    /// Full command path, e.g. `/tq nav goto` or `/mymod help`.
    path: String,
    priority: Priority,
    /// Source script/plugin ID for bulk unregistration on unload.
    source_id: String,
    /// Callback invoked with the remainder of the command line after `path`.
    handler: Box<dyn Fn(&str) + Send + Sync>,
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
        Self { entries: Vec::new(), next_id: 1 }
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
        let id = CommandId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let path = path.into();
        let source_id = source_id.into();
        tracing::debug!(
            %path, %priority, %source_id, ?id,
            "command registered"
        );
        self.entries.push(CommandEntry { id, path, priority, source_id, handler });
        id
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
        // Find all entries whose path is a prefix of the command_line.
        let matching: Vec<&CommandEntry> = self
            .entries
            .iter()
            .filter(|e| {
                command_line == e.path
                    || command_line.starts_with(&format!("{} ", e.path))
            })
            .collect();

        if matching.is_empty() {
            return false;
        }

        // Pick the entry with the lowest Priority (highest precedence).
        // Stable sort: among equal priorities, earliest-registered wins.
        let best = matching
            .into_iter()
            .min_by_key(|e| e.priority)
            .expect("matching is non-empty");

        let tail = command_line[best.path.len()..].trim_start();
        tracing::debug!(
            path = %best.path,
            priority = %best.priority,
            tail,
            "command dispatched"
        );
        (best.handler)(tail);
        true
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

/// Opaque identifier for a registered script/plugin hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptHotkeyId(u64);

/// A single hotkey registration entry.
struct HotkeyEntry {
    id: ScriptHotkeyId,
    /// Human-readable combo string, e.g. `"ctrl+shift+f5"`.
    combo: String,
    priority: Priority,
    source_id: String,
    callback: Box<dyn Fn() + Send + Sync>,
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
        Self { entries: Vec::new(), next_id: 1 }
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
        let id = ScriptHotkeyId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let combo = combo.into().to_lowercase();
        let source_id = source_id.into();
        tracing::debug!(
            %combo, %priority, %source_id, ?id,
            "hotkey registered (script/plugin)"
        );
        self.entries.push(HotkeyEntry { id, combo, priority, source_id, callback });
        id
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
        let combo_lc = combo.to_lowercase();
        let matching: Vec<&HotkeyEntry> = self
            .entries
            .iter()
            .filter(|e| e.combo == combo_lc)
            .collect();

        if matching.is_empty() {
            return false;
        }

        let best = matching
            .into_iter()
            .min_by_key(|e| e.priority)
            .expect("matching is non-empty");

        tracing::debug!(
            combo = %best.combo,
            priority = %best.priority,
            "hotkey fired (script/plugin)"
        );
        (best.callback)();
        true
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
    fn command_dispatch_returns_false_for_unregistered() {
        let reg = CommandRegistry::new();
        assert!(!reg.dispatch("/unknown"));
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

    // ── ScriptHotkeyRegistry ──────────────────────────────────────────────────

    #[test]
    fn hotkey_register_and_fire() {
        let mut reg = ScriptHotkeyRegistry::new();
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        reg.register("ctrl+f5", Priority::Script, "s", Box::new(move || { f.fetch_add(1, Ordering::Relaxed); }));

        assert!(reg.fire("ctrl+f5"));
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn hotkey_fire_case_insensitive() {
        let mut reg = ScriptHotkeyRegistry::new();
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        reg.register("Ctrl+F5", Priority::Script, "s", Box::new(move || { f.fetch_add(1, Ordering::Relaxed); }));

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
    fn hotkey_priority_resolution_plugin_beats_script() {
        let mut reg = ScriptHotkeyRegistry::new();
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));

        let o1 = order.clone();
        reg.register("alt+z", Priority::Script, "s", Box::new(move || o1.lock().unwrap().push("script")));

        let o2 = order.clone();
        reg.register("alt+z", Priority::Plugin, "p", Box::new(move || o2.lock().unwrap().push("plugin")));

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

    // ── new_shared ────────────────────────────────────────────────────────────

    #[test]
    fn new_shared_returns_empty_registries() {
        let (cmd, hk) = new_shared();
        assert!(cmd.lock().unwrap().is_empty());
        assert!(hk.lock().unwrap().is_empty());
    }
}
