use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::{LlmProvider, LlmRequest, LlmResponse};
use anyhow::Result;

/// Token budget tracking per hour.
pub struct TokenBudget {
    /// Maximum tokens per hour
    pub max_tokens_per_hour: u32,
    /// Tokens consumed in the current window
    tokens_used: u32,
    /// When the current window started (unix timestamp seconds)
    window_start_secs: u64,
}

impl TokenBudget {
    /// Create a new token budget with the given hourly limit.
    #[must_use]
    pub fn new(max_tokens_per_hour: u32) -> Self {
        Self {
            max_tokens_per_hour,
            tokens_used: 0,
            window_start_secs: 0,
        }
    }

    /// Check if we have budget for the estimated token count.
    #[must_use]
    pub fn can_afford(&self, estimated_tokens: u32, now_secs: u64) -> bool {
        if self.is_window_expired(now_secs) {
            return estimated_tokens <= self.max_tokens_per_hour;
        }
        self.tokens_used + estimated_tokens <= self.max_tokens_per_hour
    }

    /// Record tokens consumed.
    pub fn consume(&mut self, tokens: u32, now_secs: u64) {
        if self.is_window_expired(now_secs) {
            self.tokens_used = 0;
            self.window_start_secs = now_secs;
        }
        self.tokens_used += tokens;
    }

    /// Tokens remaining in the current window.
    #[must_use]
    pub fn remaining(&self, now_secs: u64) -> u32 {
        if self.is_window_expired(now_secs) {
            return self.max_tokens_per_hour;
        }
        self.max_tokens_per_hour.saturating_sub(self.tokens_used)
    }

    fn is_window_expired(&self, now_secs: u64) -> bool {
        now_secs - self.window_start_secs >= 3600
    }
}

/// Wrapper to make `LlmRequest` orderable by priority for the `BinaryHeap`.
struct PrioritizedRequest {
    request: LlmRequest,
    /// Sequence number for FIFO within same priority
    seq: u64,
}

impl PartialEq for PrioritizedRequest {
    fn eq(&self, other: &Self) -> bool {
        self.request.priority == other.request.priority && self.seq == other.seq
    }
}

impl Eq for PrioritizedRequest {}

