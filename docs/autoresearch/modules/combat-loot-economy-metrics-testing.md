# Combat / Loot / Economy / Metrics / Testing Modules

Files: `combat/{camp_loop,ch_chain,coordinator,events,heal_coordinator,mod,spell_db,spell_optimizer}.rs`, `loot/{distributor,ledger,mod,ownership,queue,store,wishlist}.rs`, `economy/{failure_handling,mod,plat_tracker,price_monitor}.rs`, `metrics/{events,kill_reporter,kill_session_store,kill_tracker,mod,store,xp_tracker}.rs`, `testing/{metrics,mod,output,scenario}.rs`.

These are the five "business logic" layers on top of raw game state.

---

## `combat/` — fight orchestration

### `combat::coordinator`

- **Purpose.** Top-level per-group combat orchestrator. Drives the assist chain, CC assignments, and CH rotations.
- **Public API.** `CombatCoordinator::tick`, plus CH chain management and CC assignment helpers.
- **Invariants.**
  - `tick` detects edges (alive ↔ dead, in-combat ↔ idle) and emits `SoulEvent::Kill` / `SoulEvent::Death` for survivors/casualties.
  - Any assist-target change is broadcast to all non-tank DPS.

### `combat::ch_chain`

- **Purpose.** Timed Complete Heal rotation FSM.
- **Public API.** `ChChain::tick` returns the firing cleric's PID each frame.
- **Invariants.**
  - Tracks frame count at ~20 fps; fires every `frames_per_interval = interval_secs × 20`.
  - Order rotates through members; `remove_member` lets callers skip dead clerics.
  - Adaptive mode adjusts interval based on tank HP delta.

### `combat::camp_loop`

- **Purpose.** Translate high-level `GameState` transitions into `CampEvent`s (PullIncoming, CombatStarted, etc.) for the `camp::state::CampLoop`.
- **Invariants.** State timeouts trigger recovery transitions; events are idempotent per edge.

### `combat::heal_coordinator`

- **Purpose.** Cross-group heal arbitration — prevents four clerics from dumping Complete Heal on the same tank.
- **Public API.** `HealCoordinator` with priority ordering + expiring claim locks. `CureCoordinator` does the same for cures.
- **Invariants.** Claim locks auto-expire so a stalled cleric never deadlocks the chain.

### `combat::spell_db` / `combat::spell_optimizer`

- **Purpose.** Spell metadata table + mana-efficiency ranking used by casters for rotation selection.

### `combat::events`

- **Purpose.** Shared combat event enum consumed by the Soul Engine.

---

## `loot/` — item pipeline

### `loot::queue`

- **Purpose.** FIFO buffer for freshly-dropped items.
- **Public API.** `LootQueue::push` returns `DroppedItem { drop_id, … }`.

### `loot::ownership`

- **Purpose.** State machine per item: `Available → Reserved → Assigned → Collected → Distributed` (or `Vendor`).
- **Public API.**
  - `OwnershipModel`, `LootCandidate`, `EntrySource`, `AuditLog`.
- **Invariants.**
  - Transitions form a directed graph; `AuditLog` appends `(drop_id, from, to, character, unix_ts)` on every edge.
  - `Available → Reserved` is a soft lock.
  - `Assigned → Collected` is the pickup confirmation.

### `loot::distributor`

- **Purpose.** Pure FSM coordinating `Reserve → Assign → Execute` with exponential backoff.
- **Public API.** `LootDistributor::enqueue`, tick-based callbacks for assign/execute.
- **Invariants.**
  - `MAX_RETRIES = 3`, `BACKOFF_BASE_MS = 100`.
  - `distributed_ids: HashSet` prevents re-queueing already-done items.
  - Permanent failures (e.g., "item already looted") do **not** re-queue; transient failures (`ClientOffline`) do.

### `loot::ledger` (via `loot::mod`)

- **Purpose.** Immutable append-only record of distributions + plat deltas.
- **Public API.** `LedgerEntry`, `EntrySource` (`Drop`, `Vendor`, `Bank`, `Distribution`).
- **Invariants.** Entries never mutate; replay reconstructs state.

### `loot::wishlist`

- **Purpose.** Per-character gear priorities + rule conditions.
- **Public API.** `WishlistManager`, `ReserveRule`.

### `loot::store`

- **Purpose.** SQLite persistence for items, TLP loot tables, drop rates, wishlists, history.

---

## `economy/` — plat + Krono tracking

