use std::sync::{Mutex, OnceLock};

use textquest_common::ipc::{Response, WatchMode, WatchpointEvent, WatchpointInfo};

const MAX_WATCHPOINTS: usize = 256;
const MAX_WATCH_BYTES: usize = 64;
const MAX_POLL_BYTES_PER_TICK: usize = 4096;
const MAX_EVENTS: usize = 256;

#[derive(Debug, Clone)]
struct Watchpoint {
    label: String,
    address: usize,
    size: usize,
    mode: WatchMode,
    last_value: Vec<u8>,
}

#[derive(Debug, Default)]
struct WatchpointState {
    watchpoints: Vec<Watchpoint>,
    events: Vec<WatchpointEvent>,
    next_poll_index: usize,
}

static WATCHPOINTS: OnceLock<Mutex<WatchpointState>> = OnceLock::new();

/// Add or replace a memory watchpoint.
pub fn add(address: usize, size: usize, label: String, mode: WatchMode) -> Result<(), String> {
    validate(address, size, &label)?;

    if mode == WatchMode::Hwbp {
        return Err(
            "HWBP watchpoints require write-breakpoint DR7 support; use Poll mode in this build"
                .to_string(),
        );
    }

    let Some(last_value) = read_memory(address, size) else {
        return Err(format!(
            "watchpoint {label} could not read initial snapshot at 0x{address:X}"
        ));
    };

    let state = WATCHPOINTS.get_or_init(|| Mutex::new(WatchpointState::default()));
    let mut state = state
        .lock()
        .map_err(|_| "watchpoint state mutex poisoned".to_string())?;

    if let Some(existing) = state.watchpoints.iter_mut().find(|wp| wp.label == label) {
        existing.address = address;
        existing.size = size;
        existing.mode = mode;
        existing.last_value = last_value;
        return Ok(());
    }

    if state.watchpoints.len() >= MAX_WATCHPOINTS {
        return Err(format!(
            "maximum watchpoint count ({MAX_WATCHPOINTS}) reached"
        ));
    }

    state.watchpoints.push(Watchpoint {
        label,
        address,
        size,
        mode,
        last_value,
    });
    Ok(())
}

/// Remove a memory watchpoint by label.
pub fn remove(label: &str) -> bool {
    let Some(state) = WATCHPOINTS.get() else {
        return false;
    };
    let Ok(mut state) = state.lock() else {
        return false;
    };
    let before = state.watchpoints.len();
    state.watchpoints.retain(|wp| wp.label != label);
    if state.next_poll_index >= state.watchpoints.len() {
        state.next_poll_index = 0;
    }
    before != state.watchpoints.len()
}

/// Return active watchpoint metadata.
pub fn list() -> Vec<WatchpointInfo> {
    let Some(state) = WATCHPOINTS.get() else {
        return Vec::new();
    };
    let Ok(state) = state.lock() else {
        return Vec::new();
    };
    state.watchpoints.iter().map(Watchpoint::to_info).collect()
}

/// Drain recent watchpoint change events.
pub fn drain_log() -> Vec<WatchpointEvent> {
    let Some(state) = WATCHPOINTS.get() else {
        return Vec::new();
    };
    let Ok(mut state) = state.lock() else {
        return Vec::new();
    };
    std::mem::take(&mut state.events)
}

/// Poll active watchpoints from the game-loop tick.
pub fn tick() {
    let Some(state) = WATCHPOINTS.get() else {
        return;
    };
    let Ok(mut state) = state.lock() else {
        return;
    };
    let events = state.poll_changes();
    if events.is_empty() {
        return;
    }
    crate::ipc::send_response(Response::WatchLog { events });
}

impl Watchpoint {
    fn to_info(&self) -> WatchpointInfo {
        WatchpointInfo {
            label: self.label.clone(),
            address: self.address,
            size: self.size,
            mode: self.mode,
            last_value: self.last_value.clone(),
        }
    }
}

impl WatchpointState {
    fn poll_changes(&mut self) -> Vec<WatchpointEvent> {
        let len = self.watchpoints.len();
        if len == 0 {
            self.next_poll_index = 0;
            return Vec::new();
        }

        let mut emitted = Vec::new();
        let mut visited = 0usize;
        let mut bytes_polled = 0usize;
        let mut index = self.next_poll_index.min(len - 1);

        while visited < len {
            let size = self.watchpoints[index].size;
            if bytes_polled > 0 && bytes_polled + size > MAX_POLL_BYTES_PER_TICK {
                break;
            }
            bytes_polled += size;

            let watchpoint = &mut self.watchpoints[index];
            if watchpoint.mode == WatchMode::Poll {
                if let Some(new_value) = read_memory(watchpoint.address, watchpoint.size) {
                    if new_value != watchpoint.last_value {
                        let event = WatchpointEvent {
                            label: watchpoint.label.clone(),
                            address: watchpoint.address,
                            size: watchpoint.size,
                            mode: watchpoint.mode,
                            old_value: watchpoint.last_value.clone(),
                            new_value: new_value.clone(),
                            timestamp_ms: timestamp_ms(),
                            writer_rip: None,
                            call_stack: Vec::new(),
                        };
                        watchpoint.last_value = new_value;
                        emitted.push(event);
                    }
                }
            }

            visited += 1;
            index = (index + 1) % len;
        }

        self.next_poll_index = index;
        if !emitted.is_empty() {
            self.events.extend(emitted.iter().cloned());
            trim_events(&mut self.events);
        }
        emitted
    }
}

fn validate(address: usize, size: usize, label: &str) -> Result<(), String> {
    if address == 0 {
        return Err("watchpoint address must be non-zero".to_string());
    }
    if size == 0 || size > MAX_WATCH_BYTES {
        return Err(format!(
            "watchpoint size must be 1..={MAX_WATCH_BYTES} bytes"
        ));
    }
    if label.trim().is_empty() {
        return Err("watchpoint label must not be empty".to_string());
    }
    if label.len() > 96 {
        return Err("watchpoint label must be at most 96 bytes".to_string());
    }
    Ok(())
}

fn trim_events(events: &mut Vec<WatchpointEvent>) {
    if events.len() > MAX_EVENTS {
        let overflow = events.len() - MAX_EVENTS;
        events.drain(..overflow);
    }
}

fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn read_memory(address: usize, size: usize) -> Option<Vec<u8>> {
    if address == 0 || size == 0 {
        return None;
    }

    #[cfg(windows)]
    {
        use windows::Win32::System::{
            Diagnostics::Debug::ReadProcessMemory, Threading::GetCurrentProcess,
        };

        let mut buf = vec![0u8; size];
        let mut bytes_read = 0usize;
        unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                address as *const core::ffi::c_void,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                size,
                Some(&mut bytes_read),
            )
            .ok()?;
        }
        if bytes_read != size {
            return None;
        }
        Some(buf)
    }

    #[cfg(not(windows))]
    {
        let _ = address;
        Some(vec![0u8; size])
    }
}
