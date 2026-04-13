# Soul Engine — Comprehensive Definition

**Status**: Phase 1 (deterministic fallback with persistent memory); Phase 2 (local LLM integration) deferred to M11.

**Last updated**: 2026-04-12

## Overview

The Soul Engine provides personality, behavioral autonomy, and social dynamics for each boxed character. It is the layer that transforms a scripted automation client into a character with mood, memories, relationships, and flavor behavior.

The engine bridges between:
- **Orchestrator**: receives game state, sends chat/emote commands via IPC
- **Memory store**: persistent SQLite for memories, conversations, relationships
- **Configuration**: `config/frostreaver.toml` `[soul]` section + per-character overrides
- **Future LLM integration**: framework ready; provider integration in M11

## Subsystems

### 1. Personality Engine (`personality.rs`)

**Purpose**: Deterministic trait-based personality that drives responses to events.

**Traits**: Big Five + EQ-themed

- **Big Five** (0.0–1.0, all default to 0.5):
  - `openness`: curiosity, creativity, novelty-seeking
  - `conscientiousness`: discipline, organization, reliability
  - `extraversion`: sociability, talkativeness, group involvement
  - `agreeableness`: cooperativeness, empathy, conflict avoidance
  - `neuroticism`: emotional instability, anxiety, reactivity

- **EQ-themed** (0.0–1.0, all default to 0.5):
  - `battle_hunger`: eagerness for combat
  - `piety`: devotion to deity and RP religiosity
  - `greed`: desire for loot/wealth
  - `wanderlust`: urge to explore new zones
  - `loyalty`: devotion to groupmates and guild
  - `mischief`: tendency toward pranks and playful behavior

**Mood States**:

- `Neutral`: baseline emotional state
- `Happy`: positive mood — more social, generous behavior
- `Angry`: aggressive mood — combat-hungry, less patient
- `Anxious`: nervous mood — cautious pulls, avoids risk
- `Bored`: triggers idle behaviors and wandering
- `Excited`: high energy — fast actions, more emotes
- `Melancholy`: sad mood — quiet, introspective behavior
- `Focused`: concentrated — efficient combat, minimal chat
- `Playful`: lighthearted — jokes, pranks, random emotes
- `Exhausted`: fatigued — slower actions, may AFK or log off

**Mood Evolution** (driven by `SoulEvent`):

- `Death` → `Anxious` (if high neuroticism) or `Angry` (if high battle_hunger) or `Melancholy`
- `Kill` → `Excited` (if high battle_hunger) or `Focused` (if high conscientiousness)
- `Loot` → `Happy` (if high greed) or `Playful` (if high mischief)
- `PlayerChat` → mood based on sentiment score (-1.0 hostile .. 1.0 friendly)
- `BotChat` → based on existing relationship tag
- `MoodShift` → explicit mood change with reason
- `GroupWipe` → `Melancholy` or `Anxious` depending on traits
- `LevelUp` → `Happy` or `Excited`
- `RelationshipChange` → context-dependent

**Current limitations**:
- Mood changes are immediate, not gradual decay
- Mood does not naturally drift toward neutral without events
- Sentiment analysis for player chat is currently neutral default

### 2. Memory Store (`memory.rs`)

**Purpose**: Persistent autobiographical memory for each character.

**Database schema** (SQLite):

- `memories`: event log with importance weighting and decay flag
  - Fields: `character_id`, `event_type`, `event_json`, `zone`, `mood_at_time`, `importance`, `created_at`, `decayed`
  - Indexed by `(character_id, created_at DESC)` and `(character_id, zone)`

- `conversations`: chat history with speaker tracking
  - Fields: `character_id`, `speaker`, `is_player`, `channel`, `message`, `sentiment`, `created_at`
  - Indexed by `(character_id, created_at DESC)` and `(character_id, speaker)`

- `memory_summaries`: periodic summaries of memory windows
  - Fields: `character_id`, `period_start`, `period_end`, `summary`, `mood_trend`, `created_at`
  - Indexed by `(character_id, period_start DESC)`

- `shared_references`: memories two characters experience together
  - Fields: `character_a`, `character_b`, `memory_id`, `description`, `created_at`
  - Indexed by `(character_a, character_b)`

- `speech_patterns`: per-character speech style evolution
  - Fields: `character_id`, `vocabulary_level`, `emote_frequency`, `typing_speed`, `catchphrases`, `adopted_slang`

