# Clickies Module — API Design & Integration

Complete API specification for implementing the clickies module in TextQuest.

## Module Structure

```
textquest-client/
├── src/
│   └── clickies/
│       ├── mod.rs              # Module export
│       ├── module.rs           # ClickiesModule struct
│       ├── config.rs           # Configuration loading/validation
│       ├── evaluator.rs        # Condition evaluation
│       ├── cooldowns.rs        # Cooldown tracking
│       ├── executor.rs         # Item execution
│       └── tests.rs            # Unit tests
```

## Core Types

### ClickiesModule

```rust
/// Main clickies module for item automation
pub struct ClickiesModule {
    // State
    pub items: Vec<ClickieItem>,
    pub cooldowns: CooldownTracker,
    pub queue: VecDeque<QueuedItem>,
    pub stats: ClickiesStats,
    
    // Configuration
    pub config: ClickiesConfig,
    config_path: PathBuf,
    last_evaluation: Instant,
    
    // Runtime
    pub enabled: bool,
    current_phase: Phase,
}

impl ClickiesModule {
    /// Create a new clickies module
    pub fn new(config_path: PathBuf) -> Result<Self>;
    
    /// Load configuration from disk
    pub fn load_config(&mut self) -> Result<()>;
    
    /// Hot-reload configuration (idempotent)
    pub fn reload_config(&mut self) -> Result<()>;
    
    /// Evaluate all items and queue those ready
    /// Returns number of items queued
    pub fn evaluate(&mut self, state: &GameState) -> Result<usize>;
    
    /// Execute queued items (non-blocking)
    pub fn execute_next(&mut self, state: &mut GameState) -> Result<()>;
    
    /// Execute all queued items
    pub fn execute_all(&mut self, state: &mut GameState) -> Result<usize>;
    
    /// Manual item queue
    pub fn queue_item(&mut self, slot: usize) -> Result<()>;
    
    /// Get current queue depth
    pub fn queue_depth(&self) -> usize;
    
    /// Clear queue (emergency)
    pub fn clear_queue(&mut self);
    
    /// Get item metadata
    pub fn get_item(&self, slot: usize) -> Option<&ClickieItem>;
    
    /// Get cooldown status
    pub fn get_cooldown(&self, slot: usize) -> CooldownStatus;
    
    /// Enable/disable module
    pub fn set_enabled(&mut self, enabled: bool);
    
    /// Get statistics
    pub fn get_stats(&self) -> &ClickiesStats;
}
```

### ClickieItem

```rust
/// Single item in the clickies list
pub struct ClickieItem {
    pub slot: u8,                      // 1-36 inventory/equipment slot
    pub name: String,                  // Human-readable name
    pub phase: Phase,                  // combat, downtime, precomm
    pub priority: u8,                  // 1-10 (1 = highest)
    pub condition: ConditionExpr,      // Condition to use item
    pub cooldown_ms: u64,              // Min time between uses
    pub last_used: Option<Instant>,    // Last use timestamp
    pub use_limit: Option<u32>,        // Max uses per session
    pub use_count: u32,                // Current use count
    pub enabled: bool,                 // Can be disabled per-item
}

impl ClickieItem {
    /// Check if item is ready (not on cooldown)
    pub fn is_ready(&self) -> bool {
        self.last_used
            .map(|t| t.elapsed().as_millis() >= self.cooldown_ms)
            .unwrap_or(true)
    }
    
    /// Check if use count not exceeded
    pub fn can_use(&self) -> bool {
        self.use_limit
            .map(|limit| self.use_count < limit)
            .unwrap_or(true)
    }
    
    /// Record a use
    pub fn mark_used(&mut self) {
        self.last_used = Some(Instant::now());
        self.use_count += 1;
    }
    
    /// Reset use count (per-session)
    pub fn reset_session(&mut self) {
        self.use_count = 0;
    }
}
```

### Condition System

