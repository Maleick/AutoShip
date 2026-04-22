use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use mlua::{Lua, Result as LuaResult, Value};
use thiserror::Error;

use crate::lua::bindings::LuaBindings;
use crate::lua::error::LuaApiError;

#[derive(Debug, Error)]
pub enum LuaLoaderError {
    #[error("Failed to read script: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Script execution error: {0}")]
    ExecutionError(String),

    #[error("Scripts directory not found: {0}")]
    ScriptsDirNotFound(PathBuf),

    #[error("Script not found: {0}")]
    ScriptNotFound(String),

    #[error("Invalid state transition for script '{id}': {reason}")]
    InvalidTransition { id: String, reason: String },
}

/// Lifecycle state for a managed Lua script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptState {
    /// Script is in the process of being loaded.
    Loading,
    /// Script loaded and execution is active.
    Running,
    /// Script is loaded but execution is temporarily gated.
    Paused,
    /// Script encountered an error during load or execution.
    Error(String),
    /// Script has been explicitly unloaded and globals cleaned up.
    Unloaded,
}

impl std::fmt::Display for ScriptState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScriptState::Loading => write!(f, "Loading"),
            ScriptState::Running => write!(f, "Running"),
            ScriptState::Paused => write!(f, "Paused"),
            ScriptState::Error(msg) => write!(f, "Error({})", msg),
            ScriptState::Unloaded => write!(f, "Unloaded"),
        }
    }
}

pub struct ScriptLoader {
    bindings: Arc<LuaBindings>,
    scripts_dir: PathBuf,
    loaded_scripts: RwLock<Vec<LoadedScript>>,
}

#[derive(Clone)]
pub struct LoadedScript {
    /// Unique identifier for the script (derived from stem of path on load).
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub state: ScriptState,
}

impl ScriptLoader {
    pub fn new(bindings: Arc<LuaBindings>, scripts_dir: PathBuf) -> Result<Self, LuaLoaderError> {
        if !scripts_dir.exists() {
            std::fs::create_dir_all(&scripts_dir)?;
        }

        tracing::info!(
            "ScriptLoader initialized with scripts_dir: {:?}",
            scripts_dir
        );

        Ok(Self {
            bindings,
            scripts_dir,
            loaded_scripts: RwLock::new(Vec::new()),
        })
    }

    pub fn load_init_script(&self) -> Result<(), LuaLoaderError> {
        let init_path = self.scripts_dir.join("init.lua");

        if !init_path.exists() {
            tracing::debug!("No init.lua found at {:?}", init_path);
            return Ok(());
        }

        self.load_script(&init_path)
    }

    pub fn load_character_script(&self, character_name: &str) -> Result<(), LuaLoaderError> {
        let char_dir = self.scripts_dir.join(character_name);

        if !char_dir.exists() {
            tracing::debug!("No script directory for character: {}", character_name);
            return Ok(());
        }

        let init_path = char_dir.join("init.lua");

        if init_path.exists() {
            self.load_script(&init_path)?;
        }

        Ok(())
    }

    pub fn load_script(&self, path: &Path) -> Result<(), LuaLoaderError> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let id = name.clone();

        // Mark as Loading while we execute.
        {
            let mut scripts = self.loaded_scripts.write();
            // If already tracked, transition back to Loading for the reload case.
            if let Some(entry) = scripts.iter_mut().find(|s| s.id == id) {
                entry.state = ScriptState::Loading;
            } else {
                scripts.push(LoadedScript {
                    id: id.clone(),
                    path: path.to_path_buf(),
                    name: name.clone(),
                    state: ScriptState::Loading,
                });
            }
        }

        tracing::info!(script_id = %id, path = ?path, "lifecycle: Loading");

        let source = std::fs::read_to_string(path).map_err(|e| {
            let mut scripts = self.loaded_scripts.write();
            if let Some(entry) = scripts.iter_mut().find(|s| s.id == id) {
                entry.state = ScriptState::Error(e.to_string());
            }
            LuaLoaderError::ReadError(e)
        })?;

        let exec_result = {
            let lua = self.bindings.get_lua();
            lua.load(&source).exec()
        };