**Operations**:
- Record events (death, loot, kills, zone changes, level ups)
- Retrieve recent memories for context
- Compute importance weighting for relevance ranking
- Apply decay to old memories (reduces weight over time)
- Generate summaries for LLM context windows
- Track shared experiences between characters

**Current limitations**:
- Memory decay is schema-only; not currently applied
- Summaries are not auto-generated
- Sentiment is not parsed from conversation text

### 3. Social Graph (`social.rs`)

**Purpose**: Model relationships between characters (both bot and real players).

**Relationship model**:
- Per-pair directed relationship (A → B)
- `faction_score`: EQ-style rating (-1000 to 1000)
- `trust`: 0.0–1.0
- `tags`: set of `SocialTag` enums
- `shared_memory_ids`: list of memories both characters experienced
- `communication_style`: hint for phrasing (e.g., "formal", "banter", "terse")

**Social tags**:
- `Friend`: positive relationship, groupmate/ally
- `Rival`: competitive relationship, contested camps, loot rivalry
- `Mentor`: teaches or guides
- `Mentee`: learns from
- `Sibling`: family bond (shared account/player lore)
- `Acquaintance`: casual contact, met once
- `Nemesis`: hostile relationship, KOS, grief history
- `Crush`: romantic interest (RP flavor)

**Social events** (driver for relationship changes):
- `FoughtTogether`: +faction for mutual trust
- `Saved`: +faction and +trust for the saver
- `LetDie`: -faction for the healer, varies for victim
- `SharedLoot`: +faction with sharing, -faction for ninjas
- `NinjaLoot`: strong -faction
- `PositiveChat`: +faction
- `NegativeChat`: -faction
- `Gossip`: indirect relationship changes via third parties
- `IdleTogether`: low-weight bond building
- `Mentored`: mentor +faction, mentee +trust

**Standing labels** (EQ-style):
- 750–1000: ally
- 400–749: warmly
- 100–399: amiably
- 0–99: indifferent
- -99..-1: apprehensive
- -399..-100: dubious
- -749..-400: threatening
- ≤-750: scowling

**Current limitations**:
- Gossip events exist but propagation mechanics not defined
- No memory of why relationships changed
- No influence on other characters' moods when a third-party gossips
- Shared references are tracked but not used for relation building

### 4. Idle Behavior System (`idle.rs`)

**Purpose**: Generate ambient actions when characters are not in combat or traveling.

**Behavior types**:
- `Sit`: regen mana/HP
- `Wander`: random movement near camp
- `Emote`: perform a random emote
- `Fish`: at nearby water
- `Craft`: tradeskills
- `VendorBrowse`: inspect vendor inventory
- `LoreChatter`: chat about zone lore or tell stories
- `BioBrk`: announce AFK message
- `LogOffToSleep`: character logs off (simulates sleep)
- `RandomJump`: fidget behavior
- `Inspect`: inspect nearby player gear

**Scheduler logic**:
- Each character has a `current` idle behavior with ticks remaining
- On each tick, scheduler evaluates whether to continue, start new, or stop
- Duration is randomized within `[min_chat_interval_secs, max_chat_interval_secs]` / tick_secs
- Selection is weighted by personality traits and mood

**Behavior weighting** (trait-driven):
- `Wander` → high wanderlust
- `Fish` → high openness or leisure moods
- `Craft` → high conscientiousness
- `VendorBrowse` → high greed
- `LoreChatter` → high extraversion, openness
- `Inspect` → high extraversion or mischief
- `BioBrk` → random, signal human presence
- `LogOffToSleep` → exhausted mood, neuroticism

**Flavor text generation**:
- Each behavior start generates optional flavor text (e.g., "sits down to regain mana")
- Fallback responder templates used in Phase 1

