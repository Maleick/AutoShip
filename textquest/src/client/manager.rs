//! `ClientManager` — discovers, tracks, and manages all EQ client sessions.

use super::{
    discovery::{MulticastPeerDiscovery, PeerDiscoveryEvent, RemotePeer},
    session::{EqSession, SlotLifecycle},
};
use crate::config::PeerDiscoveryConfig;
use anyhow::Result;
use std::{collections::HashMap, path::Path};
use textquest_common::types::ClientId;

/// Manages all EQ client sessions.
pub struct ClientManager {
    sessions: HashMap<ClientId, EqSession>,
    sessions_by_pid: HashMap<u32, ClientId>,
    next_client_id: ClientId,
    eq_process_name: String,
    peer_discovery: Option<MulticastPeerDiscovery>,
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
            peer_discovery: None,
        }
    }

    /// Enable multicast peer discovery from config.
    #[must_use]
    pub fn with_discovery_config(mut self, config: &PeerDiscoveryConfig) -> Self {
        if !config.multicast_enabled {
            return self;
        }

        match MulticastPeerDiscovery::new(config) {
            Ok(discovery) => {
                tracing::info!(
                    multicast_addr = %config.multicast_addr,
                    port = config.port,
                    "UDP multicast peer discovery enabled"
                );
                self.peer_discovery = Some(discovery);
            }
            Err(error) => {
                tracing::warn!(error = %error, "Failed to initialize UDP multicast peer discovery");
            }
        }

        self
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
            self.track_client(id, pid);
            tracing::info!(client_id = id, pid, "Discovered new EQ process");
            new_clients.push(id);
        }

        Ok(new_clients)
    }

    /// Track a client ID / PID pair produced by the launch coordinator or
    /// discovery.
    pub fn track_client(&mut self, client_id: ClientId, pid: u32) {
        if let Some(session) = self.sessions.get_mut(&client_id) {
            self.sessions_by_pid.remove(&session.pid);
            session.pid = pid;
            self.sessions_by_pid.insert(pid, client_id);
            self.next_client_id = self.next_client_id.max(client_id.saturating_add(1));
            return;
        }

        if let Some(existing_id) = self.sessions_by_pid.get(&pid).copied() {
            if existing_id != client_id {
                self.sessions.remove(&existing_id);
                self.sessions
                    .insert(client_id, EqSession::new(client_id, pid));
                self.sessions_by_pid.insert(pid, client_id);
            }
        } else if let std::collections::hash_map::Entry::Vacant(entry) =
            self.sessions.entry(client_id)
        {
            entry.insert(EqSession::new(client_id, pid));
            self.sessions_by_pid.insert(pid, client_id);
        }

        self.next_client_id = self.next_client_id.max(client_id.saturating_add(1));
    }

    /// Pump the optional multicast discovery transport.
    pub fn tick_peer_discovery(&mut self) -> Vec<PeerDiscoveryEvent> {
        let sessions: Vec<&EqSession> = self.sessions.values().collect();
        self.peer_discovery
            .as_mut()
            .map_or_else(Vec::new, |discovery| discovery.tick(sessions))
    }

    /// Get the current remote peers discovered through multicast.
    #[must_use]
    pub fn remote_peers(&self) -> Vec<&RemotePeer> {
        if let Some(discovery) = &self.peer_discovery {
            discovery.peers()
        } else {
            Vec::new()
        }
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

        let prepared = crate::inject::dll_prep::prepare_dll_locked(dll_source)?;
        crate::inject::loader::inject_dll(session.pid, prepared.path(), prepared.hash())?;

        session.dll_path = Some(prepared.path().to_path_buf());
        session.hook_status = textquest_common::types::HookStatus::Injected;

        tracing::info!(client_id, pid = session.pid, "DLL injected");
        Ok(())
    }

    /// Inject all discovered but un-injected clients.
    pub fn inject_all(&mut self, dll_source: &Path) -> Vec<(ClientId, Result<()>)> {
        let mut uninjected = Vec::new();
        for (id, session) in &self.sessions {
            if matches!(
                session.hook_status,
                textquest_common::types::HookStatus::NotInjected
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
    ///
    /// Skips clients that are already in a camp-out or exited state.
    pub fn check_health(&mut self) -> Vec<ClientId> {
        let mut needs_restart = Vec::new();

        for (id, session) in &mut self.sessions {
            // Don't flag for restart if we're already camping out or exited.
            if matches!(
                session.slot_lifecycle,
                SlotLifecycle::CampingOut | SlotLifecycle::Exited | SlotLifecycle::Relaunching
            ) {
                continue;
            }
            session.health_monitor.check();
            if session.health_monitor.should_restart() {
                needs_restart.push(*id);
            }
        }

        needs_restart
    }

    /// Initiate a graceful camp-out for a client.
    ///
    /// Transitions the session to `CampingOut` and starts the timeout tracker.
    /// The caller is responsible for sending the `/camp desktop` IPC command
    /// to the DLL via the named pipe.
    /// Returns `true` if the camp-out was initiated, `false` if the client was
    /// already camping or not found.
    pub fn initiate_camp_out(&mut self, client_id: ClientId) -> bool {
        if let Some(session) = self.sessions.get_mut(&client_id) {
            session.begin_camp_out()
        } else {
            false
        }
    }

    /// Check all camping-out sessions and return IDs that are ready for
    /// process termination (either the process exited on its own or the
    /// camp-out timeout expired).
    pub fn check_camp_outs(&mut self) -> Vec<ClientId> {
        let mut ready_to_kill = Vec::new();

        for (id, session) in &mut self.sessions {
            if session.slot_lifecycle != SlotLifecycle::CampingOut {
                continue;
            }

            if !session.health_monitor.is_process_alive() {
                // Process exited cleanly — camp succeeded.
                tracing::info!(
                    client_id = *id,
                    "Camp-out completed: process exited cleanly"
                );
                session.mark_exited();
                ready_to_kill.push(*id);
            } else if session.is_camp_out_timed_out() {
                // Camp timer expired — need to force-kill.
                tracing::warn!(
                    client_id = *id,
                    "Camp-out timed out: will force-kill process"
                );
                ready_to_kill.push(*id);
            }
        }

        ready_to_kill
    }

    /// Get all sessions currently in a camping-out state.
    #[must_use]
    pub fn camping_sessions(&self) -> Vec<&EqSession> {
        self.sessions
            .values()
            .filter(|s| s.slot_lifecycle == SlotLifecycle::CampingOut)
            .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PeerDiscoveryConfig;

    #[test]
    fn initiate_camp_out_returns_false_for_missing_client() {
        let mut mgr = ClientManager::new("eqgame.exe");
        assert!(!mgr.initiate_camp_out(999));
    }

    #[test]
    fn camping_sessions_empty_by_default() {
        let mgr = ClientManager::new("eqgame.exe");
        assert!(mgr.camping_sessions().is_empty());
    }

    #[test]
    fn check_camp_outs_ignores_non_camping_sessions() {
        let mut mgr = ClientManager::new("eqgame.exe");
        assert!(mgr.check_camp_outs().is_empty());
    }

    #[test]
    fn track_client_registers_session() {
        let mut mgr = ClientManager::new("eqgame.exe");
        mgr.track_client(7, 1234);
        let session = mgr.get(7).unwrap();
        assert_eq!(session.pid, 1234);
    }

    #[test]
    fn track_client_updates_existing_pid() {
        let mut mgr = ClientManager::new("eqgame.exe");
        mgr.track_client(7, 1234);
        mgr.track_client(7, 5678);
        let session = mgr.get(7).unwrap();
        assert_eq!(session.pid, 5678);
    }

    #[test]
    fn discovery_disabled_keeps_transport_absent() {
        let mgr =
            ClientManager::new("eqgame.exe").with_discovery_config(&PeerDiscoveryConfig::default());
        assert!(mgr.remote_peers().is_empty());
    }
}