        match exec_result {
            Ok(()) => {
                let mut scripts = self.loaded_scripts.write();
                if let Some(entry) = scripts.iter_mut().find(|s| s.id == id) {
                    entry.state = ScriptState::Running;
                }
                tracing::info!(script_id = %id, "lifecycle: Running");
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                let mut scripts = self.loaded_scripts.write();
                if let Some(entry) = scripts.iter_mut().find(|s| s.id == id) {
                    entry.state = ScriptState::Error(msg.clone());
                }
                tracing::error!(script_id = %id, error = %msg, "lifecycle: Error");
                Err(LuaLoaderError::ExecutionError(msg))
            }
        }
    }

    /// Unload a script by ID: removes its Lua global and transitions state to `Unloaded`.
    /// Calling `unload_script` on an already-unloaded script is a no-op.
    pub fn unload_script(&self, id: &str) -> Result<(), LuaLoaderError> {
        let mut scripts = self.loaded_scripts.write();
        let entry = scripts
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| LuaLoaderError::ScriptNotFound(id.to_string()))?;

        if entry.state == ScriptState::Unloaded {
            tracing::debug!(script_id = %id, "unload_script: already Unloaded, no-op");
            return Ok(());
        }

        // Remove the global symbol for this script (best-effort; script may not have registered one).
        {
            let lua = self.bindings.get_lua();
            let globals = lua.globals();
            let _ = globals.set(id, mlua::Value::Nil);
        }

        entry.state = ScriptState::Unloaded;
        tracing::info!(script_id = %id, "lifecycle: Unloaded");
        Ok(())
    }

    /// Pause a running script: transitions `Running` → `Paused`.
    /// Has no effect on scripts already paused; errors on scripts not in a pausable state.
    pub fn pause_script(&self, id: &str) -> Result<(), LuaLoaderError> {
        let mut scripts = self.loaded_scripts.write();
        let entry = scripts
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| LuaLoaderError::ScriptNotFound(id.to_string()))?;

        match &entry.state {
            ScriptState::Paused => {
                tracing::debug!(script_id = %id, "pause_script: already Paused, no-op");
                return Ok(());
            }
            ScriptState::Running => {
                entry.state = ScriptState::Paused;
                tracing::info!(script_id = %id, "lifecycle: Paused");
                Ok(())
            }
            other => Err(LuaLoaderError::InvalidTransition {
                id: id.to_string(),
                reason: format!("cannot pause from state '{}'", other),
            }),
        }
    }

    /// Resume a paused script: transitions `Paused` → `Running`.
    pub fn resume_script(&self, id: &str) -> Result<(), LuaLoaderError> {
        let mut scripts = self.loaded_scripts.write();
        let entry = scripts
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| LuaLoaderError::ScriptNotFound(id.to_string()))?;

        match &entry.state {
            ScriptState::Running => {
                tracing::debug!(script_id = %id, "resume_script: already Running, no-op");
                return Ok(());
            }
            ScriptState::Paused => {
                entry.state = ScriptState::Running;
                tracing::info!(script_id = %id, "lifecycle: Running (resumed)");
                Ok(())
            }
            other => Err(LuaLoaderError::InvalidTransition {
                id: id.to_string(),
                reason: format!("cannot resume from state '{}'", other),
            }),
        }
    }

    /// Reload a script: re-reads from disk and re-executes while preserving the script ID.
    pub fn reload_script(&self, id: &str) -> Result<(), LuaLoaderError> {
        let path = {
            let scripts = self.loaded_scripts.read();
            scripts
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.path.clone())
                .ok_or_else(|| LuaLoaderError::ScriptNotFound(id.to_string()))?
        };

        tracing::info!(script_id = %id, path = ?path, "lifecycle: Reloading");
        self.load_script(&path)
    }

    /// Return the current state of a script by ID.
    pub fn script_state(&self, id: &str) -> Option<ScriptState> {
        self.loaded_scripts
            .read()
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.state.clone())
    }

    pub fn load_directory(&self, dir: &Path) -> Result<(), LuaLoaderError> {
        if !dir.exists() {
            return Err(LuaLoaderError::ScriptsDirNotFound(dir.to_path_buf()));
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("lua") {
                self.load_script(&path)?;
            }
        }

        Ok(())
    }

    pub fn loaded_scripts(&self) -> Vec<LoadedScript> {
        self.loaded_scripts.read().clone()
    }

    pub fn execute_string(&self, code: &str) -> Result<Value, LuaLoaderError> {
        let lua = self.bindings.get_lua();
        let result = lua
            .load(code)
            .eval()
            .map_err(|e| LuaLoaderError::ExecutionError(e.to_string()))?;

        Ok(result)
    }

    pub fn call_function(
        &self,
        module: &str,
        function: &str,
        args: Vec<Value>,
    ) -> Result<Value, LuaLoaderError> {
        let lua = self.bindings.get_lua();

        let globals = lua.globals();
        let textquest: mlua::Table = globals
            .get("textquest")
            .map_err(|e| LuaLoaderError::ExecutionError(e.to_string()))?;

        let module_table: mlua::Table = textquest
            .get(module)
            .map_err(|e| LuaLoaderError::ExecutionError(e.to_string()))?;

        let func: mlua::Function = module_table
            .get(function)
            .map_err(|e| LuaLoaderError::ExecutionError(e.to_string()))?;

        let result = func
            .call(args)
            .map_err(|e| LuaLoaderError::ExecutionError(e.to_string()))?;

        Ok(result)
    }

    pub fn reload(&self) -> Result<(), LuaLoaderError> {
        self.loaded_scripts.write().clear();

        self.load_init_script()?;

        Ok(())
    }
}

