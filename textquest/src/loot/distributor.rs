//! Loot distribution FSM — Reserve → Assign → Execute.
//!
//! [`LootDistributor`] drives individual loot items through a three-phase
//! state machine.  Each phase is retried up to [`MAX_RETRIES`] times with
//! exponential back-off before the item is returned to the pending queue.
//!
//! The implementation is intentionally **pure** (no live EQ connection) so
//! that all logic can be verified with unit tests on any platform.

#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions
)]

use std::collections::VecDeque;
use std::time::Duration;

/// Maximum number of attempts per phase before an item is abandoned.
pub const MAX_RETRIES: u32 = 3;

/// Base delay for exponential back-off (doubles each retry).
pub const BACKOFF_BASE_MS: u64 = 100;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// A loot item queued for distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootItem {
    /// Unique identifier (e.g. EQ item ID).
    pub item_id: u64,
    /// Display name used for logging.
    pub name: String,
}

/// Who should receive the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentTarget {
    /// Character name of the recipient.
    pub character: String,
}

/// Outcome reported by the executor callback on each attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteResult {
    /// The action succeeded.
    Success,
    /// The target client is temporarily unavailable — eligible for retry.
    ClientOffline,
    /// A hard failure that should not be retried (e.g. item already looted).
    PermanentFailure(String),
}

// ---------------------------------------------------------------------------
// FSM phases
// ---------------------------------------------------------------------------

/// The current phase of a single distribution attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DistributionPhase {
    /// Item has been claimed; waiting to be routed to a recipient.
    Reserve,
    /// Recipient has been chosen; waiting for the pickup command to execute.
    Assign { target: AssignmentTarget },
    /// Pickup command issued; awaiting confirmation.
    Execute {
        target: AssignmentTarget,
        attempt: u32,
    },
    /// Distribution succeeded.
    Done,
    /// All retries exhausted; item returned to the pending queue.
    Failed { reason: String, permanent: bool },
}

/// A single in-flight distribution job.
#[derive(Debug, Clone)]
pub struct DistributionJob {
    /// The item being distributed.
    pub item: LootItem,
    /// Current FSM phase.
    pub phase: DistributionPhase,
    /// Total failed attempts across all phases.
    pub total_failures: u32,
}

