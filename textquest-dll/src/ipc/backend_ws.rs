//! Outbound WebSocket transport from the injected DLL to textquest-web.
//!
//! Commands still arrive through the local named-pipe listener. Responses and
//! event notifications are mirrored to the configured backend WebSocket on a
//! background thread so the game loop never blocks on network I/O.

use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::{
        Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use textquest_common::{ipc::IpcResponse, types::ClientId};
use tungstenite::Message;
use url::Url;

const DEFAULT_BACKEND_URL: &str = "ws://localhost:3001";
const BACKEND_URL_ENV: &str = "TEXTQUEST_DLL_BACKEND_URL";
const COMPAT_BACKEND_URL_ENV: &str = "TEXTQUEST_BACKEND_URL";
const MAX_PENDING_BACKEND_MESSAGES: usize = 4096;
const INITIAL_RECONNECT_BACKOFF: Duration = Duration::from_millis(250);
const MAX_RECONNECT_BACKOFF: Duration = Duration::from_secs(30);
const IDLE_PING_INTERVAL: Duration = Duration::from_secs(5);

static BACKEND_QUEUE: OnceLock<BackendQueue> = OnceLock::new();
static BACKEND_THREAD_ACTIVE: AtomicBool = AtomicBool::new(false);
static BACKEND_QUEUE_DROPS: AtomicU64 = AtomicU64::new(0);

struct BackendQueue {
    messages: Mutex<VecDeque<IpcResponse>>,
    wake: Condvar,
}

#[derive(Serialize)]
struct BackendEnvelope<'a> {
    r#type: &'static str,
    client_id: ClientId,
    session_id: u64,
    payload: &'a IpcResponse,
}

pub fn start_sender(client_id: ClientId, session_id: u64) {
    let _ = queue();
    if BACKEND_THREAD_ACTIVE.swap(true, Ordering::SeqCst) {
        return;
    }

    if let Err(error) = thread::Builder::new()
        .name(format!("textquest-backend-ws-{client_id}"))
        .spawn(move || {
            sender_loop(client_id, session_id);
            BACKEND_THREAD_ACTIVE.store(false, Ordering::SeqCst);
        })
    {
        BACKEND_THREAD_ACTIVE.store(false, Ordering::SeqCst);
        tracing::error!(
            client_id,
            error = %error,
            "Failed to spawn backend WebSocket sender thread"
        );
    }
}

pub fn stop_sender() {
    if let Some(queue) = BACKEND_QUEUE.get() {
        queue.wake.notify_all();
    }
}

pub fn enqueue_response(response: IpcResponse) {
    if !super::is_running() {
        return;
    }

    let queue = queue();
    let Ok(mut messages) = queue.messages.lock() else {
        tracing::error!("Backend WebSocket queue mutex poisoned");
        return;
    };

    if messages.len() >= MAX_PENDING_BACKEND_MESSAGES {
        messages.pop_front();
        BACKEND_QUEUE_DROPS.fetch_add(1, Ordering::SeqCst);
    }
    messages.push_back(response);
    queue.wake.notify_one();
}

fn queue() -> &'static BackendQueue {
    BACKEND_QUEUE.get_or_init(|| BackendQueue {
        messages: Mutex::new(VecDeque::new()),
        wake: Condvar::new(),
    })
}

fn sender_loop(client_id: ClientId, session_id: u64) {
    let backend_url = match configured_backend_url() {
        Ok(url) => url,
        Err(error) => {
            tracing::error!(
                client_id,
                error = %error,
                "Backend WebSocket disabled: invalid backend_url"
            );
            return;
        }
    };

    tracing::info!(
        client_id,
        backend_url = %backend_url,
        "Backend WebSocket sender started"
    );

    let mut reconnect_backoff = INITIAL_RECONNECT_BACKOFF;
    while keep_running() {
        match tungstenite::connect(backend_url.as_str()) {
            Ok((mut socket, _response)) => {
                tracing::info!(
                    client_id,
                    backend_url = %backend_url,
                    "Connected to backend WebSocket"
                );
                reconnect_backoff = INITIAL_RECONNECT_BACKOFF;

                while keep_running() {
                    let Some(response) = wait_for_response(IDLE_PING_INTERVAL) else {
                        if keep_running()
                            && let Err(error) = socket.send(Message::Ping(Vec::new().into()))
                        {
                            tracing::warn!(
                                client_id,
                                backend_url = %backend_url,
                                error = %error,
                                "Backend WebSocket ping failed; reconnecting"
                            );
                            break;
                        }
                        continue;
                    };

                    let envelope = BackendEnvelope {
                        r#type: "ipc_response",
                        client_id,
                        session_id,
                        payload: &response,
                    };
                    let payload = match serde_json::to_string(&envelope) {
                        Ok(payload) => payload,
                        Err(error) => {
                            tracing::error!(
                                client_id,
                                error = %error,
                                "Failed to serialize backend WebSocket response"
                            );
                            continue;
                        }
                    };

                    if let Err(error) = socket.send(Message::Text(payload.into())) {
                        requeue_front(response);
                        tracing::warn!(
                            client_id,
                            backend_url = %backend_url,
                            error = %error,
                            "Backend WebSocket send failed; reconnecting"
                        );
                        break;
                    }
                }
            }
            Err(error) => {
                tracing::warn!(
                    client_id,
                    backend_url = %backend_url,
                    error = %error,
                    backoff_ms = reconnect_backoff.as_millis(),
                    "Backend WebSocket connect failed"
                );
            }
        }

        sleep_with_shutdown(reconnect_backoff);
        reconnect_backoff = std::cmp::min(reconnect_backoff * 2, MAX_RECONNECT_BACKOFF);
    }

    tracing::info!(client_id, "Backend WebSocket sender exiting");
}

