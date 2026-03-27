use std::collections::BinaryHeap;
use std::cmp::Ordering;

use super::{LlmRequest, LlmResponse, LlmProvider};
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
    pub fn new(max_tokens_per_hour: u32) -> Self {
        Self {
            max_tokens_per_hour,
            tokens_used: 0,
            window_start_secs: 0,
        }
    }

    /// Check if we have budget for the estimated token count.
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

/// Wrapper to make LlmRequest orderable by priority for the BinaryHeap.
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

    /// Number of pending requests.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Tokens remaining in the current budget window.
    pub fn budget_remaining(&self, now_secs: u64) -> u32 {
        self.budget.remaining(now_secs)
    }
}
