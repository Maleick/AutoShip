/// Performance regression tests for the Soul Engine.
///
/// These tests measure wall-clock time for core Soul operations and assert
/// that they complete within tight latency budgets (with a 2x safety margin).
///
/// Budgets:
/// - `SoulCoordinator::tick()` for 36 characters: < 10ms (budget 20ms with 2x margin)
/// - `MemoryStore::recall_recent()` for 100 memories: < 5ms (budget 10ms with 2x margin)
///
/// Run with:
///   cargo test -p textquest soul::perf_tests -- --nocapture
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Instant;

    use textquest_common::combat::CombatStatus;
    use textquest_common::nav::NavStatus;
    use textquest_common::soul::{MoodState, PersonalityTraits, SoulEvent, SpeechStyle};
    use textquest_common::types::{ClientId, GameState, SpawnData};

    use crate::soul::config::{CharacterSoulConfig, SoulConfig};
    use crate::soul::coordinator::SoulCoordinator;
    use crate::soul::memory::MemoryStore;

    // ---------------------------------------------------------------------------
    // Latency budgets (milliseconds) — 2x safety margin applied here so tests
    // remain green even under mild CI load.
    // ---------------------------------------------------------------------------
    const TICK_36_CHARS_BUDGET_MS: u128 = 20; // 10ms target * 2x margin
    const RECALL_100_MEMORIES_BUDGET_MS: u128 = 10; // 5ms target * 2x margin

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn make_soul_config(enabled: bool) -> SoulConfig {
        SoulConfig {
            enabled,
            ..SoulConfig::default()
        }
    }

    fn make_char_config(name: &str) -> CharacterSoulConfig {
        CharacterSoulConfig {
            name: name.to_string(),
            traits: PersonalityTraits::default(),
            speech: SpeechStyle::default(),
            edginess: None,
            backstory: String::new(),
            quirks: Vec::new(),
        }
    }

    fn make_game_state(client_id: ClientId) -> GameState {
        GameState {
            client_id,
            local_player: Some(SpawnData {
                displayed_name: format!("Char{client_id}"),
                name: format!("Char{client_id}"),
                level: 60,
                ..SpawnData::default()
            }),
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "eastcommons".into(),
            zone_long_name: "East Commons".into(),
            actual_version: None,
        }
    }

    /// Build an in-memory `MemoryStore` via the test-only constructor.
    fn open_memory_store() -> MemoryStore {
        MemoryStore::open_in_memory()
    }

    /// Simulate building an LLM context window: fetch the N most recent memories
    /// for a character, which is the hot path before every LLM call.
    fn get_llm_context(store: &MemoryStore, character_id: ClientId, limit: usize) -> usize {
        store
            .recall_recent(character_id, limit)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    // ---------------------------------------------------------------------------
    // Benchmark helpers (called from the four timing tests below)
    // ---------------------------------------------------------------------------

    /// Time how long a single `SoulCoordinator::tick()` call takes for `n` registered
    /// characters, each with a matching `GameState`.
    fn measure_tick_duration_ms(n_characters: usize) -> u128 {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("perf_tick.db");
        let mut coord = SoulCoordinator::new(make_soul_config(true), &db_path).unwrap();

        let mut states: HashMap<ClientId, GameState> = HashMap::new();
        for i in 0..n_characters {
            let cid = (i + 1) as ClientId;
            coord.register_character(cid, &make_char_config(&format!("Char{cid}")));
            states.insert(cid, make_game_state(cid));
        }

        // Warm-up pass (not measured)
        let _ = coord.tick(&states);

        let t0 = Instant::now();
        let _ = coord.tick(&states);
        t0.elapsed().as_millis()
    }

    /// Time how long `recall_recent()` takes after 100 memories have been recorded.
    fn measure_recall_duration_ms(n_memories: usize) -> u128 {
        let store = open_memory_store();
        let character_id: ClientId = 1;

        // Pre-populate the store
        for i in 0..n_memories {
            let event = SoulEvent::Kill {
                target: format!("mob_{i}"),
                zone: "eastcommons".into(),
            };
            let _ = store.record(character_id, &event, MoodState::Neutral, 1.0);
        }

        // Warm-up pass
        let _ = get_llm_context(&store, character_id, n_memories);

        let t0 = Instant::now();
        let count = get_llm_context(&store, character_id, n_memories);
        let elapsed_ms = t0.elapsed().as_millis();

        assert_eq!(
            count, n_memories,
            "Expected all {n_memories} memories to be recalled"
        );
        elapsed_ms
    }

    // ---------------------------------------------------------------------------
    // Performance tests
    // ---------------------------------------------------------------------------

    /// `tick_duration_ok`: SoulCoordinator::tick() for 36 characters must complete
    /// within 10ms (asserted at 2x budget = 20ms).
    #[test]
    fn tick_duration_ok() {
        let elapsed_ms = measure_tick_duration_ms(36);
        println!("[perf] tick(36 chars) = {elapsed_ms}ms (budget: 10ms, 2x limit: 20ms)");
        assert!(
            elapsed_ms <= TICK_36_CHARS_BUDGET_MS,
            "tick(36 chars) took {elapsed_ms}ms — exceeded 2x budget of {TICK_36_CHARS_BUDGET_MS}ms"
        );
    }

    /// `memory_recall_ok`: recall_recent() for 100 memories must complete within
    /// 5ms (asserted at 2x budget = 10ms).
    #[test]
    fn memory_recall_ok() {
        let elapsed_ms = measure_recall_duration_ms(100);
        println!(
            "[perf] recall_recent(100 memories) = {elapsed_ms}ms (budget: 5ms, 2x limit: 10ms)"
        );
        assert!(
            elapsed_ms <= RECALL_100_MEMORIES_BUDGET_MS,
            "recall_recent(100) took {elapsed_ms}ms — exceeded 2x budget of {RECALL_100_MEMORIES_BUDGET_MS}ms"
        );
    }

    /// `tick_duration_scales_linearly`: measures tick at 1, 9, 18, and 36 characters
    /// and prints the scaling profile. Asserts each point is within the 20ms budget.
    #[test]
    fn tick_duration_scales_linearly() {
        for &n in &[1usize, 9, 18, 36] {
            let elapsed_ms = measure_tick_duration_ms(n);
            println!("[perf] tick({n} chars) = {elapsed_ms}ms");
            assert!(
                elapsed_ms <= TICK_36_CHARS_BUDGET_MS,
                "tick({n} chars) took {elapsed_ms}ms — exceeded 2x budget of {TICK_36_CHARS_BUDGET_MS}ms"
            );
        }
    }

    /// `memory_recall_scales_with_count`: measures recall at 10, 50, and 100 memories
    /// and prints the scaling profile. Asserts each point is within the 10ms budget.
    #[test]
    fn memory_recall_scales_with_count() {
        for &n in &[10usize, 50, 100] {
            let elapsed_ms = measure_recall_duration_ms(n);
            println!("[perf] recall_recent({n} memories) = {elapsed_ms}ms");
            assert!(
                elapsed_ms <= RECALL_100_MEMORIES_BUDGET_MS,
                "recall_recent({n}) took {elapsed_ms}ms — exceeded 2x budget of {RECALL_100_MEMORIES_BUDGET_MS}ms"
            );
        }
    }
}
