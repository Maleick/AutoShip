//! Stateful admin-monitoring metrics for managed sessions.

use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use textquest_common::types::ClientId;

const DEFAULT_IPC_RETENTION: usize = 120;
const DEFAULT_MEMORY_RETENTION: usize = 120;
const DEFAULT_ERROR_RETENTION: usize = 120;
const ERROR_RATE_WINDOW: Duration = Duration::from_secs(60);

/// Sample-retention settings for per-session admin monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminMonitoringRetention {
    pub ipc_samples: usize,
    pub memory_samples: usize,
    pub error_events: usize,
}

impl Default for AdminMonitoringRetention {
    fn default() -> Self {
        Self {
            ipc_samples: DEFAULT_IPC_RETENTION,
            memory_samples: DEFAULT_MEMORY_RETENTION,
            error_events: DEFAULT_ERROR_RETENTION,
        }
    }
}

/// High-level session state for admin-monitoring snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitoredSessionState {
    /// The session is currently attached to a live PID.
    Active,
    /// The session exited and is retaining its historical samples.
    Exited,
    /// The session is no longer attached to a live PID but historical samples
    /// remain available.
    Absent,
}

/// Categorized errors recorded against a managed session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionErrorKind {
    MissingSessionToken,
    PipeConnect,
    PipeAuth,
    IpcDispatch,
    HealthCheck,
    LaunchFailure,
}

#[derive(Debug, Clone, Copy)]
struct TimedSample<T> {
    at: Instant,
    value: T,
}

#[derive(Debug, Clone)]
struct SessionMonitoringData {
    pid: Option<u32>,
    state: MonitoredSessionState,
    ipc_latency_ms: VecDeque<TimedSample<u64>>,
    memory_bytes: VecDeque<TimedSample<u64>>,
    error_events: VecDeque<TimedSample<SessionErrorKind>>,
    total_errors: u64,
    last_error_kind: Option<SessionErrorKind>,
}

impl SessionMonitoringData {
    fn new() -> Self {
        Self {
            pid: None,
            state: MonitoredSessionState::Absent,
            ipc_latency_ms: VecDeque::new(),
            memory_bytes: VecDeque::new(),
            error_events: VecDeque::new(),
            total_errors: 0,
            last_error_kind: None,
        }
    }
}

/// Derived IPC latency statistics for a session.
#[derive(Debug, Clone, PartialEq)]
pub struct LatencySummary {
    pub sample_count: usize,
    pub last_ms: Option<u64>,
    pub p50_ms: Option<u64>,
    pub p95_ms: Option<u64>,
    pub p99_ms: Option<u64>,
}

/// Derived memory-growth statistics for a session.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryTrendSummary {
    pub sample_count: usize,
    pub current_bytes: Option<u64>,
    pub delta_bytes: Option<i64>,
    pub growth_bytes_per_minute: Option<f64>,
}

/// Derived error-rate statistics for a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorRateSummary {
    pub sample_count: usize,
    pub total_errors: u64,
    pub errors_per_minute: u64,
    pub last_error_kind: Option<SessionErrorKind>,
}

/// Reusable admin-monitoring snapshot for one managed session.
///
/// Behavior notes:
/// - `state` stays available after a session exits or becomes absent so later
///   callers can inspect retained history without scraping live state.
/// - `ipc_latency.*` values remain `None` until at least one successful IPC
///   round-trip is recorded.
/// - `memory.delta_bytes` and `memory.growth_bytes_per_minute` remain `None`
///   until at least two memory samples exist.
/// - `errors.errors_per_minute` reports the count of retained error events that
///   occurred in the trailing 60-second window ending at the snapshot time.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionMonitoringSnapshot {
    pub client_id: ClientId,
    pub pid: Option<u32>,
    pub state: MonitoredSessionState,
    pub ipc_latency: LatencySummary,
    pub memory: MemoryTrendSummary,
    pub errors: ErrorRateSummary,
}

