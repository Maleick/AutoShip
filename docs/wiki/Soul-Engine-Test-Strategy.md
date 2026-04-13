# Soul Engine M11 — Test Strategy & Unit Test Specifications

**Status**: Test strategy documented  
**Coverage Target**: >80% coverage for all slices  
**Test Framework**: `cargo test`

---

## Test Organization

All tests follow this structure:

```
textquest/src/soul/
  ├── mod.rs (module unit tests)
  ├── coordinator.rs (coordinator tests)
  ├── personality.rs (personality tests)
  ├── memory.rs (memory tests)
  ├── social.rs (social tests)
  ├── idle.rs (idle tests)
  ├── llm/
  │   ├── mod.rs (llm tests)
  │   ├── fallback.rs (fallback tests)
  │   └── priority_queue.rs (queue tests)
  └── sentiment.rs (NEW — sentiment tests)

textquest-common/src/
  └── soul.rs (type tests)
```

---

## Slice-by-Slice Test Specifications

### Slice #1018 — Memory Decay Core

**Unit Tests** (in `memory.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decay_old_memories_marks_flag() {
        // Setup: Create memory older than threshold
        // Action: Call decay_old_memories(30)
        // Assert: decayed flag = 1
    }

    #[test]
    fn test_decay_respects_threshold() {
        // Setup: Mix of 10-day, 30-day, 50-day memories
        // Action: decay_old_memories(30)
        // Assert: Only 50-day marked; 10/30 day untouched
    }

    #[test]
    fn test_decay_weight_reduction() {
        // Setup: Create decayed memory
        // Action: Get context, check weight
        // Assert: weight = 0.5 (50% reduction)
    }

    #[test]
    fn test_decay_returns_count() {
        // Setup: 5 old, 3 new memories
        // Action: Call decay_old_memories(30)
        // Assert: Returns 5 (count of decayed)
    }

    #[test]
    fn test_decay_empty_result() {
        // Setup: All memories < 30 days old
        // Action: decay_old_memories(30)
        // Assert: Returns 0, no database changes
    }

    #[test]
    fn test_decay_config_override() {
        // Setup: Config with decay_days = 14
        // Action: decay_old_memories(14) with custom config
        // Assert: Memories > 14 days marked
    }

    #[test]
    fn test_decay_database_integrity() {
        // Setup: 1000 memories
        // Action: decay_old_memories(30)
        // Assert: Non-decayed memories untouched, no data loss
    }
}
```

**Integration Tests** (in `tests/soul_memory.rs`):

```rust
#[test]
fn test_memory_decay_end_to_end() {
    // Setup: Create character with 10 memories spanning 60 days
    // Action: Call coordinator tick with decay enabled
    // Assert: Old memories decayed, context uses new ones
}

#[test]
fn test_decay_performance() {
    // Setup: 10,000 memories
    // Action: Call decay_old_memories()
    // Assert: Completes in < 10ms
}
```

---

### Slice #1023 — Memory Summarization

**Unit Tests** (in `memory.rs`):

```rust
#[test]
fn test_generate_summary_basic() {
    // Setup: 5 memories in 1-hour window
    // Action: generate_summary()
    // Assert: Returns compact string under 500 chars
}

#[test]
fn test_generate_summary_format() {
    // Setup: Memories with zones and events
    // Action: generate_summary()
    // Assert: Format is "[zone] event: description" per line
}

#[test]
fn test_summary_includes_mood_trend() {
    // Setup: 10 memories, 7 Excited, 3 Happy
    // Action: generate_summary()
    // Assert: mood_trend = "Excited"
}

#[test]
fn test_summary_empty_window() {
    // Setup: Empty memory window
    // Action: generate_summary()
    // Assert: Returns empty summary or null
}

#[test]
fn test_summary_character_limit() {
    // Setup: 100 memories (would exceed limit)
    // Action: generate_summary()
    // Assert: Summary truncated to 500 chars max
}

#[test]
fn test_periodic_trigger() {
    // Setup: Coordinator with summary_interval_hours = 1
    // Action: Run 10 ticks (50 seconds each)
    // Assert: No summary generated yet
    // Action: Run 72 more ticks (1 hour total)
    // Assert: Summary generated and stored
}

#[test]
fn test_summary_deduplication() {
    // Setup: Generate summary twice in same interval
    // Action: generate_summary() twice
    // Assert: Only one entry in memory_summaries
}
```

---

### Slice #1030 — Memory Context Prioritization

**Unit Tests** (in `memory.rs`):

```rust
#[test]
fn test_recency_scoring() {
    // Setup: Memory from 5, 10, 20, 30 days ago
    // Action: Calculate recency scores
    // Assert: Scores decay: 0.83, 0.67, 0.33, 0.0
}

#[test]
fn test_combined_score() {
    // Setup: Memory with importance=0.8, recency=0.5
    // Action: Calculate combined score
    // Assert: Score = 0.4 (importance * recency)
}

#[test]
fn test_context_sorting() {
    // Setup: 10 memories with varied importance/recency
    // Action: get_llm_context()
    // Assert: Sorted by combined score descending
}

#[test]
fn test_context_limit() {
    // Setup: 100 memories, max_memories=20
    // Action: get_llm_context(20)
    // Assert: Returns exactly 20 entries
}

#[test]
fn test_context_formatting() {
    // Setup: Example memories
    // Action: get_llm_context()
    // Assert: Format is "[HH:MM] zone: event" or "[2h ago] zone: event"
}

#[test]
fn test_category_prioritization() {
    // Setup: Death, Kill, Chat events
    // Action: get_llm_context()
    // Assert: Death/Kill prioritized over Chat
}

#[test]
fn test_context_with_summaries() {
    // Setup: Summaries available
    // Action: get_llm_context()
    // Assert: Summaries used when available (more compact)
}

#[test]
fn test_token_window_fit() {
    // Setup: get_llm_context() with memory limit
    // Action: Count tokens in returned context
    // Assert: Fits in 2000 token window
}
```

---

### Slice #1037 — Sentiment Scorer

**Unit Tests** (in `sentiment.rs`):

```rust
#[test]
fn test_positive_sentiment() {
    // Action: score_sentiment("nice kill!")
    // Assert: > 0.5
}

#[test]
fn test_negative_sentiment() {
    // Action: score_sentiment("you suck")
    // Assert: < -0.5
}

#[test]
fn test_neutral_sentiment() {
    // Action: score_sentiment("ok sure")
    // Assert: -0.2 < score < 0.2
}

#[test]
fn test_case_insensitive() {
    // Action: score_sentiment("NICE") vs score_sentiment("nice")
    // Assert: Equal scores
}

#[test]
fn test_multiple_keywords() {
    // Action: score_sentiment("great job awesome")
    // Assert: Accumulation: +0.3 + 0.3 + 0.3 = 0.9
}

#[test]
fn test_sentiment_bounds() {
    // Action: score_sentiment("nice nice nice...") (100x)
    // Assert: Score clamped to 1.0 max
}

#[test]
fn test_eq_patterns() {
    // Action: score_sentiment("inc!") for incoming
    // Assert: +0.2 detected
}

#[test]
fn test_empty_string() {
    // Action: score_sentiment("")
    // Assert: 0.0
}

#[test]
fn test_performance() {
    // Setup: 1000 random messages
    // Action: score_sentiment() for each
    // Assert: Total < 1ms
}
```

---

### Slice #1044 — Sentiment Integration

**Unit Tests** (in `coordinator.rs`):

```rust
#[test]
fn test_sentiment_recorded() {
    // Setup: PlayerChat event with message
    // Action: process_soul_event()
    // Assert: Sentiment stored in conversations table
}

#[test]
fn test_mood_shift_positive() {
    // Setup: Character in Neutral mood
    // Action: Sentiment score 0.7
    // Assert: Mood shifts toward Happy/Excited
}

#[test]
fn test_mood_shift_negative() {
    // Setup: Character in Neutral mood
    // Action: Sentiment score -0.7
    // Assert: Mood shifts toward Angry/Anxious
}

#[test]
fn test_mood_no_shift_neutral() {
    // Setup: Character in Neutral mood
    // Action: Sentiment score 0.2
    // Assert: Mood unchanged
}

#[test]
fn test_multiple_events_accumulate() {
    // Setup: 5 weak positive events (0.4 each)
    // Action: Process all 5
    // Assert: Mood flips to Happy
}
```

---

### Slice #1059 — Mood Decay

**Unit Tests** (in `personality.rs`):

```rust
#[test]
fn test_mood_decay_excited() {
    // Setup: Character in Excited mood
    // Action: decay_mood() for 100 ticks (500s)
    // Assert: Mood shifts toward Neutral
}

#[test]
fn test_mood_decay_rates() {
    // Setup: Various moods
    // Action: decay_mood() for 30 ticks
    // Assert: Excited decays faster than Melancholy
}

#[test]
fn test_mood_stability_minimum() {
    // Setup: Excited mood with 60-tick stability
    // Action: decay_mood() for 30 ticks
    // Assert: Mood unchanged (still Excited)
    // Action: decay_mood() for 40+ ticks
    // Assert: Mood decays (past stability window)
}

#[test]
fn test_trait_modifiers() {
    // Setup: High conscientiousness character
    // Action: decay_mood() on same event sequence
    // Assert: Decays slower than baseline
}

#[test]
fn test_mood_never_goes_negative() {
    // Setup: Extreme decay for 1000 ticks
    // Action: decay_mood()
    // Assert: Final mood = Neutral, not undefined
}

#[test]
fn test_neutral_mood_stable() {
    // Setup: Neutral mood
    // Action: decay_mood() for 100 ticks
    // Assert: Mood remains Neutral
}
```

---

### Slice #1068 — Rate Limiting

**Unit Tests** (in `coordinator.rs`):

```rust
#[test]
fn test_per_character_limit() {
    // Setup: 5 requests per character per min
    // Action: Queue 7 requests for same character in 1 min
    // Assert: First 5 queued, next 2 use fallback
}

#[test]
fn test_global_limit() {
    // Setup: 20 requests per minute global
    // Action: 3 characters × 8 requests each (24 total)
    // Assert: First 20 queued, last 4 use fallback
}

#[test]
fn test_high_priority_not_dropped() {
    // Setup: Over limits with High priority request
    // Action: Queue High priority request
    // Assert: Uses fallback, not dropped
}

#[test]
fn test_low_priority_dropped() {
    // Setup: Over limits with Low priority request
    // Action: Queue Low priority request
    // Assert: Dropped silently
}

#[test]
fn test_window_sliding() {
    // Setup: 5 requests at t=0
    // Action: Wait 30s, then queue 6 more requests
    // Assert: At t=30, old requests aging out, new batch fits
}

#[test]
fn test_configuration_honored() {
    // Setup: Config with custom limits
    // Action: Set per_char=3, global=10
    // Assert: Limits enforced per config
}

#[test]
fn test_queue_cleanup() {
    // Setup: Large queue with 1000+ entries
    // Action: Check memory after pruning
    // Assert: Old entries removed, no memory leak
}
```

---

### Slice #1076 — Gossip Handler

**Unit Tests** (in `social.rs`):

```rust
#[test]
fn test_gossip_faction_updates() {
    // Setup: A gossips to B about C, tone=0.5
    // Action: apply_gossip(A, B, C, 0.5)
    // Assert: A→B +faction, A→C -faction, B→C affected
}

#[test]
fn test_gossip_three_party() {
    // Setup: No prior relationship between B and C
    // Action: apply_gossip(A to B about C, A dislikes C)
    // Assert: B influenced toward disliking C
}

#[test]
fn test_gossip_memory_creation() {
    // Setup: Gossip event
    // Action: apply_gossip()
    // Assert: Witnessed event created for B
    // Assert: shared_reference created for A, B, C
}

#[test]
fn test_gossip_positive_tone() {
    // Setup: A praises C to B (tone=1.0)
    // Action: apply_gossip(A, B, C, 1.0)
    // Assert: A→C +faction larger, B→C +faction
}

#[test]
fn test_gossip_negative_tone() {
    // Setup: A insults C to B (tone=-1.0)
    // Action: apply_gossip(A, B, C, -1.0)
    // Assert: A→C -faction large, B→C -faction
}

#[test]
fn test_faction_clamping() {
    // Setup: B already at 750 faction with C
    // Action: apply_gossip with additional +100
    // Assert: B→C clamped to 1000 max
}
```

---

### Slice #1084 — Personality Drift

**Unit Tests** (in `personality.rs`):

```rust
#[test]
fn test_drift_death_increases_neuroticism() {
    // Setup: Character with neuroticism=0.5
    // Action: apply_trait_drift(Death event)
    // Assert: neuroticism += 0.01, clamped to [0, 1]
}

#[test]
fn test_drift_loot_increases_greed() {
    // Setup: Character with greed=0.5
    // Action: apply_trait_drift(Loot event)
    // Assert: greed += 0.01
}

#[test]
fn test_drift_zone_exploration() {
    // Setup: New zone entered
    // Action: apply_trait_drift(ZoneEnter event)
    // Assert: openness += 0.01, wanderlust += 0.01
}

#[test]
fn test_drift_magnitude_limit() {
    // Setup: Character with conscientiousness=0.5
    // Action: 100 Loot events in single session
    // Assert: greed drifts but max +0.15 per session
}

#[test]
fn test_drift_persistence() {
    // Setup: Character drifts during session
    // Action: Save traits to DB at checkpoint
    // Action: Reload character
    // Assert: Traits match saved values
}

#[test]
fn test_drift_min_max_bounds() {
    // Setup: Character at trait=0.95
    // Action: apply_trait_drift(+0.1)
    // Assert: Clamped to 1.0, not exceeded
}

#[test]
fn test_drift_zero_trait() {
    // Setup: Trait at 0.0
    // Action: apply_trait_drift(+0.01)
    // Assert: Becomes 0.01 (not negative)
}
```

---

### Slice #1092 — Zone Awareness

**Unit Tests** (in `idle.rs`):

```rust
#[test]
fn test_zone_metadata_loading() {
    // Setup: config/soul_zones.toml exists
    // Action: Load zone metadata
    // Assert: All zones loaded correctly
}

#[test]
fn test_fish_only_has_water() {
    // Setup: Character in zone with has_water=false
    // Action: Calculate behavior weights
    // Assert: Fish weight = 0
}

#[test]
fn test_vendor_browse_needs_vendors() {
    // Setup: Character in zone with has_vendors=true
    // Action: Calculate behavior weights
    // Assert: VendorBrowse weight > 0
}

#[test]
fn test_wander_weight_safe_zone() {
    // Setup: Zone with safe=true
    // Action: Calculate weights for Wander
    // Assert: Weight is high
}

#[test]
fn test_wander_weight_dangerous_zone() {
    // Setup: Zone with safe=false
    // Action: Calculate weights for Wander
    // Assert: Weight is low
}

#[test]
fn test_anxious_mood_in_dangerous() {
    // Setup: Anxious mood in unsafe zone
    // Action: Calculate weights
    // Assert: Sit weight increased, Wander decreased
}

#[test]
fn test_zone_fallback_defaults() {
    // Setup: Unknown zone
    // Action: Look up metadata
    // Assert: Returns sensible defaults (neutral)
}
```

---

## Integration Test Template

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_soul_coordinator_tick() {
        // Setup: Create coordinator with config
        // Register character with traits
        // Add memories and relationships
        
        // Action: Call tick() with game state
        
        // Assert:
        // - No panics
        // - Memory updated correctly
        // - Mood changed or stayed consistent
        // - IPC commands generated if needed
        // - No database corruption
    }

    #[test]
    fn test_full_flow_sentiment_to_action() {
        // Setup: Character, game state with player chat
        // Action: Full flow: sentiment → mood → action
        // Assert: SoulAction emitted with correct message
    }

    #[test]
    fn test_orchestrator_integration() {
        // Setup: Mock orchestrator + soul coordinator
        // Action: Simulate combat kill event
        // Assert: Soul reacts, emits IPC command
        // Assert: No orchestrator tick delays
    }
}
```

---

## Test Coverage Goals

| Module | Target Coverage | Current |
|--------|-----------------|---------|
| `coordinator.rs` | >85% | TBD |
| `memory.rs` | >85% | TBD |
| `personality.rs` | >85% | TBD |
| `social.rs` | >80% | TBD |
| `idle.rs` | >80% | TBD |
| `llm/mod.rs` | >90% | TBD |
| `sentiment.rs` | >85% | TBD (new) |

## Running Tests

```bash
# All tests
cargo test -p textquest

