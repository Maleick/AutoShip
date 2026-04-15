//! IPC between the injected DLL and the TextQuest orchestrator.
//!
//! Provides two channels:
//! - **Shared memory** (`SharedStateWriter`): DLL publishes `GameState` each
//!   tick
//! - **Named pipe** (`CommandListener`): orchestrator sends `Command`s, DLL
//!   replies
//!
//! The listener runs on a background thread. Received commands are buffered in
//! a channel and drained each game tick via `poll_commands()`.

pub mod pipe;
pub mod shared;

use std::{
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use textquest_common::{
    ipc::{Command, IpcCommand, IpcResponse, Response, SessionToken},
    types::{ClientId, SharedStateFrame},
};

use self::{pipe::CommandListener, shared::SharedStateWriter};

/// Shared memory writer, created once at IPC start.
static SHARED_WRITER: OnceLock<Mutex<SharedStateWriter>> = OnceLock::new();

/// Buffered commands received from the orchestrator pipe.
static PENDING_COMMANDS: OnceLock<Mutex<Vec<IpcCommand>>> = OnceLock::new();

/// Flag checked by the listener thread to know when to exit.
static IPC_RUNNING: AtomicBool = AtomicBool::new(false);

/// True when the IPC listener thread is active.
pub fn is_running() -> bool {
    IPC_RUNNING.load(Ordering::SeqCst)
}

/// Start IPC: shared memory writer + command listener thread.
///
/// `client_id` identifies this EQ client instance. `token` is the session token
/// generated at injection time; the orchestrator must present it when
/// connecting to the command pipe.
pub fn start(client_id: ClientId, token: SessionToken) -> Result<(), Box<dyn std::error::Error>> {
    if IPC_RUNNING.load(Ordering::SeqCst) {
        tracing::warn!(client_id, "IPC already running, ignoring duplicate start");
        return Ok(());
    }
    if crate::hooks::integrity::is_safe_mode() {
        tracing::warn!(
            client_id,
            "IPC start skipped: DLL is in safe mode after hook integrity failure"
        );
        return Err(std::io::Error::other(
            "IPC start skipped: DLL is in safe mode after hook integrity failure",
        )
        .into());
    }

    // --- Shared memory writer ---
    let session_id = textquest_common::ipc::session_id_from_token(&token);
    let writer = match SharedStateWriter::new(client_id, session_id) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!(client_id, error = %e, "Failed to create shared memory writer");
            return Err(e.into());
        }
    };
    let _ = SHARED_WRITER.set(Mutex::new(writer));
    let _ = PENDING_COMMANDS.set(Mutex::new(Vec::new()));

    IPC_RUNNING.store(true, Ordering::SeqCst);

    // --- Command listener thread ---
    thread::Builder::new()
        .name(format!("textquest-ipc-{client_id}"))
        .spawn(move || {
            listener_loop(client_id, token);
        })
        .map_err(|e| {
            IPC_RUNNING.store(false, Ordering::SeqCst);
            tracing::error!(client_id, error = %e, "Failed to spawn IPC listener thread");
            e
        })?;

    tracing::info!(client_id, "IPC started");
    Ok(())
}

/// Stop IPC and clean up. Safe to call multiple times.
pub fn stop() {
    if !IPC_RUNNING.swap(false, Ordering::SeqCst) {
        return;
    }

    // The listener thread checks IPC_RUNNING and will exit on its own.
    // SharedStateWriter and CommandListener clean up via Drop.
    tracing::info!("IPC stopped");
}

/// Drain any commands received since the last call. Intended to be called once
/// per game tick from the hook thread.
pub fn poll_commands() -> Vec<IpcCommand> {
    let Some(pending) = PENDING_COMMANDS.get() else {
        return Vec::new();
    };
    let Ok(mut queue) = pending.lock() else {
        tracing::error!("IPC command queue mutex poisoned");
        return Vec::new();
    };
    std::mem::take(&mut *queue)
}

