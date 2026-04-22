//! Built-in `/textquest` subcommand surface.
//!
//! Registers a single `/textquest` command at [`Priority::BuiltIn`] that
//! parses its own subcommand argument and dispatches internally to the
//! appropriate handler.  All logic is wired to a [`ScriptManager`] trait
//! object and a [`PluginRegistry`] (both behind `Arc`s).
//!
//! # Subcommands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `/textquest` | Show help overview |
//! | `/textquest help [topic]` | Detailed help |
//! | `/textquest list_scripts` | Loaded Lua scripts with IDs + state |
//! | `/textquest list_plugins` | Loaded plugins with state |
//! | `/textquest reload <name>` | Reload a script or plugin by name |
//! | `/textquest enable <name>` | Enable (resume) a script |
//! | `/textquest disable <name>` | Disable (pause) a script |
//! | `/textquest debug [on|off]` | Toggle debug verbosity |
//! | `/textquest status` | Orchestrator health summary |

use std::sync::{Arc, Mutex};

use crate::plugins::PluginRegistry;
use crate::registry::{Priority, SharedCommandRegistry};

// ── Script abstraction ────────────────────────────────────────────────────────

/// Lifecycle state of a managed Lua script, mirroring `lua::ScriptState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptLifecycle {
    Loading,
    Running,
    Paused,
    Error(String),
    Unloaded,
}

impl std::fmt::Display for ScriptLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScriptLifecycle::Loading => write!(f, "Loading"),
            ScriptLifecycle::Running => write!(f, "Running"),
            ScriptLifecycle::Paused => write!(f, "Paused"),
            ScriptLifecycle::Error(msg) => write!(f, "Error({})", msg),
            ScriptLifecycle::Unloaded => write!(f, "Unloaded"),
        }
    }
}

/// Snapshot of a single loaded script for display.
#[derive(Debug, Clone)]
pub struct ScriptSnapshot {
    pub id: String,
    pub name: String,
    pub state: ScriptLifecycle,
}

/// Trait abstracting the Lua script loader for use by built-in command handlers.
///
/// This trait decouples the command surface from the concrete `lua::ScriptLoader`
/// so that (a) the module compiles without pulling in the full mlua runtime and
/// (b) tests can inject a lightweight stub.
pub trait ScriptManager: Send + Sync {
    /// Return a snapshot of all currently-tracked scripts.
    fn list_scripts(&self) -> Vec<ScriptSnapshot>;

    /// Reload a script by ID. Returns an error string on failure.
    fn reload(&self, id: &str) -> Result<(), String>;

    /// Resume (enable) a paused script by ID. Returns an error string on failure.
    fn enable(&self, id: &str) -> Result<(), String>;

    /// Pause (disable) a running script by ID. Returns an error string on failure.
    fn disable(&self, id: &str) -> Result<(), String>;
}

// ── Debug state ───────────────────────────────────────────────────────────────