pub fn create_loader(scripts_dir: PathBuf) -> Result<ScriptLoader, LuaLoaderError> {
    let bindings = Arc::new(LuaBindings::new()?);
    bindings.register_apis()?;

    let loader = ScriptLoader::new(bindings, scripts_dir)?;

    Ok(loader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Arc;

    fn make_loader() -> (ScriptLoader, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let scripts_dir = temp_dir.path().to_path_buf();
        let bindings = Arc::new(LuaBindings::new().expect("create bindings"));
        bindings.register_apis().expect("register APIs");
        let loader = ScriptLoader::new(bindings, scripts_dir).expect("create loader");
        (loader, temp_dir)
    }

    fn write_script(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create script file");
        f.write_all(content.as_bytes()).expect("write script");
        path
    }

    #[test]
    fn test_script_loader_creation() {
        let (loader, _dir) = make_loader();
        assert!(loader.loaded_scripts().is_empty());
    }

    #[test]
    fn test_execute_string() {
        let (loader, _dir) = make_loader();

        let result: i64 = loader
            .execute_string("return 42")
            .expect("execute")
            .cast()
            .expect("cast to i64");

        assert_eq!(result, 42);
    }

    #[test]
    fn test_load_sets_running_state() {
        let (loader, dir) = make_loader();
        let path = write_script(dir.path(), "hello.lua", "-- hello");

        loader.load_script(&path).expect("load");

        assert_eq!(loader.script_state("hello"), Some(ScriptState::Running));
    }

    #[test]
    fn test_unload_clears_state() {
        let (loader, dir) = make_loader();
        let path = write_script(dir.path(), "myscript.lua", "-- body");

        loader.load_script(&path).expect("load");
        assert_eq!(loader.script_state("myscript"), Some(ScriptState::Running));

        loader.unload_script("myscript").expect("unload");
        assert_eq!(loader.script_state("myscript"), Some(ScriptState::Unloaded));
    }

    #[test]
    fn test_double_unload_is_noop() {
        let (loader, dir) = make_loader();
        let path = write_script(dir.path(), "noop.lua", "-- x");

        loader.load_script(&path).expect("load");
        loader.unload_script("noop").expect("first unload");
        // Second unload must succeed without error.
        loader.unload_script("noop").expect("second unload (no-op)");
        assert_eq!(loader.script_state("noop"), Some(ScriptState::Unloaded));
    }

    #[test]
    fn test_pause_resume() {
        let (loader, dir) = make_loader();
        let path = write_script(dir.path(), "pausable.lua", "-- p");

        loader.load_script(&path).expect("load");
        assert_eq!(loader.script_state("pausable"), Some(ScriptState::Running));

        loader.pause_script("pausable").expect("pause");
        assert_eq!(loader.script_state("pausable"), Some(ScriptState::Paused));

        loader.resume_script("pausable").expect("resume");
        assert_eq!(loader.script_state("pausable"), Some(ScriptState::Running));
    }

    #[test]
    fn test_reload_preserves_id() {
        let (loader, dir) = make_loader();
        let path = write_script(dir.path(), "reloadme.lua", "-- v1");

        loader.load_script(&path).expect("initial load");
        assert_eq!(loader.script_state("reloadme"), Some(ScriptState::Running));

        // Overwrite content to simulate file change.
        write_script(dir.path(), "reloadme.lua", "-- v2");

        loader.reload_script("reloadme").expect("reload");
        // ID should still be tracked and Running.
        assert_eq!(loader.script_state("reloadme"), Some(ScriptState::Running));
        // Only one entry with that ID.
        let count = loader
            .loaded_scripts()
            .iter()
            .filter(|s| s.id == "reloadme")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_pause_from_non_running_errors() {
        let (loader, dir) = make_loader();
        let path = write_script(dir.path(), "errscript.lua", "-- e");

        loader.load_script(&path).expect("load");
        loader.unload_script("errscript").expect("unload");

        // Pausing an unloaded script should error.
        assert!(loader.pause_script("errscript").is_err());
    }

    #[test]
    fn test_script_not_found_errors() {
        let (loader, _dir) = make_loader();
        assert!(loader.unload_script("ghost").is_err());
        assert!(loader.pause_script("ghost").is_err());
        assert!(loader.resume_script("ghost").is_err());
        assert!(loader.reload_script("ghost").is_err());
    }
}