/// Bounded in-memory monitoring store for admin diagnostics.
#[derive(Debug, Default)]
pub struct AdminMonitoringStore {
    retention: AdminMonitoringRetention,
    sessions: HashMap<ClientId, SessionMonitoringData>,
    pid_to_client_id: HashMap<u32, ClientId>,
}

impl AdminMonitoringStore {
    /// Create a monitoring store with default retention.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a monitoring store with explicit retention settings.
    #[must_use]
    pub fn with_retention(retention: AdminMonitoringRetention) -> Self {
        Self {
            retention,
            sessions: HashMap::new(),
            pid_to_client_id: HashMap::new(),
        }
    }

    /// Mark a session as active and associate it with the provided PID.
    pub fn register_session(&mut self, client_id: ClientId, pid: u32) {
        let previous_client_id = self
            .pid_to_client_id
            .get(&pid)
            .copied()
            .filter(|existing| *existing != client_id);
        if let Some(previous_client_id) = previous_client_id {
            self.mark_session_absent(previous_client_id);
        }

        let previous_pid = self
            .sessions
            .get(&client_id)
            .and_then(|session| session.pid)
            .filter(|existing| *existing != pid);
        if let Some(previous_pid) = previous_pid {
            self.detach_process(previous_pid);
        }

        let session = self.ensure_session(client_id);
        session.pid = Some(pid);
        session.state = MonitoredSessionState::Active;
        self.pid_to_client_id.insert(pid, client_id);
    }

    /// Mark a session as exited while preserving retained samples.
    pub fn mark_session_exited(&mut self, client_id: ClientId) {
        self.ensure_session(client_id).state = MonitoredSessionState::Exited;
    }

    /// Mark a session as absent and clear the active PID association.
    pub fn mark_session_absent(&mut self, client_id: ClientId) {
        let previous_pid = self
            .sessions
            .get(&client_id)
            .and_then(|session| session.pid);
        if let Some(previous_pid) = previous_pid {
            self.pid_to_client_id.remove(&previous_pid);
        }

        let session = self.ensure_session(client_id);
        session.pid = None;
        session.state = MonitoredSessionState::Absent;
    }

    /// Detach a PID from the monitoring store, keeping retained history.
    pub fn detach_process(&mut self, pid: u32) {
        let Some(client_id) = self.pid_to_client_id.remove(&pid) else {
            return;
        };
        let Some(session) = self.sessions.get_mut(&client_id) else {
            return;
        };
        if session.pid == Some(pid) {
            if session.state == MonitoredSessionState::Active {
                session.pid = None;
                session.state = MonitoredSessionState::Absent;
            }
        }
    }

    /// Resolve the tracked client ID for a PID, if the store knows it.
    #[must_use]
    pub fn client_id_for_pid(&self, pid: u32) -> Option<ClientId> {
        self.pid_to_client_id.get(&pid).copied()
    }

    /// Record a successful IPC round-trip in milliseconds.
    pub fn record_ipc_latency(&mut self, client_id: ClientId, latency_ms: u64) {
        self.record_ipc_latency_at(client_id, latency_ms, Instant::now());
    }

    /// Record a successful IPC round-trip at an explicit time.
    pub fn record_ipc_latency_at(&mut self, client_id: ClientId, latency_ms: u64, at: Instant) {
        let retention = self.retention.ipc_samples;
        let session = self.ensure_session(client_id);
        session.ipc_latency_ms.push_back(TimedSample {
            at,
            value: latency_ms,
        });
        trim_to_capacity(&mut session.ipc_latency_ms, retention);
    }

    /// Record an observed memory sample for a session.
    pub fn record_memory_sample(&mut self, client_id: ClientId, memory_bytes: u64) {
        self.record_memory_sample_at(client_id, memory_bytes, Instant::now());
    }

    /// Record an observed memory sample at an explicit time.
    pub fn record_memory_sample_at(&mut self, client_id: ClientId, memory_bytes: u64, at: Instant) {
        let retention = self.retention.memory_samples;
        let session = self.ensure_session(client_id);
        session.memory_bytes.push_back(TimedSample {
            at,
            value: memory_bytes,
        });
        trim_to_capacity(&mut session.memory_bytes, retention);
    }