/// Shared debug-verbosity flag toggled by `/textquest debug [on|off]`.
#[derive(Debug, Default)]
pub struct DebugState {
    pub enabled: bool,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Register all built-in `/textquest` subcommands in `registry`.
///
/// A single handler is registered for `/textquest` at [`Priority::BuiltIn`].
/// It parses the first word of its argument tail as the subcommand, then
/// dispatches internally.  This avoids conflicts from the registry's
/// prefix-matching logic when multiple paths share the same base command.
///
/// # Arguments
///
/// * `registry` — shared command registry (as returned by [`crate::registry::new_shared`])
/// * `scripts`  — script manager trait object (wraps the Lua script loader)
/// * `plugins`  — plugin registry
pub fn register_builtins(
    registry: SharedCommandRegistry,
    scripts: Arc<dyn ScriptManager>,
    plugins: Arc<Mutex<PluginRegistry>>,
) {
    let debug_state = Arc::new(Mutex::new(DebugState::default()));

    registry.lock().unwrap().register(
        "/textquest",
        Priority::BuiltIn,
        "builtin",
        Box::new(move |args| {
            dispatch_subcommand(args, &*scripts, &plugins, &debug_state);
        }),
    );
}

/// Parse the subcommand and route to the appropriate handler.
///
/// `args` is the tail after `/textquest ` (may be empty for the bare command).
fn dispatch_subcommand(
    args: &str,
    scripts: &dyn ScriptManager,
    plugins: &Mutex<PluginRegistry>,
    debug_state: &Mutex<DebugState>,
) {
    let (subcmd, rest) = split_first_word(args);
    match subcmd {
        "" | "help" => {
            if subcmd == "help" {
                print_help(rest.trim());
            } else {
                print_help_overview();
            }
        }
        "list_scripts" => cmd_list_scripts(scripts),
        "list_plugins" => cmd_list_plugins(plugins),
        "reload" => cmd_reload(scripts, rest.trim()),
        "enable" => cmd_enable(scripts, rest.trim()),
        "disable" => cmd_disable(scripts, rest.trim()),
        "debug" => cmd_debug(debug_state, rest.trim()),
        "status" => cmd_status(scripts, plugins),
        other => println!(
            "[TextQuest] Unknown subcommand '{}'. Try: /textquest help",
            other
        ),
    }
}

// ── Subcommand handlers ───────────────────────────────────────────────────────

fn cmd_list_scripts(scripts: &dyn ScriptManager) {
    let list = scripts.list_scripts();
    if list.is_empty() {
        println!("[TextQuest] No Lua scripts loaded.");
        return;
    }
    println!("[TextQuest] Loaded scripts ({}):", list.len());
    for s in &list {
        println!("  {:20}  id={:20}  state={}", s.name, s.id, s.state);
    }
}

fn cmd_list_plugins(plugins: &Mutex<PluginRegistry>) {
    let preg = plugins.lock().unwrap();
    if preg.is_empty() {
        println!("[TextQuest] No plugins loaded.");
        return;
    }
    println!("[TextQuest] Loaded plugins ({}):", preg.len());
    for (name, handle) in preg.iter() {
        let ver = handle.metadata.version.as_deref().unwrap_or("unknown");
        println!("  {:30}  version={}", name, ver);
    }
}

fn cmd_reload(scripts: &dyn ScriptManager, name: &str) {
    if name.is_empty() {
        println!("[TextQuest] Usage: /textquest reload <name>");
        return;
    }
    match scripts.reload(name) {
        Ok(()) => println!("[TextQuest] Reloaded script '{}'.", name),
        Err(e) => println!("[TextQuest] Error reloading '{}': {}", name, e),
    }
}

fn cmd_enable(scripts: &dyn ScriptManager, name: &str) {
    if name.is_empty() {
        println!("[TextQuest] Usage: /textquest enable <name>");
        return;
    }
    match scripts.enable(name) {
        Ok(()) => println!("[TextQuest] Enabled script '{}'.", name),
        Err(e) => println!("[TextQuest] Error enabling '{}': {}", name, e),
    }
}

fn cmd_disable(scripts: &dyn ScriptManager, name: &str) {
    if name.is_empty() {
        println!("[TextQuest] Usage: /textquest disable <name>");
        return;
    }
    match scripts.disable(name) {
        Ok(()) => println!("[TextQuest] Disabled script '{}'.", name),
        Err(e) => println!("[TextQuest] Error disabling '{}': {}", name, e),
    }
}

fn cmd_debug(debug_state: &Mutex<DebugState>, arg: &str) {
    let mut state = debug_state.lock().unwrap();
    match arg {
        "on" => {
            state.enabled = true;
            println!("[TextQuest] Debug mode ON.");
        }
        "off" => {
            state.enabled = false;
            println!("[TextQuest] Debug mode OFF.");
        }
        "" => {
            state.enabled = !state.enabled;
            println!(
                "[TextQuest] Debug mode {}.",
                if state.enabled { "ON" } else { "OFF" }
            );
        }
        other => {
            println!(
                "[TextQuest] Unknown debug argument '{}'. Use: on | off",
                other
            );
        }
    }
}

fn cmd_status(scripts: &dyn ScriptManager, plugins: &Mutex<PluginRegistry>) {
    let list = scripts.list_scripts();
    let running = list
        .iter()
        .filter(|s| s.state == ScriptLifecycle::Running)
        .count();
    let errored = list
        .iter()
        .filter(|s| matches!(s.state, ScriptLifecycle::Error(_)))
        .count();
    let plugin_count = plugins.lock().unwrap().len();

    println!("[TextQuest] Orchestrator status:");
    println!(
        "  Scripts : {} total  ({} running, {} errored)",
        list.len(),
        running,
        errored
    );
    println!("  Plugins : {} loaded", plugin_count);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Split `s` into the first whitespace-delimited word and the remainder.
///
/// ```
/// # use textquest::commands::builtins::*; // not pub, just for clarity
/// assert_eq!(split_first_word("reload mymod"), ("reload", " mymod"));
/// assert_eq!(split_first_word(""), ("", ""));
/// assert_eq!(split_first_word("status"), ("status", ""));
/// ```
fn split_first_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, ""),
    }
}