impl PartialOrd for PrioritizedRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then earlier sequence number first (FIFO)
        self.request
            .priority
            .cmp(&other.request.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

/// Priority queue for LLM requests.
/// Phase 1: processes requests immediately via the fallback provider.
/// Phase 2+: batches requests and dispatches to real LLM providers with budget control.
pub struct LlmRequestQueue {
    queue: BinaryHeap<PrioritizedRequest>,
    next_seq: u64,
    budget: TokenBudget,
}

impl LlmRequestQueue {
    /// Create a new request queue with the given hourly token budget.
    #[must_use]
    pub fn new(max_tokens_per_hour: u32) -> Self {
        Self {
            queue: BinaryHeap::new(),
            next_seq: 0,
            budget: TokenBudget::new(max_tokens_per_hour),
        }
    }

    /// Enqueue a request for processing.
    pub fn enqueue(&mut self, request: LlmRequest) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.queue.push(PrioritizedRequest { request, seq });
    }

    /// Process the highest-priority request using the given provider.
    /// Returns None if the queue is empty or budget is exhausted.
    pub fn process_next(
        &mut self,
        provider: &mut dyn LlmProvider,
        now_secs: u64,
    ) -> Option<Result<(LlmRequest, LlmResponse)>> {
        // Check budget (estimate ~200 tokens per request for LLM, 0 for fallback)
        let estimated = if provider.name() == "trait-driven-fallback" {
            0
        } else {
            200
        };

        if estimated > 0 && !self.budget.can_afford(estimated, now_secs) {
            return None;
        }

        let prioritized = self.queue.pop()?;

        let result = provider.generate(&prioritized.request);

        match &result {
            Ok(response) => {
                self.budget.consume(response.tokens_used, now_secs);
                Some(Ok((prioritized.request, response.clone())))
            }
            Err(_) => Some(result.map(|r| (prioritized.request, r))),
        }
    }

    /// Process all queued requests (Phase 1: immediate dispatch).
    pub fn process_all(
        &mut self,
        provider: &mut dyn LlmProvider,
        now_secs: u64,
    ) -> Vec<(LlmRequest, LlmResponse)> {
        let mut results = Vec::new();
        while let Some(result) = self.process_next(provider, now_secs) {
            match result {
                Ok(pair) => results.push(pair),
                Err(e) => {
                    tracing::warn!("LLM generation failed: {}", e);
                }
            }
        }
        results
    }

    /// Pop the next highest-priority request without processing it.
    /// Returns None if the queue is empty.
    pub fn pop_next(&mut self, _now_secs: u64) -> Option<LlmRequest> {
        self.queue.pop().map(|p| p.request)
    }

    /// Number of pending requests.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Tokens remaining in the current budget window.
    #[must_use]
    pub fn budget_remaining(&self, now_secs: u64) -> u32 {
        self.budget.remaining(now_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EdginessLevel;
    use crate::llm::fallback::TraitDrivenResponder;
    use crate::llm::{LlmPriority, LlmRequest, Situation};
    use textquest_common::soul::{MoodState, PersonalityTraits, SpeechStyle};

    fn make_request(name: &str, priority: LlmPriority) -> LlmRequest {
        LlmRequest {
            character_name: name.into(),
            traits: PersonalityTraits::default(),
            mood: MoodState::Neutral,
            speech_style: SpeechStyle::default(),
            situation: Situation::IdleChatter,
            priority,
            memory_context: Vec::new(),
            backstory: String::new(),
        }
    }

    // ─── TokenBudget tests ───

    #[test]
    fn token_budget_can_afford_within_limit() {
        let budget = TokenBudget::new(1000);
        assert!(budget.can_afford(500, 100));
        assert!(budget.can_afford(1000, 100));
        assert!(!budget.can_afford(1001, 100));
    }

    #[test]
    fn token_budget_consume_reduces_remaining() {
        let mut budget = TokenBudget::new(1000);
        budget.consume(400, 100);
        assert_eq!(budget.remaining(100), 600);
        assert!(budget.can_afford(600, 100));
        assert!(!budget.can_afford(601, 100));
    }

    #[test]
    fn token_budget_window_expiry_resets() {
        let mut budget = TokenBudget::new(1000);
        budget.consume(900, 100);
        assert_eq!(budget.remaining(100), 100);

        // After an hour, the window resets
        let after_hour = 100 + 3600;
        assert_eq!(budget.remaining(after_hour), 1000);
        assert!(budget.can_afford(1000, after_hour));
    }

    #[test]
    fn token_budget_consume_after_expiry_starts_new_window() {
        let mut budget = TokenBudget::new(1000);
        budget.consume(800, 100);

        // Consume after window expiry
        budget.consume(200, 100 + 3600);
        assert_eq!(budget.remaining(100 + 3600), 800);
    }

    #[test]
    fn token_budget_remaining_with_no_consumption() {
        let budget = TokenBudget::new(5000);
        assert_eq!(budget.remaining(0), 5000);
    }

    // ─── LlmRequestQueue tests ───

    #[test]
    fn enqueue_and_pop_returns_request() {
        let mut queue = LlmRequestQueue::new(10000);
        queue.enqueue(make_request("Alice", LlmPriority::Medium));

        let req = queue.pop_next(0).unwrap();
        assert_eq!(req.character_name, "Alice");
    }

    #[test]
    fn pop_next_returns_none_when_empty() {
        let mut queue = LlmRequestQueue::new(10000);
        assert!(queue.pop_next(0).is_none());
    }

    #[test]
    fn pop_next_returns_highest_priority_first() {
        let mut queue = LlmRequestQueue::new(10000);
        queue.enqueue(make_request("Low", LlmPriority::Low));
        queue.enqueue(make_request("High", LlmPriority::High));
        queue.enqueue(make_request("Medium", LlmPriority::Medium));

        let first = queue.pop_next(0).unwrap();
        assert_eq!(first.character_name, "High");

        let second = queue.pop_next(0).unwrap();
        assert_eq!(second.character_name, "Medium");

        let third = queue.pop_next(0).unwrap();
        assert_eq!(third.character_name, "Low");
    }

    #[test]
    fn same_priority_fifo_order() {
        let mut queue = LlmRequestQueue::new(10000);
        queue.enqueue(make_request("First", LlmPriority::Medium));
        queue.enqueue(make_request("Second", LlmPriority::Medium));
        queue.enqueue(make_request("Third", LlmPriority::Medium));

        let first = queue.pop_next(0).unwrap();
        assert_eq!(first.character_name, "First");

        let second = queue.pop_next(0).unwrap();
        assert_eq!(second.character_name, "Second");

        let third = queue.pop_next(0).unwrap();
        assert_eq!(third.character_name, "Third");
    }

    #[test]
    fn pending_count_tracks_queue_size() {
        let mut queue = LlmRequestQueue::new(10000);
        assert_eq!(queue.pending_count(), 0);

        queue.enqueue(make_request("A", LlmPriority::Low));
        queue.enqueue(make_request("B", LlmPriority::High));
        assert_eq!(queue.pending_count(), 2);

        queue.pop_next(0);
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn process_next_with_fallback_provider() {
        let mut queue = LlmRequestQueue::new(10000);
        queue.enqueue(make_request("Alice", LlmPriority::Medium));

        let mut provider = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let result = queue.process_next(&mut provider, 0).unwrap().unwrap();

        assert_eq!(result.0.character_name, "Alice");
        assert!(!result.1.text.is_empty());
        assert!(!result.1.from_llm);
        assert_eq!(result.1.tokens_used, 0);
    }

    #[test]
    fn process_all_drains_queue() {
        let mut queue = LlmRequestQueue::new(10000);
        queue.enqueue(make_request("A", LlmPriority::Low));
        queue.enqueue(make_request("B", LlmPriority::Medium));
        queue.enqueue(make_request("C", LlmPriority::High));

        let mut provider = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let results = queue.process_all(&mut provider, 0);

        assert_eq!(results.len(), 3);
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn budget_remaining_reflects_queue_state() {
        let queue = LlmRequestQueue::new(5000);
        assert_eq!(queue.budget_remaining(0), 5000);
    }

    #[test]
    fn token_budget_zero_max_blocks_everything() {
        let budget = TokenBudget::new(0);
        assert!(!budget.can_afford(1, 0));
        assert_eq!(budget.remaining(0), 0);
    }

    #[test]
    fn token_budget_consume_exactly_max() {
        let mut budget = TokenBudget::new(500);
        budget.consume(500, 100);
        assert_eq!(budget.remaining(100), 0);
        assert!(!budget.can_afford(1, 100));
    }

    #[test]
    fn token_budget_can_afford_zero_tokens() {
        let budget = TokenBudget::new(1000);
        assert!(budget.can_afford(0, 0));
    }

    #[test]
    fn token_budget_saturating_remaining() {
        let mut budget = TokenBudget::new(100);
        budget.consume(200, 0); // consume more than max
        assert_eq!(budget.remaining(0), 0); // should saturate at 0
    }

    #[test]
    fn queue_process_all_returns_highest_priority_first() {
        let mut queue = LlmRequestQueue::new(10000);
        queue.enqueue(make_request("Low", LlmPriority::Low));
        queue.enqueue(make_request("High", LlmPriority::High));
        queue.enqueue(make_request("Medium", LlmPriority::Medium));

        let mut provider = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let results = queue.process_all(&mut provider, 0);

        assert_eq!(results[0].0.character_name, "High");
        assert_eq!(results[1].0.character_name, "Medium");
        assert_eq!(results[2].0.character_name, "Low");
    }

    #[test]
    fn queue_enqueue_increments_pending() {
        let mut queue = LlmRequestQueue::new(10000);
        for i in 0..5 {
            queue.enqueue(make_request(&format!("Char{}", i), LlmPriority::Low));
        }
        assert_eq!(queue.pending_count(), 5);
    }

    #[test]
    fn queue_budget_remaining_after_process() {
        let mut queue = LlmRequestQueue::new(10000);
        queue.enqueue(make_request("Test", LlmPriority::Low));
        let mut provider = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        queue.process_next(&mut provider, 0);
        // Fallback uses 0 tokens
        assert_eq!(queue.budget_remaining(0), 10000);
    }

    #[test]
    fn pop_next_decrements_pending() {
        let mut queue = LlmRequestQueue::new(10000);
        queue.enqueue(make_request("A", LlmPriority::Low));
        queue.enqueue(make_request("B", LlmPriority::High));
        assert_eq!(queue.pending_count(), 2);
        queue.pop_next(0);
        assert_eq!(queue.pending_count(), 1);
        queue.pop_next(0);
        assert_eq!(queue.pending_count(), 0);
        queue.pop_next(0); // no-op
        assert_eq!(queue.pending_count(), 0);
    }
}