/// Publish a game state snapshot to shared memory. Intended to be called once
/// per game tick from the hook thread.
pub fn publish_state(frame: &SharedStateFrame) {
    let Some(writer_lock) = SHARED_WRITER.get() else {
        return;
    };
    let Ok(mut writer) = writer_lock.lock() else {
        tracing::error!("IPC shared memory writer mutex poisoned");
        return;
    };
    if let Err(e) = writer.write(frame) {
        tracing::error!(error = %e, "Failed to publish game state");
    }
}

/// Buffered responses to send back to the orchestrator on the next pipe write.
static PENDING_RESPONSES: OnceLock<Mutex<Vec<Response>>> = OnceLock::new();

/// Dedicated buffer for chat messages captured by the `dsp_chat` HWBP hook.
///
/// Kept separate from `PENDING_RESPONSES` so that `PollPackets` does not
/// consume chat messages and `PollChat` does not consume packet events.
static PENDING_CHAT: OnceLock<Mutex<Vec<textquest_common::ipc::ChatMessageInfo>>> = OnceLock::new();

/// Maximum number of chat messages retained in `PENDING_CHAT` before oldest
/// entries are dropped. Prevents unbounded growth when the orchestrator is not
/// polling `Command::PollChat`.
const MAX_PENDING_CHAT: usize = 2048;
/// Maximum number of IPC responses retained in `PENDING_RESPONSES` before
/// oldest entries are dropped. Prevents unbounded growth when the orchestrator
/// falls behind polling.
const MAX_PENDING_RESPONSES: usize = 4096;

/// Enqueue a response to be sent to the orchestrator.
/// Called from the game loop thread (e.g., login FSM phase updates).
pub fn send_response(response: Response) {
    if !is_running() {
        return;
    }
    let pending = PENDING_RESPONSES.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut queue) = pending.lock() {
        if queue.len() >= MAX_PENDING_RESPONSES {
            // Keep command/status responses preferred under packet flood by
            // treating packet events as lossy once the bounded queue is full.
            if matches!(response, Response::PacketEvent { .. }) {
                return;
            }

            // For non-packet responses, evict the oldest buffered entry to
            // keep the queue bounded.
            queue.remove(0);
        }
        queue.push(response);
    }
}

/// Enqueue a captured chat message into the dedicated chat buffer.
///
/// Called from the `dsp_chat` HWBP callback on every in-game chat event.
/// Messages stored here are returned by `Command::PollChat` /
/// `Response::ChatBatch`.
///
/// Skips buffering when IPC is not running. Enforces a retention cap of
/// [`MAX_PENDING_CHAT`] entries; oldest messages are dropped when the cap is
/// exceeded to prevent unbounded memory growth.
pub fn push_chat_message(text: String, color: i32, timestamp_ms: u64) {
    if !is_running() {
        return;
    }
    let pending = PENDING_CHAT.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut queue) = pending.lock() {
        if queue.len() >= MAX_PENDING_CHAT {
            // Drop oldest entries to keep the buffer bounded.
            let overflow = queue.len() - MAX_PENDING_CHAT + 1;
            queue.drain(..overflow);
        }
        queue.push(textquest_common::ipc::ChatMessageInfo {
            text,
            color,
            timestamp_ms,
        });
    }
}

/// Drain pending responses. Called by the IPC listener thread.
pub fn drain_responses() -> Vec<Response> {
    let Some(pending) = PENDING_RESPONSES.get() else {
        return Vec::new();
    };
    let Ok(mut queue) = pending.lock() else {
        return Vec::new();
    };
    std::mem::take(&mut *queue)
}

/// Drain only `SpawnEventBatch`-relevant responses from `PENDING_RESPONSES`,
/// leaving all other variants in the queue.
///
/// This prevents `PollSpawnEvents` from consuming unrelated responses
/// while preserving future non-spawn payloads.
pub fn drain_spawn_responses() -> Vec<textquest_common::ipc::SpawnEvent> {
    let Some(pending) = PENDING_RESPONSES.get() else {
        return Vec::new();
    };
    let Ok(mut queue) = pending.lock() else {
        return Vec::new();
    };
    let mut spawn_events = Vec::new();
    let mut remaining = Vec::new();
    for response in std::mem::take(&mut *queue) {
        match response {
            Response::SpawnEventBatch { events } => {
                spawn_events.extend(events);
            }
            _ => remaining.push(response),
        }
    }
    *queue = remaining;
    spawn_events
}