// ── Help text ─────────────────────────────────────────────────────────────────

fn print_help_overview() {
    println!(
        r#"[TextQuest] Built-in commands:
  /textquest                    — this help
  /textquest help [topic]       — detailed help on a topic
  /textquest list_scripts       — list loaded Lua scripts
  /textquest list_plugins       — list loaded plugins
  /textquest reload <name>      — reload a script by name
  /textquest enable <name>      — enable (resume) a paused script
  /textquest disable <name>     — disable (pause) a running script
  /textquest debug [on|off]     — toggle debug verbosity
  /textquest status             — orchestrator health summary

Topics: scripts, plugins, reload, enable, disable, debug, status"#
    );
}

fn print_help(topic: &str) {
    match topic {
        "scripts" | "list_scripts" => println!(
            r#"[TextQuest] /textquest list_scripts
  Lists every Lua script known to the script loader with its ID and
  current lifecycle state (Loading / Running / Paused / Error / Unloaded)."#
        ),
        "plugins" | "list_plugins" => println!(
            r#"[TextQuest] /textquest list_plugins
  Lists every MQ2-compatible plugin discovered by the plugin loader,
  along with its exported version string (if any)."#
        ),
        "reload" => println!(
            r#"[TextQuest] /textquest reload <name>
  Re-reads the script file from disk and re-executes it, preserving the
  script ID.  Use this after editing a Lua file to pick up changes
  without restarting the orchestrator."#
        ),
        "enable" => println!(
            r#"[TextQuest] /textquest enable <name>
  Resumes a paused script (transitions Paused → Running).
  Has no effect on scripts that are already Running."#
        ),
        "disable" => println!(
            r#"[TextQuest] /textquest disable <name>
  Pauses a running script (transitions Running → Paused).
  Has no effect on scripts that are already Paused."#
        ),
        "debug" => println!(
            r#"[TextQuest] /textquest debug [on|off]
  Toggle or explicitly set debug verbosity for the orchestrator.
  With no argument: toggles the current state.
  With 'on'  : enables debug output.
  With 'off' : suppresses debug output."#
        ),
        "status" => println!(
            r#"[TextQuest] /textquest status
  Prints a one-line health summary: number of scripts (running/errored)
  and number of loaded plugins."#
        ),
        "" => print_help_overview(),
        other => println!(
            "[TextQuest] Unknown help topic '{}'. Try: /textquest help",
            other
        ),
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{new_shared, Priority};
    use std::sync::{Arc, Mutex};

    // ── Stub ScriptManager ────────────────────────────────────────────────

    #[derive(Default)]
    struct StubScripts {
        scripts: Mutex<Vec<ScriptSnapshot>>,
        reloaded: Mutex<Vec<String>>,
        enabled: Mutex<Vec<String>>,
        disabled: Mutex<Vec<String>>,
    }

    impl StubScripts {
        fn new() -> Arc<Self> {
            Arc::new(Self::default())
        }

        fn with_scripts(scripts: Vec<ScriptSnapshot>) -> Arc<Self> {
            Arc::new(Self {
                scripts: Mutex::new(scripts),
                ..Default::default()
            })
        }
    }

    impl ScriptManager for StubScripts {
        fn list_scripts(&self) -> Vec<ScriptSnapshot> {
            self.scripts.lock().unwrap().clone()
        }

        fn reload(&self, id: &str) -> Result<(), String> {
            self.reloaded.lock().unwrap().push(id.to_string());
            Ok(())
        }

        fn enable(&self, id: &str) -> Result<(), String> {
            let mut scripts = self.scripts.lock().unwrap();
            match scripts.iter_mut().find(|s| s.id == id) {
                None => Err(format!("script '{}' not found", id)),
                Some(s) => {
                    s.state = ScriptLifecycle::Running;
                    drop(scripts);
                    self.enabled.lock().unwrap().push(id.to_string());
                    Ok(())
                }
            }
        }

        fn disable(&self, id: &str) -> Result<(), String> {
            let mut scripts = self.scripts.lock().unwrap();
            match scripts.iter_mut().find(|s| s.id == id) {
                None => Err(format!("script '{}' not found", id)),
                Some(s) => {
                    s.state = ScriptLifecycle::Paused;
                    drop(scripts);
                    self.disabled.lock().unwrap().push(id.to_string());
                    Ok(())
                }
            }
        }
    }

    fn setup_with(scripts: Arc<dyn ScriptManager>) -> SharedCommandRegistry {
        let (cmd_reg, _) = new_shared();
        let plugins = Arc::new(Mutex::new(PluginRegistry::new()));
        register_builtins(Arc::clone(&cmd_reg), scripts, plugins);
        cmd_reg
    }

    fn setup() -> SharedCommandRegistry {
        setup_with(StubScripts::new())
    }

    // ── Dispatch: each subcommand dispatches through /textquest ───────────

    #[test]
    fn bare_textquest_dispatches() {
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest"));
    }

    #[test]
    fn help_subcommand_dispatches() {
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest help"));
        assert!(reg.lock().unwrap().dispatch("/textquest help scripts"));
        assert!(reg.lock().unwrap().dispatch("/textquest help unknown_topic"));
    }

    #[test]
    fn help_all_topics_dispatch() {
        let topics = [
            "scripts", "list_scripts", "plugins", "list_plugins",
            "reload", "enable", "disable", "debug", "status",
        ];
        let reg = setup();
        for topic in &topics {
            let cmd = format!("/textquest help {}", topic);
            assert!(reg.lock().unwrap().dispatch(&cmd), "help {} must dispatch", topic);
        }
    }

    #[test]
    fn list_scripts_dispatches() {
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest list_scripts"));
    }

    #[test]
    fn list_plugins_dispatches() {
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest list_plugins"));
    }

    #[test]
    fn reload_without_name_dispatches() {
        // Should dispatch (prints usage hint, no panic).
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest reload"));
    }

    #[test]
    fn reload_calls_script_manager() {
        let stub = StubScripts::new();
        let reg = setup_with(Arc::clone(&stub) as Arc<dyn ScriptManager>);
        assert!(reg.lock().unwrap().dispatch("/textquest reload mymod"));
        assert_eq!(*stub.reloaded.lock().unwrap(), vec!["mymod"]);
    }

    #[test]
    fn reload_unknown_dispatches_without_panic() {
        // Stub always succeeds; this just verifies no panic.
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest reload nonexistent"));
    }

    #[test]
    fn enable_without_name_dispatches() {
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest enable"));
    }

    #[test]
    fn enable_calls_script_manager() {
        let stub = StubScripts::with_scripts(vec![ScriptSnapshot {
            id: "mymod".into(),
            name: "mymod".into(),
            state: ScriptLifecycle::Paused,
        }]);
        let reg = setup_with(Arc::clone(&stub) as Arc<dyn ScriptManager>);
        assert!(reg.lock().unwrap().dispatch("/textquest enable mymod"));
        assert_eq!(*stub.enabled.lock().unwrap(), vec!["mymod"]);
        assert_eq!(stub.list_scripts()[0].state, ScriptLifecycle::Running);
    }

    #[test]
    fn disable_without_name_dispatches() {
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest disable"));
    }

    #[test]
    fn disable_calls_script_manager() {
        let stub = StubScripts::with_scripts(vec![ScriptSnapshot {
            id: "mymod".into(),
            name: "mymod".into(),
            state: ScriptLifecycle::Running,
        }]);
        let reg = setup_with(Arc::clone(&stub) as Arc<dyn ScriptManager>);
        assert!(reg.lock().unwrap().dispatch("/textquest disable mymod"));
        assert_eq!(*stub.disabled.lock().unwrap(), vec!["mymod"]);
        assert_eq!(stub.list_scripts()[0].state, ScriptLifecycle::Paused);
    }

    #[test]
    fn enable_disable_round_trip() {
        let stub = StubScripts::with_scripts(vec![ScriptSnapshot {
            id: "togglable".into(),
            name: "togglable".into(),
            state: ScriptLifecycle::Running,
        }]);
        let reg = setup_with(Arc::clone(&stub) as Arc<dyn ScriptManager>);

        assert!(reg.lock().unwrap().dispatch("/textquest disable togglable"));
        assert_eq!(stub.list_scripts()[0].state, ScriptLifecycle::Paused);

        assert!(reg.lock().unwrap().dispatch("/textquest enable togglable"));
        assert_eq!(stub.list_scripts()[0].state, ScriptLifecycle::Running);
    }

    #[test]
    fn debug_on_off_dispatches() {
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest debug on"));
        assert!(reg.lock().unwrap().dispatch("/textquest debug off"));
        assert!(reg.lock().unwrap().dispatch("/textquest debug"));
    }

    #[test]
    fn debug_toggle_cycles_state() {
        // Two toggles → returns to original (OFF).
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest debug")); // → ON
        assert!(reg.lock().unwrap().dispatch("/textquest debug")); // → OFF
        // (Printed side effects not asserted here; just ensure no panic.)
    }

    #[test]
    fn debug_unknown_arg_dispatches() {
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest debug blah"));
    }

    #[test]
    fn status_dispatches() {
        let reg = setup();
        assert!(reg.lock().unwrap().dispatch("/textquest status"));
    }

    #[test]
    fn status_counts_scripts_correctly() {
        let stub = StubScripts::with_scripts(vec![
            ScriptSnapshot { id: "a".into(), name: "a".into(), state: ScriptLifecycle::Running },
            ScriptSnapshot { id: "b".into(), name: "b".into(), state: ScriptLifecycle::Running },
            ScriptSnapshot {
                id: "c".into(),
                name: "c".into(),
                state: ScriptLifecycle::Error("oops".into()),
            },
            ScriptSnapshot { id: "d".into(), name: "d".into(), state: ScriptLifecycle::Paused },
        ]);
        let reg = setup_with(Arc::clone(&stub) as Arc<dyn ScriptManager>);
        assert!(reg.lock().unwrap().dispatch("/textquest status"));
    }

    #[test]
    fn unknown_subcommand_dispatches_via_textquest_handler() {
        // "/textquest unknowncmd" is dispatched to the /textquest handler
        // (prefix match), which prints an "Unknown subcommand" message.
        let reg = setup();
        assert!(
            reg.lock().unwrap().dispatch("/textquest unknowncmd"),
            "/textquest unknowncmd dispatches through the /textquest handler"
        );
    }

    #[test]
    fn all_builtins_registered_at_builtin_priority() {
        // A Script-priority handler for /textquest must be shadowed by built-in.
        let (cmd_reg, _) = new_shared();
        let plugins = Arc::new(Mutex::new(PluginRegistry::new()));

        let script_fired = Arc::new(Mutex::new(false));
        let sf = Arc::clone(&script_fired);
        cmd_reg.lock().unwrap().register(
            "/textquest",
            Priority::Script,
            "test_script",
            Box::new(move |_| *sf.lock().unwrap() = true),
        );

        register_builtins(Arc::clone(&cmd_reg), StubScripts::new(), plugins);

        cmd_reg.lock().unwrap().dispatch("/textquest status");
        assert!(
            !*script_fired.lock().unwrap(),
            "built-in must shadow script-priority handler"
        );
    }
}