```rust
/// Condition expression for item use
pub struct ConditionExpr {
    raw: String,
    parsed: Option<Condition>,
}

pub enum Condition {
    // Stat-based conditions
    HpPct(Comparison),              // hp_pct < 50
    ManaPct(Comparison),             // mana_pct > 30
    EndurancePct(Comparison),        // endurance_pct < 40
    
    // Status conditions
    InCombat,                        // in_combat
    NotInCombat,                     // not_in_combat
    Buffed(String),                  // buffed(buff_name)
    NotBuffed(String),               // not_buffed(buff_name)
    
    // Item conditions
    ItemReady,                       // item_ready (not on cooldown)
    LastUsedSeconds(u64),            // last_used > 30s
    
    // Sustenance conditions
    Hunger(Comparison),              // hunger > 30
    Thirst(Comparison),              // thirst > 30
    
    // Composite
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
    Not(Box<Condition>),
}

pub enum Comparison {
    Less(u8),        // <
    LessEqual(u8),   // <=
    Equal(u8),       // ==
    GreaterEqual(u8),// >=
    Greater(u8),     // >
}

impl ConditionExpr {
    /// Parse condition string
    pub fn parse(raw: &str) -> Result<Self>;
    
    /// Evaluate condition against game state
    pub fn evaluate(&self, state: &GameState) -> bool;
}
```

### Cooldown Tracker

```rust
/// Tracks per-item cooldowns
pub struct CooldownTracker {
    cooldowns: HashMap<u8, Instant>,
}

impl CooldownTracker {
    pub fn new() -> Self;
    
    /// Check if item is ready
    pub fn is_ready(&self, slot: u8) -> bool;
    
    /// Get remaining cooldown in milliseconds
    pub fn remaining_ms(&self, slot: u8) -> u64;
    
    /// Set cooldown
    pub fn set(&mut self, slot: u8, duration_ms: u64);
    
    /// Clear cooldown
    pub fn clear(&mut self, slot: u8);
    
    /// Get all active cooldowns
    pub fn active(&self) -> Vec<(u8, u64)>;
}
```

### Queue & Execution

```rust
/// Item queued for execution
pub struct QueuedItem {
    pub slot: u8,
    pub name: String,
    pub queued_at: Instant,
    pub phase: Phase,
}

pub struct ClickiesStats {
    pub total_items: u32,           // Total configured items
    pub items_queued: u32,          // Items waiting to execute
    pub items_executed: u32,        // Items used this session
    pub evaluation_count: u32,      // Times evaluated
    pub execution_time_ms: u64,     // Total execution time
}
```

## Integration with GameState

Add to existing `GameState`:

```rust
pub struct GameState {
    // ... existing fields ...
    pub clickies: Option<ClickiesModule>,
}

pub struct CombatState {
    // ... existing fields ...
    pub hp_pct: u8,
    pub mana_pct: u8,
    pub endurance_pct: u8,
    pub hunger: u8,
    pub thirst: u8,
    pub in_combat: bool,
    pub buffs: Vec<BuffInfo>,
}

pub struct BuffInfo {
    pub name: String,
    pub duration: Duration,
}
```

## Configuration Loading

```rust
/// Configuration for clickies module
pub struct ClickiesConfig {
    pub enabled: bool,
    pub evaluation_frequency_ms: u64,
    pub max_queue_depth: usize,
    pub verbose_logging: bool,
    pub groups: Vec<ItemGroup>,
}

pub struct ItemGroup {
    pub name: String,
    pub phase: Phase,
    pub priority: u8,
    pub items: Vec<ItemConfig>,
}

pub struct ItemConfig {
    pub slot: u8,
    pub name: String,
    pub condition: String,  // Raw condition text
    pub cooldown_ms: u64,
    pub use_limit: Option<u32>,
}

impl ClickiesConfig {
    /// Load from TOML file
    pub fn from_file(path: &Path) -> Result<Self>;
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()>;
}
```

## Camp Loop Integration

Update camp loop to call clickies:

```rust
// In textquest-client/src/camp.rs

pub async fn run_camp_loop(state: Arc<Mutex<AppState>>) -> Result<()> {
    let mut last_eval = Instant::now();
    let mut last_clickies_eval = Instant::now();
    
    loop {
        let mut app_state = state.lock().await;
        
        // Existing logic
        handle_buffs(&mut app_state)?;
        handle_mana_recovery(&mut app_state)?;
        
        // NEW: Clickies evaluation (every 500ms)
        if last_clickies_eval.elapsed().as_millis() >= 500 {
            if let Some(clickies) = &mut app_state.clickies {
                // Update game state
                let game_state = GameState::from_app_state(&app_state);
                
                // Evaluate and queue
                match clickies.evaluate(&game_state) {
                    Ok(count) => {
                        if count > 0 {
                            tracing::debug!("Queued {} clickies", count);
                        }
                    }
                    Err(e) => tracing::warn!("Clickies eval error: {}", e),
                }
                
                // Execute next item
                if clickies.queue_depth() > 0 {
                    let mut game_state = GameState::from_app_state(&app_state);
                    if let Err(e) = clickies.execute_next(&mut game_state) {
                        tracing::warn!("Clickies exec error: {}", e);
                    }
                }
            }
            last_clickies_eval = Instant::now();
        }
        
        // Continue loop
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

## Test Fixtures

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    fn make_test_clickies() -> ClickiesModule {
        let config = ClickiesConfig {
            enabled: true,
            evaluation_frequency_ms: 500,
            max_queue_depth: 10,
            verbose_logging: false,
            groups: vec![],
        };
        
        ClickiesModule {
            items: vec![],
            cooldowns: CooldownTracker::new(),
            queue: VecDeque::new(),
            stats: ClickiesStats::default(),
            config,
            config_path: PathBuf::from("/tmp/clickies.toml"),
            last_evaluation: Instant::now(),
            enabled: true,
            current_phase: Phase::Combat,
        }
    }
    
    fn make_test_game_state() -> GameState {
        GameState {
            hp_pct: 100,
            mana_pct: 100,
            endurance_pct: 100,
            hunger: 0,
            thirst: 0,
            in_combat: false,
            buffs: vec![],
        }
    }
    
    #[test]
    fn test_condition_evaluation() {
        let condition = ConditionExpr::parse("hp_pct < 50").unwrap();
        let mut state = make_test_game_state();
        state.hp_pct = 40;
        assert!(condition.evaluate(&state));
    }
    
    #[test]
    fn test_cooldown_tracking() {
        let mut tracker = CooldownTracker::new();
        tracker.set(1, 5000);
        assert!(!tracker.is_ready(1));
        assert!(tracker.remaining_ms(1) <= 5000);
    }
    
    #[test]
    fn test_queue_management() {
        let mut clickies = make_test_clickies();
        clickies.queue_item(1).unwrap();
        assert_eq!(clickies.queue_depth(), 1);
        clickies.clear_queue();
        assert_eq!(clickies.queue_depth(), 0);
    }
}
```

## Error Handling

```rust
pub enum ClickiesError {
    ConfigNotFound(PathBuf),
    ConfigParseError(String),
    InvalidCondition(String),
    InventorySlotOutOfRange(u8),
    ItemNotAvailable(u8),
    ExecutionFailed(String),
}

impl std::fmt::Display for ClickiesError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ClickiesError::ConfigNotFound(p) => write!(f, "Config not found: {:?}", p),
            ClickiesError::ConfigParseError(e) => write!(f, "Config parse error: {}", e),
            ClickiesError::InvalidCondition(e) => write!(f, "Invalid condition: {}", e),
            ClickiesError::InventorySlotOutOfRange(s) => write!(f, "Slot out of range: {}", s),
            ClickiesError::ItemNotAvailable(s) => write!(f, "Item not available at slot: {}", s),
            ClickiesError::ExecutionFailed(e) => write!(f, "Execution failed: {}", e),
        }
    }
}

impl std::error::Error for ClickiesError {}
```

## Performance Considerations

### Evaluation (Every 500ms)
- Parse conditions once at config load
- Cache parsed condition tree
- Use bitflags for status checks
- Minimal allocations

### Execution (Every queue item)
- Single item click per iteration
- Avoid blocking on DLL calls
- Queue items for async execution

### Memory
- Pre-allocate item vector (typically 10-20 items)
- Reuse condition evaluator instance
- No per-frame allocations

## Extensibility

Future additions:
- `if_not_silenced` condition for casters
- `if_not_rooted` condition for melee
- Per-group enable/disable
- Lua scripting for custom conditions
- Web UI for configuration editing
- Statistics export (usage patterns)

## Related Code Patterns

Reference implementations in codebase:
- **Condition evaluation**: `textquest-dll/src/combat/state.rs` (ability conditions)
- **Cooldown tracking**: `textquest-dll/src/combat/ability_cooldowns.rs`
- **Configuration loading**: `textquest/src/config.rs`
- **Queue execution**: `textquest-client/src/ability_queue.rs`