**Current limitations**:
- No zone-specific behaviors (e.g., fishing only near water)
- No interaction with other characters during idle
- No cumulative effects (fatigue from idle doesn't reduce alertness)
- LogOffToSleep doesn't actually remove character from rotation temporarily

### 5. LLM Integration (`llm/`)

**Purpose**: Framework for generating rich text (chat, emotes, commentary) via local or future remote models.

**Components**:

- **Provider trait** (`llm/mod.rs`):
  - `generate(&mut self, request: &LlmRequest) -> Result<LlmResponse>`
  - `name() -> &str` for logging
  - `is_available() -> bool` for graceful fallback

- **Request model**:
  - `character_name`
  - `traits`: personality vector
  - `mood`: current mood state
  - `speech_style`: vocabulary, emote frequency, typing speed, catchphrases, slang
  - `situation`: context enum (PlayerChat, GameEvent, IdleChatter, BotChat, CombatReaction, FleetCommentary)
  - `priority`: Low/Medium/High
  - `memory_context`: recent memories for context window
  - `backstory`: character backstory snippet

- **Response model**:
  - `text`: generated message
  - `from_llm`: bool (true = LLM, false = fallback)
  - `tokens_used`: u32

- **Priority queue** (`llm/priority_queue.rs`):
  - Ensures high-priority requests (player chat, real events) are processed first
  - Low-priority (idle chatter) can be deferred or dropped under load

- **Fallback responder** (`llm/fallback.rs`):
  - `TraitDrivenResponder` implements `LlmProvider` trait
  - Generates templated responses based on traits and mood
  - No real LLM calls; deterministic
  - Used in Phase 1 until M11 local model integration

- **API client** (`llm/api_client.rs`):
  - Not yet used; framework for future local ollama integration
  - Will support operator-provided API keys for local instances

**Current limitations**:
- API client exists but not wired to coordinator
- No real LLM requests sent; fallback only
- Situation context could be richer (e.g., raid context, DZ lockout state)
- No token budget enforcement in Phase 1

### 6. Soul Coordinator (`coordinator.rs`)

**Purpose**: Tick-driven orchestrator that wires all subsystems together.

**Main loop** (called every ~5 seconds by orchestrator):

1. For each registered character:
   - Evaluate mood changes based on new game events
   - Check idle scheduler for new behaviors
   - Generate LLM requests for pending chat/emotes
   - Convert `SoulAction` → IPC `Command`s

2. Dispatch `(ClientId, Command)` tuples back to orchestrator

**Registration**:
- `register_character(client_id, char_config)` initializes a `CharacterSoul` with:
  - Traits and speech style from config
  - Edginess level
  - Backstory
  - Personality engine (seeded from client ID)
  - Idle scheduler
  - Fallback responder

**Tick operations**:
- Increment tick counter
- For each soul, check idle scheduler and emit idle actions
- For each soul, check for pending memory events and record them
- For each soul, check for pending chat requests and generate responses
- For each soul, check for relationship changes and update social graph

**Configurations** (`config.rs`):

- **Global `SoulConfig`**:
  - `enabled`: bool (disable entire system)
  - `edginess`: `EdginessLevel` (Mild, Moderate, Spicy)
  - `idle_tick_secs`: seconds per tick
  - `min_chat_interval_secs`, `max_chat_interval_secs`: idle chatter timing
  - `enable_inter_character_chat`: bool
  - `enable_player_responses`: bool
  - `llm`: `LlmConfig` (provider, model, temperature, etc.)
  - `relationship`: `Vec<RelationshipSeed>` (pre-defined relationships)

- **Per-character `CharacterSoulConfig`**:
  - `name`: must match ToonConfig
  - `traits`: personality override
  - `speech`: speech style override
  - `edginess`: per-character override
  - `backstory`: RP context
  - `quirks`: unique behaviors for this character

**IPC actions**:

The coordinator emits `SoulAction` enums which are converted to `Command`:

- `Say { channel: SayChannel, message: String, target: Option<String> }`:
  - Routes to `/say`, `/shout`, `/ooc`, `/gu`, `/g`, `/tell`, `/auction`

- `Emote { emote: String }`:
  - Routes to `/emote` command

- `StartIdle { behavior: IdleBehaviorType }`:
  - Signals the DLL to enter an idle behavior loop

- `StopIdle`:
  - Signals the DLL to stop idle behavior

**Current limitations**:
- Tick is synchronous; LLM requests block the tick
- No queueing for chat if LLM is slow
- No rate limiting on chat output
- No integration with camp loop state (e.g., don't chat during critical pulls)

## Event Types (`SoulEvent`)

The soul system tracks these events:

- `Death { zone: String, killer: Option<String> }`
- `Kill { target: String, zone: String }`
- `Loot { item: String, zone: String }`
- `PlayerChat { player_name: String, sentiment: f32 }`
- `BotChat { character_name: String }`
- `Witnessed { description: String }`
- `MoodShift { from: MoodState, to: MoodState, reason: String }`
- `ZoneEnter { zone: String }`
- `LevelUp { new_level: u8 }`
- `GroupWipe { zone: String }`
- `RelationshipChange { character: String, delta: f32 }`

## Configuration Example

```toml
[soul]
enabled = true
edginess = "moderate"
idle_tick_secs = 5
min_chat_interval_secs = 30
max_chat_interval_secs = 120
enable_inter_character_chat = true
enable_player_responses = true

[soul.llm]
provider = "none"  # "ollama" in M11
model = "gemma3:4b"  # example placeholder; use the provider-specific model tag you deploy
temperature = 0.8
max_tokens = 100

[[soul.relationship]]
from = "Legolas"
to = "Gandalf"
faction = 500
tags = ["Friend"]
trust = 0.9

[[soul.character]]
name = "Legolas"
traits.openness = 0.8
traits.extraversion = 0.9
traits.battle_hunger = 0.7
speech.vocabulary_level = 0.6
speech.emote_frequency = 0.7
edginess = "moderate"
backstory = "A noble elf archer with wanderlust."
quirks = ["Often hums elvish songs", "Meticulous about gear"]
```

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│ Orchestrator (main.rs)                                  │
│ - Game state polling (5s ticks)                         │
│ - Camp loop state machine                               │
│ - Combat coordination                                   │
└─────────────────┬───────────────────────────────────────┘
                  │ tick() with HashMap<ClientId, GameState>
                  ▼
        ┌─────────────────────┐
        │ SoulCoordinator     │
        │ - register_character│
        │ - tick()            │
        │ - event recording   │
        └────┬────────┬───────┘
             │        │
      ┌──────▼──┐  ┌──▼──────────┐
      │ Per-Soul│  │ Memory Store │
      │ ┌────┐ │  │ (SQLite)     │
      │ │Mood│ │  │              │
      │ └────┘ │  └──────────────┘
      │ ┌────────────┐
      │ │PersonalityE│
      │ │ngine       │
      │ └────────────┘
      │ ┌────────────┐
      │ │IdleSchedul │
      │ │er          │
      │ └────────────┘
      │ ┌────────────┐
      │ │SocialGraph │
      │ └────────────┘
      └──────┬─────────────┐
             │             │
      ┌──────▼──┐   ┌──────▼──────┐
      │LLM Queue│   │Fallback      │
      │Request/ │   │Responder     │
      │Response │   │(Phase 1)     │
      └─────────┘   └──────────────┘
             │
      ┌──────▼───────────────────┐
      │ SoulAction / Command      │
      │ (Say, Emote, StartIdle)   │
      └──────────────────────────┘
             │
      ┌──────▼──────────────────────┐
      │ Orchestrator IPC dispatch   │
      │ (route to DLL via NamedPipe)│
      └────────────────────────────┘
```

## Current vs. Roadmap

### Phase 1 (Current)

✅ Personality trait system with mood states  
✅ Persistent memory store (SQLite schema)  
✅ Social relationship graph  
✅ Idle behavior scheduler with trait weighting  
✅ Deterministic fallback responder  
✅ Event-driven mood changes  
✅ Configuration framework  

### Phase 2 (M11)

⏳ Local LLM integration (ollama, Gemma 4)  
⏳ Memory decay and summarization  
⏳ Richer sentiment analysis  
⏳ Player chat context injection into LLM  
⏳ Personality drift over time  
⏳ Gossip propagation between characters  
⏳ Inter-character banter generation  
⏳ Combat reaction commentary  
⏳ Fleet-wide event commentary (Discord integration)  

## Key Design Principles

1. **Operator safety**: No external API calls (only local models). Full transparency of what the AI is doing.
2. **Persistence first**: Memory and relationships survive server restarts.
3. **Trait-driven**: Personality is deterministic and readable; not a black box.
4. **Graceful fallback**: If LLM is unavailable, templated responses keep the system functional.
5. **Tick-driven**: No async complexity in Phase 1; coordinator is synchronous.
6. **No disruption to gameplay**: Soul actions are cosmetic; they never interrupt combat, navigation, or orchestrator commands.

## Testing Coverage

- `textquest-common/src/soul.rs`: 40+ unit tests for types and serialization
- `textquest/src/soul/`: unit tests for each subsystem (coordinator, personality, idle, social)
- Integration tests for full tick cycle

Run with: `cargo test -p textquest -p textquest-common`