/// Drain only `PacketEvent` responses from `PENDING_RESPONSES`, leaving all
/// other response variants (e.g. `NavSignals`, `ContainerSlots`) intact in the
/// queue.
///
/// This prevents `PollPackets` from silently discarding unrelated queued
/// responses.
pub fn drain_packet_responses() -> Vec<textquest_common::ipc::PacketEventInfo> {
    let Some(pending) = PENDING_RESPONSES.get() else {
        return Vec::new();
    };
    let Ok(mut queue) = pending.lock() else {
        return Vec::new();
    };
    let mut packet_events = Vec::new();
    let mut remaining = Vec::new();
    for response in std::mem::take(&mut *queue) {
        match response {
            Response::PacketEvent {
                client_id,
                opcode,
                direction,
                timestamp_ms,
                payload_size,
            } => {
                packet_events.push(textquest_common::ipc::PacketEventInfo {
                    client_id,
                    opcode,
                    direction,
                    timestamp_ms,
                    payload_size,
                });
            }
            _ => remaining.push(response),
        }
    }
    *queue = remaining;
    packet_events
}

/// Drain pending chat messages. Called by the IPC listener for
/// `Command::PollChat`.
pub fn drain_chat_messages() -> Vec<textquest_common::ipc::ChatMessageInfo> {
    let Some(pending) = PENDING_CHAT.get() else {
        return Vec::new();
    };
    let Ok(mut queue) = pending.lock() else {
        return Vec::new();
    };
    std::mem::take(&mut *queue)
}

/// Handle commands that must work even before the game loop runs
/// (e.g., at the login screen). Returns true if handled.
fn handle_immediate_command(cmd: &Command) -> bool {
    const MAX_LOGIN_FIELD_CHARS: usize = 128;

    match cmd {
        Command::CalibrateLogin => {
            let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
            let eqmain_base = crate::login::eqmain::find_eqmain();
            tracing::info!(
                eq_base = format!("{:#x}", eq_base),
                eqmain_base = format!("{:#x}", eqmain_base),
                "CalibrateLogin: running calibration dump"
            );
            if eqmain_base == 0 {
                tracing::warn!("CalibrateLogin: eqmain.dll not loaded yet; skipping dump");
            } else {
                crate::login::widgets::calibrate_login_dump(eqmain_base);
            }
            true
        }
        Command::StartLogin {
            account_name,
            password,
            server_name,
            character_name,
        } => {
            let account_name: String = account_name.chars().take(MAX_LOGIN_FIELD_CHARS).collect();
            let password: String = password.chars().take(MAX_LOGIN_FIELD_CHARS).collect();

            // Zeroizing wrapper wipes the plaintext from memory on drop.
            let password = zeroize::Zeroizing::new(password);

            tracing::info!(
                account = %account_name,
                server = %server_name,
                character = %character_name,
                "StartLogin received (password redacted)"
            );

            if crate::hooks::eqmain_hook::is_active() {
                tracing::info!("Routing StartLogin through main-thread GiveTime hook");
                // Clone password before moving into queue_login — we need it
                // for the WM_CHAR backup path on this (IPC) thread.
                let pw_for_wm_char = zeroize::Zeroizing::new((*password).clone());
                crate::hooks::eqmain_hook::queue_login(
                    account_name,
                    password,
                    server_name.to_string(),
                    character_name.to_string(),
                );
                // Give main thread time to write credentials to EQLogin struct,
                // then submit login via WM_CHAR from this thread.
                // PostMessage works from any thread — the main thread pumps them.
                std::thread::sleep(std::time::Duration::from_millis(300));
                let eqmain_base = crate::login::eqmain::find_eqmain();
                if eqmain_base != 0 {
                    let typed =
                        crate::login::widgets::type_password_wm_char(eqmain_base, &pw_for_wm_char);
                    tracing::info!(typed, "IPC thread: WM_CHAR password + Enter submitted");
                }
                drop(pw_for_wm_char);
            } else {
                // Fallback: eqmain hook not installed — execute directly on IPC thread.
                tracing::warn!("eqmain hook not active, writing credentials on IPC thread");
                let eqmain_base = crate::login::eqmain::find_eqmain();
                if eqmain_base != 0 {
                    crate::login::widgets::type_credentials_to_window(
                        eqmain_base,
                        &account_name,
                        &password,
                    );
                }
                crate::login::start_login(
                    account_name,
                    (*password).clone(),
                    server_name.to_string(),
                    character_name.to_string(),
                );
            }

            true
        }
        _ => false,
    }
}

