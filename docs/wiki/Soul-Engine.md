# Soul Engine

## Current Operator View

The Soul Engine is TextQuest's personality and idle-behavior layer.

It is configured through the `[soul]` section of `config/textquest.toml` and can influence:

- idle behavior
- chat flavor
- mood changes
- long-term memory
- social relationships between characters

The persistent database path is `data/soul_memory.db`.

## Current Behavior

The current implementation is in `textquest/src/soul/` and shared types live in `textquest-common/src/soul.rs`.

Current components:

- `personality.rs`: deterministic trait-driven personality engine
- `memory.rs`: persistent memory store
- `social.rs`: relationship graph
- `idle.rs`: idle behavior scheduler
- `llm/`: request queue, provider trait, and fallback responder
- `coordinator.rs`: tick-driven orchestrator entry point

The shared trait model includes:

- Big Five traits
- EQ-themed traits such as battle hunger, piety, greed, wanderlust, loyalty, and mischief

## Current LLM Boundary

This is the most important implementation detail to understand:

- the queue and provider abstraction already exist
- the current coordinator initializes the queue with a zero token budget
- requests are answered by the deterministic fallback responder today
- the live provider surface is local-only (`ollama` or `none`), matching the roadmap's no-external-API requirement
- code comments explicitly describe real provider integration as a later phase

So the Soul Engine is present and useful now, but current repo behavior is still fallback-driven rather than live-provider-driven.

## Configuration Surface

Current supported soul config categories include:

- global enable/disable
- edginess level
- idle tick timing
- chat timing
- inter-character chat toggle
- player-chat response toggle
- per-character overrides
- relationship seeds

## Internals

The main coordinator:

- tracks a `CharacterSoul` per client
- updates mood and idle state on each tick
- records memories and conversation history
- can emit `Say` and `SoulAction` IPC commands

Important current note from code:

- player-chat sentiment is still treated as neutral by default, with richer sentiment analysis deferred to the future LLM phase.

## Current Behavior vs Roadmap

### Current behavior

- Mood, trait, idle, memory, and social-graph systems are implemented now.
- The feature is persistent and not purely cosmetic because it maintains SQLite-backed state.

### Roadmap and validation notes

- Real external provider integrations and richer in-game chat behavior now sit under `M11` in the canonical roadmap.
- If you are documenting or demoing Soul behavior today, describe it as deterministic fallback plus persistent memory, not as fully live LLM autonomy.
- Keep operator-facing descriptions explicit that Soul inference is intended to stay local and operator-safe.
