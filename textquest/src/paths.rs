use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
};

const LOG_WRITE_PROBE: &str = ".textquest-log-write-test";

pub fn resolve_log_dir() -> PathBuf {
    select_writable_log_dir(log_dir_candidates())
}

pub fn data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("TEXTQUEST_DATA_DIR")
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }

    if let Ok(exe_path) = std::env::current_exe()
        && let Some(parent) = exe_path.parent()
    {
        return parent.to_path_buf();
    }

    PathBuf::from(".")
}

fn rolling_log_path_pattern(file_prefix: &str) -> PathBuf {
    resolve_log_dir().join(format!("{file_prefix}.*"))
}

pub fn orchestrator_log_path() -> PathBuf {
    rolling_log_path_pattern("textquest.log")
}

pub fn dump_log_path() -> PathBuf {
    rolling_log_path_pattern("textquest-dump.log")
}

pub fn dll_log_path() -> PathBuf {
    std::env::temp_dir()
        .join("textquest")
        .join("textquest-dll.log")
}

pub fn dump_command_label() -> &'static str {
    if cfg!(windows) {
        "textquest.exe --dump"
    } else {
        "textquest --dump"
    }
}

fn log_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(windows)]
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local_app_data).join("TextQuest").join("logs"));
    }

    if let Ok(exe_path) = std::env::current_exe()
        && let Some(parent) = exe_path.parent()
    {
        candidates.push(parent.join("logs"));
    }

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("logs"));
    }

    candidates.push(std::env::temp_dir().join("textquest").join("logs"));
    candidates
}

fn select_writable_log_dir<I>(candidates: I) -> PathBuf
where
    I: IntoIterator<Item = PathBuf>,
{
    for candidate in candidates {
        if ensure_writable_dir(&candidate).is_ok() {
            return candidate;
        }
    }

    let fallback = std::env::temp_dir().join("textquest").join("logs");
    let _ = ensure_writable_dir(&fallback);
    fallback
}

fn ensure_writable_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    let probe = probe_path(path);
    {
        let _file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&probe)?;
    }
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

fn probe_path(path: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    path.join(format!("{LOG_WRITE_PROBE}-{}-{nanos}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::{
        LOG_WRITE_PROBE, dll_log_path, dump_command_label, orchestrator_log_path, probe_path,
        select_writable_log_dir,
    };

    #[test]
    fn select_writable_log_dir_skips_uncreatable_candidate() {
        let root = std::env::temp_dir().join(format!(
            "textquest-log-dir-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("temp root");

        let blocked_parent = root.join("blocked-parent");
        std::fs::write(&blocked_parent, b"not a directory").expect("blocked parent file");
        let blocked_candidate = blocked_parent.join("logs");
        let good_candidate = root.join("good").join("logs");

        let selected = select_writable_log_dir([blocked_candidate.clone(), good_candidate.clone()]);

        assert_eq!(selected, good_candidate);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dll_log_path_uses_textquest_temp_directory() {
        let path = dll_log_path();
        assert!(
            path.ends_with(std::path::Path::new("textquest").join("textquest-dll.log")),
            "unexpected dll log path: {}",
            path.display()
        );
    }

    #[test]
    fn orchestrator_log_path_reports_daily_rolling_pattern() {
        let path = orchestrator_log_path();
        assert!(path.to_string_lossy().contains("textquest.log.*"));
    }

    #[test]
    fn probe_path_is_process_specific() {
        let path = probe_path(&std::env::temp_dir());
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        assert!(name.starts_with(LOG_WRITE_PROBE));
        assert!(name.contains(&std::process::id().to_string()));
    }

    #[test]
    fn dump_command_label_matches_platform() {
        if cfg!(windows) {
            assert_eq!(dump_command_label(), "textquest.exe --dump");
        } else {
            assert_eq!(dump_command_label(), "textquest --dump");
        }
    }
}