    /// Record an operational error for a session.
    pub fn record_error(&mut self, client_id: ClientId, kind: SessionErrorKind) {
        self.record_error_at(client_id, kind, Instant::now());
    }

    /// Record an operational error at an explicit time.
    pub fn record_error_at(&mut self, client_id: ClientId, kind: SessionErrorKind, at: Instant) {
        let retention = self.retention.error_events;
        let session = self.ensure_session(client_id);
        session.total_errors = session.total_errors.saturating_add(1);
        session.last_error_kind = Some(kind);
        session
            .error_events
            .push_back(TimedSample { at, value: kind });
        trim_to_capacity(&mut session.error_events, retention);
    }

    /// Build a derived monitoring snapshot for one session.
    #[must_use]
    pub fn snapshot(&self, client_id: ClientId, now: Instant) -> Option<SessionMonitoringSnapshot> {
        let session = self.sessions.get(&client_id)?;
        Some(SessionMonitoringSnapshot {
            client_id,
            pid: session.pid,
            state: session.state,
            ipc_latency: build_latency_summary(&session.ipc_latency_ms),
            memory: build_memory_summary(&session.memory_bytes),
            errors: build_error_summary(session, now),
        })
    }

    fn ensure_session(&mut self, client_id: ClientId) -> &mut SessionMonitoringData {
        self.sessions
            .entry(client_id)
            .or_insert_with(SessionMonitoringData::new)
    }
}

fn trim_to_capacity<T>(samples: &mut VecDeque<TimedSample<T>>, capacity: usize) {
    if capacity == 0 {
        samples.clear();
        return;
    }
    while samples.len() > capacity {
        samples.pop_front();
    }
}

fn build_latency_summary(samples: &VecDeque<TimedSample<u64>>) -> LatencySummary {
    let values = samples
        .iter()
        .map(|sample| sample.value)
        .collect::<Vec<_>>();
    LatencySummary {
        sample_count: values.len(),
        last_ms: samples.back().map(|sample| sample.value),
        p50_ms: percentile(&values, 50),
        p95_ms: percentile(&values, 95),
        p99_ms: percentile(&values, 99),
    }
}

fn build_memory_summary(samples: &VecDeque<TimedSample<u64>>) -> MemoryTrendSummary {
    let current = samples.back().map(|sample| sample.value);
    let Some(oldest) = samples.front() else {
        return MemoryTrendSummary {
            sample_count: 0,
            current_bytes: None,
            delta_bytes: None,
            growth_bytes_per_minute: None,
        };
    };
    let newest = samples.back().expect("front implies back");
    if samples.len() < 2 {
        return MemoryTrendSummary {
            sample_count: 1,
            current_bytes: current,
            delta_bytes: None,
            growth_bytes_per_minute: None,
        };
    }

    let delta = newest.value as i128 - oldest.value as i128;
    let span_secs = newest
        .at
        .checked_duration_since(oldest.at)
        .unwrap_or(Duration::ZERO)
        .as_secs_f64();
    MemoryTrendSummary {
        sample_count: samples.len(),
        current_bytes: current,
        delta_bytes: Some(clamp_i128_to_i64(delta)),
        growth_bytes_per_minute: (span_secs > 0.0).then_some(delta as f64 / span_secs * 60.0),
    }
}

fn build_error_summary(session: &SessionMonitoringData, now: Instant) -> ErrorRateSummary {
    let errors_per_minute = session
        .error_events
        .iter()
        .filter(|sample| {
            now.checked_duration_since(sample.at)
                .is_some_and(|age| age <= ERROR_RATE_WINDOW)
        })
        .count() as u64;

    ErrorRateSummary {
        sample_count: session.error_events.len(),
        total_errors: session.total_errors,
        errors_per_minute,
        last_error_kind: session.last_error_kind,
    }
}

