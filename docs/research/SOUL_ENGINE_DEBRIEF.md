# Soul Engine Session Debrief Integration

## Overview

The Soul Engine receives session debrief data on session close, allowing character personalities to reference real session outcomes in their dialogue. This enables narrative that feels responsive to actual gameplay ("you died 4 times in PoFire — want me to flag the spot?").

## Architecture

### Session Debrief Flow

```
Session Close
    ↓
Generate SessionDebrief { wins, losses, suggestions, metrics }
    ↓
POST /api/soul/debrief
    ↓
SoulAuditState.debrief_by_character[character_id] ← debrief
    ↓
Personality System
    ↓
Next character interaction references debrief
```

### Data Types

```rust
pub struct SessionDebrief {
    pub session_id: String,
    pub character_id: u64,
    pub wins: Vec<String>,          // "cleared Lower Guk"
    pub losses: Vec<String>,        // "died at zone line"
    pub suggestions: Vec<String>,   // "flag dangerous zones"
    pub duration_secs: u64,
    pub damage_dealt: u64,
    pub damage_taken: u64,
    pub mobs_defeated: u32,
    pub deaths: u32,
    pub created_at: String,
}
```

## Constraints: "No Numeric Authority"

Soul Engine is **never** the source of numeric truth. Personalities can **describe** metrics, but must follow strict rules:

### 1. Read-Only Metrics

All numeric values come from the aggregation engine. Soul Engine never authors numbers:

```rust
// ✗ FORBIDDEN: LLM-authored numeric claim
"You killed around 50 mobs today."

// ✓ CORRECT: Exact value from debrief
"You killed {mobs_defeated} mobs."
```

### 2. Exact Value Mapping

When a personality references a metric, it must read from the debrief structure. No paraphrasing or estimation:

```rust
// ✗ FORBIDDEN: Estimated/paraphrased value
"You took a lot of damage today."

// ✓ CORRECT: Templated placeholder
"You took {damage_taken} damage."
```

### 3. No Config Authority

Soul Engine **cannot write to character configuration**:

```rust
// ✗ FORBIDDEN: Soul Engine modifying config
trait_engine.set_trait(character_id, "cautious", 0.9)?;
config.edginess_level = EdginessLevel::Aggressive;

// ✓ CORRECT: Soul Engine suggests, operator decides
personality.suggest_review("/improve");
```

### 4. One-Way Flow (No Feedback Loop)

Debrief data flows into Soul Engine, but Soul Engine output does not feed back into suggestions:

```
Aggregation Engine → SessionDebrief → Soul Engine Personalities
                                          ↓
                                      Dialogue & Suggestions
                                          ↓
                                        (logged, not fed back)
```

### 5. Templated Prompts

Soul Engine prompts use placeholders instead of free-form values:

```
Instead of:
"You defeated approximately {estimated_mobs} enemies."

Use:
"You defeated {mobs_defeated} enemies."

Where {mobs_defeated} is substituted from debrief before calling the LLM.
```

## API Endpoints

### POST /api/soul/debrief

Receive session debrief on session close.

```json
{
  "session_id": "s2026-04-25-001",
  "character_id": 42,
  "wins": ["cleared Lower Guk", "obtained rare loot"],
  "losses": ["died at zone line"],
  "suggestions": ["flag dangerous zones", "improve melee timing"],
  "duration_secs": 3600,
  "damage_dealt": 50000,
  "damage_taken": 12000,
  "mobs_defeated": 47,
  "deaths": 2,
  "created_at": "2026-04-25T10:00:00Z"
}
```

Response:

```json
{
  "status": "debrief_received"
}
```

### GET /api/soul/debrief/{character_id}

Retrieve latest session debrief for a character.

Response:

```json
{
  "session_id": "s2026-04-25-001",
  "character_id": 42,
  ...
}
```

## Personality Integration

Personalities can reference debrief during conversation:

```rust
// In personality prompt template:
let prompt = format!(
    "This session: {} mobs defeated, {} deaths, {} damage taken. \
     Wins: {}. Losses: {}. Suggestions: {}.\n\
     Provide personalized feedback.",
    debrief.mobs_defeated,
    debrief.deaths,
    debrief.damage_taken,
    debrief.wins.join(", "),
    debrief.losses.join(", "),
    debrief.suggestions.join(", "),
);
```

## Testing & Verification

### Unit Tests

1. **SessionDebrief numeric preservation**: Verify serialization preserves exact values
2. **Endpoint response**: Verify POST stores debrief, GET retrieves it
3. **Character isolation**: Verify debriefs are stored per-character, not mixed

### Integration Tests

1. **Config write rejection**: Verify Soul Engine cannot write config (would fail at handler level)
2. **Numeric value accuracy**: Verify debrief values match aggregation output
3. **Personality reference**: Verify personalities retrieve and reference debrief in output

### Manual Verification

1. Check conversation logs: Do personality responses reference exact session metrics?
2. Check audit trail: Is debrief logged but not fed back into suggestions?
3. Check config: Did personality suggestions never modify character config?

## Implementation Notes

- `SessionDebrief` is stored in-memory in `SoulAuditState.debrief_by_character`.
- For persistence, implement optional SQLite migration (future work).
- Soul Engine has **no write capability** to config — enforced at the handler layer.
- Numeric values in debrief are immutable once created by aggregation engine.

## Example: Character Banter

```
Player closes session after session with 47 mobs defeated, 2 deaths, 12k damage taken.

SessionDebrief sent to Soul Engine.

Next character interaction:

  Frostreaver: "Back so soon? I heard you had trouble in PoFire.
               You took 12,000 damage and died twice. That zone-line
               is a killer—want me to flag those dangerous spots?"
```

This dialogue is:

- ✓ Responsive to real session data
- ✓ References exact numeric values from debrief
- ✓ Suggests operator review (doesn't modify config)
- ✓ Uses debrief as context, not feedback source