# Soul-specific tests
cargo test -p textquest soul::

# With coverage
cargo tarpaulin -p textquest --out Html

# Performance tests (with --release)
cargo test --release -p textquest -- --nocapture
```

---

## Test Checklist per Slice

Before marking a slice complete:

- [ ] All unit tests passing
- [ ] Integration tests passing
- [ ] Code coverage >80% for slice module
- [ ] No clippy warnings
- [ ] Documentation tests passing
- [ ] Benchmark for performance-critical code
- [ ] Edge cases tested (empty, max, boundary values)
- [ ] Error handling tested
- [ ] Thread safety validated (if applicable)

---

## Continuous Integration

Add to `.github/workflows/`:

```yaml
name: Soul Engine Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Test Soul Engine
        run: cargo test -p textquest soul::
      - name: Coverage
        run: cargo tarpaulin -p textquest -o Xml
      - name: Upload Coverage
        uses: codecov/codecov-action@v4
```

---

## Conclusion

This test strategy ensures:
- ✅ **Comprehensive coverage** (>80% per module)
- ✅ **Unit + integration tests** (small + large scope)
- ✅ **Performance validation** (no regressions)
- ✅ **Edge case handling** (robustness)
- ✅ **CI integration** (automated checks)

All slice issues can now be implemented with confidence that tests validate functionality.