fn wait_for_response(timeout: Duration) -> Option<IpcResponse> {
    let queue = queue();
    let Ok(mut messages) = queue.messages.lock() else {
        tracing::error!("Backend WebSocket queue mutex poisoned");
        return None;
    };

    loop {
        if let Some(response) = messages.pop_front() {
            return Some(response);
        }
        if !keep_running() {
            return None;
        }

        let Ok((guard, result)) = queue.wake.wait_timeout(messages, timeout) else {
            tracing::error!("Backend WebSocket queue mutex poisoned");
            return None;
        };
        messages = guard;
        if result.timed_out() {
            return None;
        }
    }
}

fn requeue_front(response: IpcResponse) {
    let queue = queue();
    let Ok(mut messages) = queue.messages.lock() else {
        tracing::error!("Backend WebSocket queue mutex poisoned");
        return;
    };

    messages.push_front(response);
    while messages.len() > MAX_PENDING_BACKEND_MESSAGES {
        messages.pop_back();
        BACKEND_QUEUE_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

fn sleep_with_shutdown(duration: Duration) {
    let queue = queue();
    if let Ok(messages) = queue.messages.lock() {
        let _ = queue.wake.wait_timeout(messages, duration);
    } else {
        thread::sleep(duration);
    }
}

fn keep_running() -> bool {
    super::is_running() && !crate::SHUTTING_DOWN.load(Ordering::SeqCst)
}

fn configured_backend_url() -> Result<String, String> {
    if let Some(raw_url) = env_backend_url(BACKEND_URL_ENV) {
        return normalize_backend_url(&raw_url);
    }
    if let Some(raw_url) = env_backend_url(COMPAT_BACKEND_URL_ENV) {
        return normalize_backend_url(&raw_url);
    }

    for path in config_paths() {
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(raw_url) = backend_url_from_toml(&contents) {
            return normalize_backend_url(&raw_url)
                .map_err(|error| format!("{}: {error}", path.display()));
        }
    }

    normalize_backend_url(DEFAULT_BACKEND_URL)
}

fn env_backend_url(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        paths.push(current_dir.join("config").join("textquest-dll.toml"));
        paths.push(current_dir.join("config").join("textquest.toml"));
    }
    if let Ok(current_exe) = std::env::current_exe()
        && let Some(exe_dir) = current_exe.parent()
    {
        paths.push(exe_dir.join("config").join("textquest-dll.toml"));
        paths.push(exe_dir.join("config").join("textquest.toml"));
    }
    paths.push(PathBuf::from(r"C:\textquest\config\textquest-dll.toml"));
    paths.push(PathBuf::from(r"C:\textquest\config\textquest.toml"));
    paths
}

fn backend_url_from_toml(contents: &str) -> Option<String> {
    let value: toml::Value = toml::from_str(contents).ok()?;
    value
        .get("backend_url")
        .and_then(toml::Value::as_str)
        .or_else(|| {
            value
                .get("dll")
                .and_then(|section| section.get("backend_url"))
                .and_then(toml::Value::as_str)
        })
        .or_else(|| {
            value
                .get("backend")
                .and_then(|section| section.get("url"))
                .and_then(toml::Value::as_str)
        })
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_backend_url(raw_url: &str) -> Result<String, String> {
    let raw_url = raw_url.trim();
    let mut url = Url::parse(raw_url).map_err(|error| error.to_string())?;
    match url.scheme() {
        "ws" | "wss" => {}
        scheme => return Err(format!("unsupported scheme `{scheme}`; expected ws or wss")),
    }
    if url.path().is_empty() || url.path() == "/" {
        url.set_path("/ws");
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_backend_base_url_to_websocket_endpoint() {
        assert_eq!(
            normalize_backend_url("ws://localhost:3001").unwrap(),
            "ws://localhost:3001/ws"
        );
    }

    #[test]
    fn reads_backend_url_from_root_toml_key() {
        assert_eq!(
            backend_url_from_toml(r#"backend_url = "wss://backend.example.com/ws""#).as_deref(),
            Some("wss://backend.example.com/ws")
        );
    }
}
