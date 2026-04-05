//! `ClientManager` — discovers, tracks, and manages all EQ client sessions.

use super::session::EqSession;
use anyhow::{Context, Result};
use dmft_common::types::ClientId;
use std::collections::HashMap;
use std::path::Path;

/// Manages all EQ client sessions.
pub struct ClientManager {
    sessions: HashMap<ClientId, EqSession>,
    sessions_by_pid: HashMap<u32, ClientId>,
    next_client_id: ClientId,
    eq_process_name: String,
}

impl ClientManager {
    #[must_use]
    /// Create a new client manager that watches for the given EQ process name.
    pub fn new(process_name: &str) -> Self {
        Self {
            sessions: HashMap::new(),
            sessions_by_pid: HashMap::new(),
            next_client_id: 1,
            eq_process_name: process_name.to_string(),
        }
    }

    /// Discover running EQ processes and create sessions for new ones.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn discover(&mut self) -> Result<Vec<ClientId>> {
        let pids = crate::process::memory::find_processes_by_name(&self.eq_process_name)?;
        let mut new_clients = Vec::new();

        for pid in pids {
            if self.sessions_by_pid.contains_key(&pid) {
                continue;
            }
            let id = self.next_client_id;
            self.next_client_id += 1;
            let session = EqSession::new(id, pid);
            tracing::info!(client_id = id, pid, "Discovered new EQ process");
            self.sessions.insert(id, session);
            self.sessions_by_pid.insert(pid, id);
            new_clients.push(id);
        }

        Ok(new_clients)
    }

    /// Inject the DLL into a specific client.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn inject(&mut self, client_id: ClientId, dll_source: &Path) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&client_id)
            .ok_or_else(|| anyhow::anyhow!("Client {client_id} not found"))?;

        let dll_bytes = std::fs::read(dll_source)
            .with_context(|| format!("Failed to read DLL: {}", dll_source.display()))?;
        let base = crate::inject::reflective::inject_reflective(session.pid, &dll_bytes)
            .map_err(|e| anyhow::anyhow!("Reflective injection failed: {e}"))?;

        tracing::info!(base = format!("{base:#x}"), "Reflective loader mapped DLL");
        session.dll_path = Some(dll_source.to_path_buf());
        session.hook_status = dmft_common::types::HookStatus::Injected;

        tracing::info!(client_id, pid = session.pid, "DLL injected");
        Ok(())
    }

    /// Inject all discovered but un-injected clients.
    pub fn inject_all(&mut self, dll_source: &Path) -> Vec<(ClientId, Result<()>)> {
        let mut uninjected = Vec::new();
        for (id, session) in &self.sessions {
            if matches!(
                session.hook_status,
                dmft_common::types::HookStatus::NotInjected
            ) {
                uninjected.push(*id);
            }
        }

        uninjected
            .into_iter()
            .map(|id| {
                let result = self.inject(id, dll_source);
                (id, result)
            })
            .collect()
    }

    /// Check health of all clients and return IDs that need restart.
    pub fn check_health(&mut self) -> Vec<ClientId> {
        let mut needs_restart = Vec::new();

        for (id, session) in &mut self.sessions {
            session.health_monitor.check();
            if session.health_monitor.should_restart() {
                needs_restart.push(*id);
            }
        }

        needs_restart
    }

    /// Get a reference to a session.
    #[must_use]
    pub fn get(&self, client_id: ClientId) -> Option<&EqSession> {
        self.sessions.get(&client_id)
    }

    /// Get a mutable reference to a session.
    pub fn get_mut(&mut self, client_id: ClientId) -> Option<&mut EqSession> {
        self.sessions.get_mut(&client_id)
    }

    /// Get all active sessions.
    #[must_use]
    pub fn active_sessions(&self) -> Vec<&EqSession> {
        self.sessions.values().filter(|s| s.is_active()).collect()
    }

    /// Total number of managed sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// All sessions.
    pub fn all_sessions(&self) -> impl Iterator<Item = &EqSession> {
        self.sessions.values()
    }

    /// Remove a session (client was shut down intentionally).
    pub fn remove(&mut self, client_id: ClientId) -> Option<EqSession> {
        if let Some(session) = self.sessions.remove(&client_id) {
            self.sessions_by_pid.remove(&session.pid);
            if let Some(ref dll_path) = session.dll_path {
                crate::inject::dll_prep::cleanup_dll(dll_path);
            }
            Some(session)
        } else {
            None
        }
    }
}