fn percentile(values: &[u64], pct: usize) -> Option<u64> {
    if values.is_empty() {
        return None;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = sorted.len().saturating_mul(pct).div_ceil(100);
    let index = rank.saturating_sub(1);
    sorted.get(index).copied()
}

fn clamp_i128_to_i64(value: i128) -> i64 {
    value.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

/// Best-effort resident-memory sampler for a process.
#[must_use]
#[cfg(target_os = "linux")]
pub fn sample_process_memory_bytes(pid: u32) -> Option<u64> {
    let path = format!("/proc/{pid}/status");
    let content = std::fs::read_to_string(path).ok()?;
    let line = content.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kb = line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())?;
    Some(kb.saturating_mul(1024))
}

/// Best-effort resident-memory sampler for a process.
#[must_use]
#[cfg(windows)]
pub fn sample_process_memory_bytes(pid: u32) -> Option<u64> {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
    };

    let handle =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) }.ok()?;
    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    let result = unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    }
    .ok()
    .map(|_| counters.WorkingSetSize as u64);
    let _ = unsafe { CloseHandle(handle) };
    result
}

/// Best-effort resident-memory sampler for unsupported non-Windows targets.
#[must_use]
#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn sample_process_memory_bytes(_pid: u32) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn retention(
        ipc_samples: usize,
        memory_samples: usize,
        error_events: usize,
    ) -> AdminMonitoringRetention {
        AdminMonitoringRetention {
            ipc_samples,
            memory_samples,
            error_events,
        }
    }

    #[test]
    fn snapshot_without_samples_reports_empty_derived_fields() {
        let mut store = AdminMonitoringStore::with_retention(retention(4, 4, 4));
        let now = Instant::now();

        store.register_session(7, 700);

        let snapshot = store
            .snapshot(7, now)
            .expect("session snapshot should exist");
        assert_eq!(snapshot.client_id, 7);
        assert_eq!(snapshot.pid, Some(700));
        assert_eq!(snapshot.state, MonitoredSessionState::Active);
        assert_eq!(snapshot.ipc_latency.sample_count, 0);
        assert_eq!(snapshot.ipc_latency.p50_ms, None);
        assert_eq!(snapshot.ipc_latency.p95_ms, None);
        assert_eq!(snapshot.ipc_latency.p99_ms, None);
        assert_eq!(snapshot.memory.sample_count, 0);
        assert_eq!(snapshot.memory.current_bytes, None);
        assert_eq!(snapshot.memory.delta_bytes, None);
        assert_eq!(snapshot.memory.growth_bytes_per_minute, None);
        assert_eq!(snapshot.errors.total_errors, 0);
        assert_eq!(snapshot.errors.errors_per_minute, 0);
        assert_eq!(snapshot.errors.last_error_kind, None);
    }

    #[test]
    fn ipc_samples_trim_to_retention_and_compute_percentiles() {
        let mut store = AdminMonitoringStore::with_retention(retention(5, 4, 4));
        let start = Instant::now();

        store.register_session(11, 1_111);
        for (offset_secs, latency_ms) in [(0, 10), (1, 20), (2, 30), (3, 40), (4, 50), (5, 60)] {
            store.record_ipc_latency_at(11, latency_ms, start + Duration::from_secs(offset_secs));
        }

        let snapshot = store
            .snapshot(11, start + Duration::from_secs(10))
            .expect("session snapshot should exist");
        assert_eq!(snapshot.ipc_latency.sample_count, 5);
        assert_eq!(snapshot.ipc_latency.last_ms, Some(60));
        assert_eq!(snapshot.ipc_latency.p50_ms, Some(40));
        assert_eq!(snapshot.ipc_latency.p95_ms, Some(60));
        assert_eq!(snapshot.ipc_latency.p99_ms, Some(60));
    }

    #[test]
    fn memory_trend_requires_multiple_samples_for_growth() {
        let mut store = AdminMonitoringStore::with_retention(retention(4, 4, 4));
        let start = Instant::now();

        store.register_session(21, 2_100);
        store.record_memory_sample_at(21, 512, start);

        let single = store
            .snapshot(21, start)
            .expect("session snapshot should exist");
        assert_eq!(single.memory.sample_count, 1);
        assert_eq!(single.memory.current_bytes, Some(512));
        assert_eq!(single.memory.delta_bytes, None);
        assert_eq!(single.memory.growth_bytes_per_minute, None);

        store.record_memory_sample_at(21, 640, start + Duration::from_secs(60));
        store.record_memory_sample_at(21, 896, start + Duration::from_secs(120));

        let grown = store
            .snapshot(21, start + Duration::from_secs(120))
            .expect("session snapshot should exist");
        assert_eq!(grown.memory.sample_count, 3);
        assert_eq!(grown.memory.current_bytes, Some(896));
        assert_eq!(grown.memory.delta_bytes, Some(384));
        assert!(
            grown
                .memory
                .growth_bytes_per_minute
                .is_some_and(|rate| (rate - 192.0).abs() < f64::EPSILON)
        );
    }

    #[test]
    fn error_rate_counts_recent_events_and_trims_retention() {
        let mut store = AdminMonitoringStore::with_retention(retention(4, 4, 3));
        let start = Instant::now();

        store.register_session(31, 3_100);
        store.record_error_at(
            31,
            SessionErrorKind::PipeConnect,
            start - Duration::from_secs(90),
        );
        store.record_error_at(
            31,
            SessionErrorKind::PipeAuth,
            start - Duration::from_secs(30),
        );
        store.record_error_at(
            31,
            SessionErrorKind::IpcDispatch,
            start - Duration::from_secs(10),
        );
        store.record_error_at(31, SessionErrorKind::HealthCheck, start);

        let snapshot = store
            .snapshot(31, start)
            .expect("session snapshot should exist");
        assert_eq!(snapshot.errors.sample_count, 3);
        assert_eq!(snapshot.errors.total_errors, 4);
        assert_eq!(snapshot.errors.errors_per_minute, 3);
        assert_eq!(
            snapshot.errors.last_error_kind,
            Some(SessionErrorKind::HealthCheck)
        );
    }

    #[test]
    fn rebinding_pid_marks_previous_client_absent() {
        let mut store = AdminMonitoringStore::with_retention(retention(4, 4, 4));
        let now = Instant::now();

        store.register_session(51, 5_100);
        store.record_ipc_latency_at(51, 15, now);

        store.register_session(52, 5_100);

        let previous = store.snapshot(51, now).expect("previous session snapshot");
        assert_eq!(previous.state, MonitoredSessionState::Absent);
        assert_eq!(previous.pid, None);

        let current = store.snapshot(52, now).expect("current session snapshot");
        assert_eq!(current.state, MonitoredSessionState::Active);
        assert_eq!(current.pid, Some(5_100));
    }

    #[test]
    fn future_error_events_are_excluded_from_error_rate_window() {
        let mut store = AdminMonitoringStore::with_retention(retention(4, 4, 4));
        let start = Instant::now();

        store.register_session(61, 6_100);
        store.record_error_at(
            61,
            SessionErrorKind::PipeConnect,
            start + Duration::from_secs(5),
        );

        let snapshot = store
            .snapshot(61, start)
            .expect("session snapshot should exist");
        assert_eq!(snapshot.errors.total_errors, 1);
        assert_eq!(snapshot.errors.errors_per_minute, 0);
    }

    #[test]
    fn session_state_changes_preserve_history() {
        let mut store = AdminMonitoringStore::with_retention(retention(4, 4, 4));
        let now = Instant::now();

        store.register_session(41, 4_100);
        store.record_ipc_latency_at(41, 25, now);
        store.mark_session_exited(41);

        let exited = store
            .snapshot(41, now)
            .expect("session snapshot should exist");
        assert_eq!(exited.state, MonitoredSessionState::Exited);
        assert_eq!(exited.pid, Some(4_100));
        assert_eq!(exited.ipc_latency.p50_ms, Some(25));

        store.mark_session_absent(41);

        let absent = store
            .snapshot(41, now)
            .expect("session snapshot should exist");
        assert_eq!(absent.state, MonitoredSessionState::Absent);
        assert_eq!(absent.pid, None);
        assert_eq!(absent.ipc_latency.p50_ms, Some(25));
    }
}
