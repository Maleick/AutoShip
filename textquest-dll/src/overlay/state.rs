use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
};

use super::theme::Theme;

/// Persisted geometry for a single window.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedWindow {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub minimized: bool,
}

/// Full overlay state that gets written to disk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct OverlayState {
    pub windows: HashMap<String, PersistedWindow>,
    pub theme: Theme,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Load overlay state from `path`. Returns a default state if the file does not
/// exist (first run).
pub fn load(path: &Path) -> Result<OverlayState, StateError> {
    if !path.exists() {
        return Ok(OverlayState::default());
    }
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Atomically write overlay state to `path`.
///
/// Writes to a `.tmp` sibling file first, then renames — avoids corrupting the
/// state file if the process is killed mid-write.
pub fn save(state: &OverlayState, path: &Path) -> Result<(), StateError> {
    let json = serde_json::to_vec_pretty(state)?;
    let tmp = tmp_path(path);
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut p = path.to_path_buf();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("overlay");
    p.set_file_name(format!("{file_name}.tmp"));
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_state() -> OverlayState {
        let mut s = OverlayState {
            theme: Theme::Light,
            ..Default::default()
        };
        s.windows.insert(
            "main".into(),
            PersistedWindow {
                x: 50.0,
                y: 80.0,
                width: 200.0,
                height: 150.0,
                minimized: false,
            },
        );
        s
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("overlay.json");

        let state = make_state();
        save(&state, &path).unwrap();
        let loaded = load(&path).unwrap();

        assert_eq!(loaded.theme, Theme::Light);
        let win = loaded.windows.get("main").unwrap();
        assert_eq!(win.x, 50.0);
        assert!(!win.minimized);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let state = load(&path).unwrap();
        assert_eq!(state.theme, Theme::default());
        assert!(state.windows.is_empty());
    }

    #[test]
    fn multiple_windows_persist() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("overlay.json");

        let mut state = OverlayState::default();
        for i in 0..5 {
            state.windows.insert(
                format!("win{i}"),
                PersistedWindow {
                    x: i as f32 * 10.0,
                    y: 0.0,
                    width: 100.0,
                    height: 80.0,
                    minimized: i % 2 == 0,
                },
            );
        }

        save(&state, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.windows.len(), 5);
        assert!(loaded.windows["win2"].minimized);
        assert!(!loaded.windows["win1"].minimized);
    }

    #[test]
    fn atomic_write_leaves_no_tmp_on_success() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("overlay.json");
        save(&make_state(), &path).unwrap();
        assert!(path.exists());
        assert!(!dir.path().join("overlay.json.tmp").exists());
    }
}