impl DistributionJob {
    fn new(item: LootItem) -> Self {
        Self {
            item,
            phase: DistributionPhase::Reserve,
            total_failures: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Distributor
// ---------------------------------------------------------------------------

/// Callback type used to resolve a recipient for an item.
///
/// Returns `Some(target)` when a willing and eligible recipient is found,
/// or `None` when no assignment is possible right now.
pub type AssignFn = Box<dyn Fn(&LootItem) -> Option<AssignmentTarget> + Send + Sync>;

/// Callback type used to execute the pickup command.
///
/// The implementer should attempt to make the `target` character pick up
/// `item` and return an [`ExecuteResult`] indicating success or failure.
pub type ExecuteFn = Box<dyn Fn(&LootItem, &AssignmentTarget, u32) -> ExecuteResult + Send + Sync>;

/// The loot distribution state machine.
///
/// # Usage
///
/// 1. Push items with [`LootDistributor::enqueue`].
/// 2. Call [`LootDistributor::tick`] on each game/orchestrator loop
///    iteration, passing fresh `assign` and `execute` callbacks.
/// 3. Inspect [`LootDistributor::done`] and [`LootDistributor::failed`] for
///    completed items.
pub struct LootDistributor {
    /// Items waiting to begin the Reserve phase.
    pending: VecDeque<LootItem>,
    /// Items currently being processed.
    active: Option<DistributionJob>,
    /// Successfully distributed items (idempotency guard: IDs stored here).
    done: Vec<LootItem>,
    /// Items that exhausted all retries.
    failed: Vec<(LootItem, String)>,
    /// Set of item IDs already distributed (idempotency).
    distributed_ids: std::collections::HashSet<u64>,
}

impl LootDistributor {
    /// Create a new, empty distributor.
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            active: None,
            done: Vec::new(),
            failed: Vec::new(),
            distributed_ids: std::collections::HashSet::new(),
        }
    }

    /// Add `item` to the pending queue.
    ///
    /// Idempotent — if the item has already been successfully distributed
    /// (its ID is in the done set), the enqueue is silently ignored.
    pub fn enqueue(&mut self, item: LootItem) {
        if self.distributed_ids.contains(&item.item_id) {
            tracing::debug!(
                item_id = item.item_id,
                "enqueue skipped — already distributed"
            );
            return;
        }
        // Avoid double-queuing an item that is already active or pending.
        if self
            .active
            .as_ref()
            .is_some_and(|a| a.item.item_id == item.item_id)
        {
            return;
        }
        if self.pending.iter().any(|i| i.item_id == item.item_id) {
            return;
        }
        tracing::debug!(item_id = item.item_id, name = %item.name, "queued loot item");
        self.pending.push_back(item);
    }

    /// Advance the FSM by one tick.
    ///
    /// `assign` resolves a recipient; `execute` issues the pickup command.
    /// Returns the back-off duration the caller should wait before the next
    /// tick (0 ms when no back-off is needed).
    pub fn tick(&mut self, assign: &AssignFn, execute: &ExecuteFn) -> Duration {
        // Promote the next pending item if nothing is in-flight.
        if self.active.is_none()
            && let Some(item) = self.pending.pop_front()
        {
            self.active = Some(DistributionJob::new(item));
        }

        let job = match self.active.take() {
            Some(j) => j,
            None => return Duration::ZERO,
        };

        let (next_job, backoff) = self.advance(job, assign, execute);

        match next_job.phase {
            DistributionPhase::Done => {
                tracing::info!(
                    item_id = next_job.item.item_id,
                    name = %next_job.item.name,
                    "loot distribution complete"
                );
                self.distributed_ids.insert(next_job.item.item_id);
                self.done.push(next_job.item);
            }
            DistributionPhase::Failed {
                ref reason,
                permanent,
            } => {
                tracing::warn!(
                    item_id = next_job.item.item_id,
                    name = %next_job.item.name,
                    %reason,
                    permanent,
                    "loot distribution failed"
                );
                self.failed.push((next_job.item.clone(), reason.clone()));
                // Only return to the pending queue when retries were exhausted
                // (transient failure).  Permanent failures must not be re-queued
                // as they would loop indefinitely.
                if !permanent {
                    self.pending.push_back(next_job.item);
                }
            }
            _ => {
                self.active = Some(next_job);
            }
        }

        backoff
    }

    /// Drive the FSM one step for `job`, returning the updated job and any
    /// back-off duration the caller should respect.
    fn advance(
        &self,
        mut job: DistributionJob,
        assign: &AssignFn,
        execute: &ExecuteFn,
    ) -> (DistributionJob, Duration) {
        match job.phase.clone() {
            DistributionPhase::Reserve => {
                // Reserve is always instantaneous in the current design.
                tracing::debug!(item_id = job.item.item_id, "phase: Reserve → Assign");
                job.phase = match assign(&job.item) {
                    Some(target) => DistributionPhase::Assign { target },
                    None => {
                        job.total_failures += 1;
                        self.handle_failure(
                            &mut job,
                            "no eligible recipient found during Reserve".into(),
                        )
                    }
                };
                (job, Duration::ZERO)
            }

            DistributionPhase::Assign { target } => {
                tracing::debug!(
                    item_id = job.item.item_id,
                    character = %target.character,
                    "phase: Assign → Execute"
                );
                job.phase = DistributionPhase::Execute { target, attempt: 0 };
                (job, Duration::ZERO)
            }

            DistributionPhase::Execute { target, attempt } => {
                let result = execute(&job.item, &target, attempt);
                match result {
                    ExecuteResult::Success => {
                        tracing::debug!(
                            item_id = job.item.item_id,
                            character = %target.character,
                            attempt,
                            "phase: Execute → Done"
                        );
                        job.phase = DistributionPhase::Done;
                        (job, Duration::ZERO)
                    }
                    ExecuteResult::ClientOffline => {
                        let next_attempt = attempt + 1;
                        job.total_failures += 1;
                        if next_attempt >= MAX_RETRIES {
                            let reason = format!(
                                "client '{}' offline after {} attempts",
                                target.character, MAX_RETRIES
                            );
                            job.phase = self.handle_failure(&mut job.clone(), reason);
                            (job, Duration::ZERO)
                        } else {
                            let backoff = backoff_duration(next_attempt);
                            tracing::debug!(
                                item_id = job.item.item_id,
                                attempt = next_attempt,
                                backoff_ms = backoff.as_millis(),
                                "client offline — scheduling retry"
                            );
                            job.phase = DistributionPhase::Execute {
                                target,
                                attempt: next_attempt,
                            };
                            (job, backoff)
                        }
                    }
                    ExecuteResult::PermanentFailure(reason) => {
                        tracing::warn!(
                            item_id = job.item.item_id,
                            %reason,
                            "permanent failure during Execute"
                        );
                        job.total_failures += 1;
                        job.phase = DistributionPhase::Failed {
                            reason: reason.clone(),
                            permanent: true,
                        };
                        (job, Duration::ZERO)
                    }
                }
            }

            // Terminal states — should not be re-entered.
            phase @ (DistributionPhase::Done | DistributionPhase::Failed { .. }) => {
                job.phase = phase;
                (job, Duration::ZERO)
            }
        }
    }

    /// Transition `job` to `Failed` if retries are exhausted, otherwise stay
    /// in Reserve for another attempt.  Returns the new phase.
    fn handle_failure(&self, job: &mut DistributionJob, reason: String) -> DistributionPhase {
        if job.total_failures >= MAX_RETRIES {
            DistributionPhase::Failed {
                reason,
                permanent: false,
            }
        } else {
            // Back to Reserve for another assignment attempt.
            DistributionPhase::Reserve
        }
    }

    // ------------------------------------------------------------------
    // Accessors
    // ------------------------------------------------------------------

    /// Items that completed distribution successfully this session.
    pub fn done(&self) -> &[LootItem] {
        &self.done
    }

    /// Items that were returned to the queue after exhausting retries.
    pub fn failed(&self) -> &[(LootItem, String)] {
        &self.failed
    }

    /// Number of items still waiting in the pending queue.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Whether a job is currently in-flight.
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }
}