/// Background thread: creates a `CommandListener` and loops receiving commands
/// until `IPC_RUNNING` is cleared or the DLL is shutting down.
fn listener_loop(client_id: ClientId, token: SessionToken) {
    let mut listener = match CommandListener::new(client_id, token) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(client_id, error = %e, "Failed to create command listener");
            IPC_RUNNING.store(false, Ordering::SeqCst);
            return;
        }
    };

    tracing::info!(client_id, "IPC listener thread started");

    let mut consecutive_errors: u32 = 0;

    while IPC_RUNNING.load(Ordering::SeqCst) && !crate::SHUTTING_DOWN.load(Ordering::SeqCst) {
        match listener.receive() {
            Ok(ipc_cmd) => {
                consecutive_errors = 0;
                tracing::debug!(client_id, ?ipc_cmd, "Received command");

                let correlation_id = ipc_cmd.correlation_id;
                let cmd = ipc_cmd.command;

                if handle_immediate_command(&cmd) {
                    // Immediate commands get an Ack response.
                    let _ = listener.respond(&IpcResponse::echo(
                        Response::CommandResult {
                            success: true,
                            message: "handled".into(),
                        },
                        correlation_id,
                    ));
                    listener.disconnect();
                    continue;
                }

                // Respond to Ping inline — no need to queue.
                if matches!(&cmd, Command::Ping) {
                    let _ = listener.respond(&IpcResponse::echo(
                        Response::Pong {
                            client_id,
                            timestamp_ms: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map_or(0, |d| d.as_millis() as u64),
                        },
                        correlation_id,
                    ));
                    listener.disconnect();
                    continue;
                }

                // Respond to PollPackets inline — drain only packet events.
                // Uses drain_packet_responses() so other queued responses are preserved.
                if matches!(&cmd, Command::PollPackets) {
                    let events = drain_packet_responses();
                    let _ = listener.respond(&IpcResponse::echo(
                        Response::PacketBatch { events },
                        correlation_id,
                    ));
                    listener.disconnect();
                    continue;
                }

                // Respond to PollSpawnEvents inline — drain only spawn event responses.
                if matches!(&cmd, Command::PollSpawnEvents) {
                    let events = drain_spawn_responses();
                    let _ = listener.respond(&IpcResponse::echo(
                        Response::SpawnEventBatch { events },
                        correlation_id,
                    ));
                    listener.disconnect();
                    continue;
                }

                // Respond to PollChat inline — drain accumulated chat messages.
                if matches!(&cmd, Command::PollChat) {
                    let messages = drain_chat_messages();
                    let _ = listener.respond(&IpcResponse::echo(
                        Response::ChatBatch { messages },
                        correlation_id,
                    ));
                    listener.disconnect();
                    continue;
                }

                // Queue for game loop processing (bounded to prevent OOM
                // if the game loop stalls during loading screens).
                const MAX_PENDING: usize = 256;
                let queued = if let Some(pending) = PENDING_COMMANDS.get()
                    && let Ok(mut queue) = pending.lock()
                {
                    if queue.len() < MAX_PENDING {
                        queue.push(IpcCommand {
                            command: cmd,
                            correlation_id,
                        });
                        true
                    } else {
                        tracing::warn!(
                            client_id,
                            "Command queue full ({MAX_PENDING}), dropping command"
                        );
                        false
                    }
                } else {
                    false
                };

                // Send Ack so the orchestrator isn't left waiting.
                let _ = listener.respond(&IpcResponse::echo(
                    Response::CommandResult {
                        success: queued,
                        message: if queued { "queued" } else { "queue full" }.into(),
                    },
                    correlation_id,
                ));

                // Reset pipe for next connection. The orchestrator uses
                // fire-and-forget (connect → token → command → close), so the
                // server must DisconnectNamedPipe after each command to accept
                // the next client via ConnectNamedPipe.
                listener.disconnect();
            }
            Err(e) => {
                if IPC_RUNNING.load(Ordering::SeqCst) {
                    consecutive_errors += 1;
                    if consecutive_errors <= 3 {
                        tracing::warn!(client_id, error = %e, "Command listener error, resetting");
                    } else if consecutive_errors == 4 {
                        tracing::warn!(
                            client_id,
                            consecutive_errors,
                            "Suppressing repeated pipe errors"
                        );
                    }
                    // Connection dropped — disconnect will make next receive() wait
                    // for a new connection.
                    listener.disconnect();
                    let backoff_ms = std::cmp::min(10 * (1u64 << consecutive_errors.min(9)), 5000);
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                }
            }
        }
    }

    tracing::info!(client_id, "IPC listener thread exiting");
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── is_running() ──────────────────────────────────────────────────────────

    #[test]
    fn is_running_reflects_atomic_state() {
        // We cannot safely toggle IPC_RUNNING without risking interaction with
        // other tests or threads, but we can verify that `is_running()` reads
        // the same value that the AtomicBool currently holds.
        let expected = IPC_RUNNING.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(is_running(), expected);
    }

    // ─── Module-level functions before IPC start ──────────────────────────────

    #[test]
    fn poll_commands_returns_empty_vec_before_start() {
        // PENDING_COMMANDS is only set after start() is called.
        // Before any start(), get() returns None and poll_commands() returns vec![].
        if PENDING_COMMANDS.get().is_none() {
            let cmds = poll_commands();
            assert!(cmds.is_empty(), "expected empty vec before IPC start");
        }
        // If PENDING_COMMANDS is already set (another test started IPC),
        // we just skip — the test is vacuously satisfied.
    }

    // ─── send_response() before IPC start ─────────────────────────────────────

    #[test]
    fn send_response_is_safe_when_ipc_not_running() {
        // Temporarily ensure IPC is marked as not running to check the early-return
        // path in send_response().  We restore the value afterward.
        let was_running = IPC_RUNNING.swap(false, std::sync::atomic::Ordering::SeqCst);
        send_response(textquest_common::ipc::Response::Pong {
            client_id: 0,
            timestamp_ms: 0,
        });
        IPC_RUNNING.store(was_running, std::sync::atomic::Ordering::SeqCst);
    }

    // ─── publish_state() before IPC start ─────────────────────────────────────

    #[test]
    fn publish_state_is_safe_before_start() {
        // SHARED_WRITER is None before start() — publish_state should be a no-op.
        if SHARED_WRITER.get().is_none() {
            let frame = textquest_common::types::SharedStateFrame {
                client_id: 0,
                local_player: None,
                target: None,
                nearby_spawns: None,
                active_buffs: vec![],
                pet: None,
                timestamp_ms: 0,
                nav_status: textquest_common::nav::NavStatus::Idle,
                combat_status: textquest_common::combat::CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
                spawn_epoch: 0,
                actual_version: None,
            };
            publish_state(&frame); // must not panic
        }
    }

    // ─── stop() is idempotent ─────────────────────────────────────────────────

    #[test]
    fn stop_is_idempotent() {
        // Calling stop() multiple times must not panic.
        stop();
        stop();
        // Restore invariant: ensure is_running stays consistent with the stored flag.
        assert!(!is_running());
    }
}