### `EconomyLedger` (implemented in `loot::ledger`, re-exported via `loot::mod`)

- **Purpose.** Immutable plat ledger with daily trend reporting. There is **no** `economy::ledger` module — the type lives in `textquest/src/loot/ledger.rs` and is surfaced to economy-facing code through `loot::mod`'s re-exports.
- **Public API.** `EconomyLedger::trend_report` → per-day summary.
- **Invariants.** Append-only; used for camp profitability analysis.

### `economy::price_monitor`

- **Purpose.** Passive scraper that captures Krono-denominated trade chat, dedupes it, and persists observations.
- **Public API.** `TradePriceMonitor::record_chat`, `TradePriceStore`.
- **Invariants.**
  - Dedup window: `DEDUPE_WINDOW_MS = 5000` per `(speaker_pid, item_name)`.
  - Only hub zones (nexus, poknowledge, …) are recorded.
  - Non-Krono prices are rejected.

### `economy::plat_tracker`

- **Purpose.** `CoinStack` for plat/gold/silver/copper denomination math.

### `economy::failure_handling`

- **Purpose.** Detects ledger anomalies and routes them to recovery flows.
- **Public API.** `FailureRouter`, `FailureState`, `RecoveryAction`.

---

## `metrics/` — fleet-wide telemetry

### `metrics::kill_tracker`

- **Purpose.** Records every kill + computes per-mob / per-client / efficiency stats.
- **Public API.**
  - `KillRecord { mob, level, zone, killer_pid, kill_time_ms, total_damage, timestamp }` + `dps()`.
  - `ClientDpsStats` with `session_dps()`.
  - `MobStats`, `EfficiencyScore` (0–100 composite from kills/hr, avg kill time, death ratio).
- **Invariants.**
  - `killer_pid` = killing-blow client; `total_damage` is fleet-wide sum.
  - Each kill produces one `FleetEvent` and one metrics snapshot.

### `metrics::kill_reporter`

- **Purpose.** Format and publish kill summaries (Discord, logs).

### `metrics::kill_session_store`

- **Purpose.** SQLite-backed persistence + query API for kill history.

### `metrics::store`

- **Purpose.** Unified SQLite schema covering events, DPS snapshots, loot history, lockouts, plat ledger.

### `metrics::events`

- **Purpose.** `FleetEventLog` + `FleetEvent` structured event type.
- **Invariants.** Events are immutable, indexed by `(character, zone, timestamp)`.

### `metrics::xp_tracker`

- **Purpose.** XP-per-hour aggregation.

---

## `testing/` — scenario harness

### `testing::scenario`

- **Purpose.** Async scenario trait used by autonomous tests.
- **Public API.**
  - `trait TestScenario { async fn run(&mut self, duration) -> ScenarioResult; }`.
  - `ScenarioResult` (success flag, wall duration, structured metrics, errors).
- **Invariants.** Scenarios must honour the provided wall-clock budget and return structured results regardless of pass/fail.

### `testing::metrics`

- **Purpose.** `MetricsAggregator` — accumulate `Counter` / `Gauge` / `Histogram` samples during a run.
- **Invariants.** Percentiles are nearest-rank over sorted samples; JSON export includes `count/sum/avg` per key.

### `testing::output`

- **Purpose.** Manage test-session directories.
- **Public API.** `SessionManager`, `SessionMetadata`.
- **Invariants.**
  - Directory format `session-YYYY-MM-DD-HHMMSS-{short_id}/`.
  - `metadata.json` rewritten whenever `set_target_duration` / `set_config_hash` is called.

---

## Cross-module flow

1. **Combat → kill events.** `CombatCoordinator` ends combat → emits `SoulEvent::Kill` → soul coordinator forwards to `KillTracker`.
2. **Loot intake.** Drops hit `LootQueue::push` → `OwnershipModel` claims → `LootDistributor` schedules + assigns → execute callback pushes a slash command via IPC.
3. **Distribution → ledger.** A successful execution writes a `LedgerEntry { source: Distribution, plat_delta }` into `EconomyLedger`.
4. **Trade chat → price monitor.** Chat ingress → `TradePriceMonitor::record_chat` → dedup → persisted via `TradePriceStore`.
5. **Testing harness.** Scenarios instantiate `CombatCoordinator`, `LootDistributor`, `KillTracker` in-process; metrics collected in `MetricsAggregator` and written under `SessionManager`'s directory.