impl Default for LootDistributor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute exponential back-off: `BACKOFF_BASE_MS * 2^attempt`.
fn backoff_duration(attempt: u32) -> Duration {
    let shift = u64::from(attempt).min(63);
    let multiplier = 1u64.wrapping_shl(shift as u32);
    let ms = BACKOFF_BASE_MS.saturating_mul(multiplier);
    Duration::from_millis(ms)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64) -> LootItem {
        LootItem {
            item_id: id,
            name: format!("TestItem#{id}"),
        }
    }

    fn target(name: &str) -> AssignmentTarget {
        AssignmentTarget {
            character: name.to_owned(),
        }
    }

    /// Always assigns to "Warrior".
    fn always_assign() -> AssignFn {
        Box::new(|_| Some(target("Warrior")))
    }

    /// Always succeeds on Execute.
    fn always_succeed() -> ExecuteFn {
        Box::new(|_, _, _| ExecuteResult::Success)
    }

    /// Always reports client offline.
    fn always_offline() -> ExecuteFn {
        Box::new(|_, _, _| ExecuteResult::ClientOffline)
    }

    /// Always reports a permanent failure.
    fn always_permanent() -> ExecuteFn {
        Box::new(|_, _, _| ExecuteResult::PermanentFailure("item already looted".into()))
    }

    // ------------------------------------------------------------------
    // Happy path
    // ------------------------------------------------------------------

    #[test]
    fn happy_path_completes_in_three_ticks() {
        let mut d = LootDistributor::new();
        d.enqueue(item(1));

        let assign = always_assign();
        let execute = always_succeed();

        // Tick 1: Reserve → Assign
        // Tick 2: Assign → Execute (attempt 0)
        // Tick 3: Execute → Done
        for _ in 0..3 {
            d.tick(&assign, &execute);
        }

        assert_eq!(d.done().len(), 1);
        assert_eq!(d.done()[0].item_id, 1);
        assert!(d.failed().is_empty());
        assert_eq!(d.pending_count(), 0);
        assert!(!d.is_active());
    }

    #[test]
    fn happy_path_multiple_items_sequential() {
        let mut d = LootDistributor::new();
        d.enqueue(item(10));
        d.enqueue(item(20));

        let assign = always_assign();
        let execute = always_succeed();

        // 3 ticks per item.
        for _ in 0..6 {
            d.tick(&assign, &execute);
        }

        assert_eq!(d.done().len(), 2);
    }

    // ------------------------------------------------------------------
    // Idempotency
    // ------------------------------------------------------------------

    #[test]
    fn idempotent_enqueue_skips_already_distributed() {
        let mut d = LootDistributor::new();
        d.enqueue(item(42));

        let assign = always_assign();
        let execute = always_succeed();

        for _ in 0..3 {
            d.tick(&assign, &execute);
        }
        assert_eq!(d.done().len(), 1);

        // Re-enqueue the same item — should be silently dropped.
        d.enqueue(item(42));
        assert_eq!(d.pending_count(), 0);
    }

    #[test]
    fn idempotent_enqueue_skips_double_queue() {
        let mut d = LootDistributor::new();
        d.enqueue(item(5));
        d.enqueue(item(5)); // duplicate

        assert_eq!(d.pending_count(), 1);
    }

    // ------------------------------------------------------------------
    // Client offline auto-retry
    // ------------------------------------------------------------------

    #[test]
    fn client_offline_retries_and_eventually_succeeds() {
        // First two attempts offline, third succeeds.
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = call_count.clone();

        let mut d = LootDistributor::new();
        d.enqueue(item(7));

        let assign = always_assign();
        let execute: ExecuteFn = Box::new(move |_, _, _| {
            let n = cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n < 2 {
                ExecuteResult::ClientOffline
            } else {
                ExecuteResult::Success
            }
        });

        // Reserve → Assign (tick 1)
        // Assign → Execute/0 (tick 2)
        // Execute/0 offline → Execute/1 (tick 3)
        // Execute/1 offline → Execute/2 (tick 4)
        // Execute/2 success → Done (tick 5)
        for _ in 0..5 {
            d.tick(&assign, &execute);
        }

        assert_eq!(d.done().len(), 1);
        assert!(d.failed().is_empty());
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[test]
    fn client_offline_three_failures_returns_to_queue() {
        let mut d = LootDistributor::new();
        d.enqueue(item(9));

        let assign = always_assign();
        let execute = always_offline();

        // Drive enough ticks to exhaust all retries.
        for _ in 0..10 {
            d.tick(&assign, &execute);
        }

        // Item should be in the failed list.
        assert!(!d.failed().is_empty());
        assert_eq!(d.failed()[0].0.item_id, 9);
        // Item should have been returned to the pending queue.
        assert_eq!(d.pending_count(), 1);
    }

    // ------------------------------------------------------------------
    // Permanent failure
    // ------------------------------------------------------------------

    #[test]
    fn permanent_failure_does_not_retry() {
        let mut d = LootDistributor::new();
        d.enqueue(item(3));

        let assign = always_assign();
        let execute = always_permanent();

        for _ in 0..5 {
            d.tick(&assign, &execute);
        }

        // Permanent failures are logged in the failed list …
        assert!(!d.failed().is_empty());
        assert_eq!(d.failed()[0].0.item_id, 3);
        // … but NOT re-queued (they would loop indefinitely).
        assert!(d.done().is_empty());
        assert_eq!(d.pending_count(), 0);
        assert!(!d.is_active());
    }

    // ------------------------------------------------------------------
    // Back-off duration
    // ------------------------------------------------------------------

    #[test]
    fn backoff_doubles_each_attempt() {
        assert_eq!(backoff_duration(0), Duration::from_millis(BACKOFF_BASE_MS));
        assert_eq!(
            backoff_duration(1),
            Duration::from_millis(BACKOFF_BASE_MS * 2)
        );
        assert_eq!(
            backoff_duration(2),
            Duration::from_millis(BACKOFF_BASE_MS * 4)
        );
    }

    // ------------------------------------------------------------------
    // No-assignment path
    // ------------------------------------------------------------------

    #[test]
    fn no_assignment_retries_and_fails() {
        let mut d = LootDistributor::new();
        d.enqueue(item(11));

        let assign: AssignFn = Box::new(|_| None);
        let execute = always_succeed();

        for _ in 0..20 {
            d.tick(&assign, &execute);
        }

        assert!(!d.failed().is_empty());
        assert!(d.done().is_empty());
    }
}
