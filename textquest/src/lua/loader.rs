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
}

pub struct ScriptLoader {
    bindings: Arc<LuaBindings>,
    scripts_dir: PathBuf,
    loaded_scripts: RwLock<Vec<LoadedScript>>,
}

pub struct LoadedScript {
    pub path: PathBuf,
    pub name: String,
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
        let source = std::fs::read_to_string(path).map_err(|e| LuaLoaderError::ReadError(e))?;

        let lua = self.bindings.get_lua();
        lua.load(&source)
            .exec()
            .map_err(|e| LuaLoaderError::ExecutionError(e.to_string()))?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        self.loaded_scripts.write().push(LoadedScript {
            path: path.to_path_buf(),
            name,
        });

        tracing::info!("Loaded script: {:?}", path);

        Ok(())
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
    use std::sync::Arc;

    #[test]
    fn test_script_loader_creation() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let scripts_dir = temp_dir.path().to_path_buf();

        let bindings = Arc::new(LuaBindings::new().expect("create bindings"));
        bindings.register_apis().expect("register APIs");

        let loader = ScriptLoader::new(bindings, scripts_dir).expect("create loader");

        assert!(loader.loaded_scripts().is_empty());
    }

    #[test]
    fn test_execute_string() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let scripts_dir = temp_dir.path().to_path_buf();

        let bindings = Arc::new(LuaBindings::new().expect("create bindings"));
        bindings.register_apis().expect("register APIs");

        let loader = ScriptLoader::new(bindings, scripts_dir).expect("create loader");

        let result: i64 = loader
            .execute_string("return 42")
            .expect("execute")
            .cast()
            .expect("cast to i64");

        assert_eq!(result, 42);
    }
}
