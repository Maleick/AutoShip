# Soul Engine — Performance Tuning Guide

This guide covers the concrete knobs available to operators who want to tune the Soul Engine for their hardware and fleet size.

---

## Overview of the Tick Pipeline

The Soul Engine runs inside the main orchestrator tick loop, which fires every **5000ms** by default. Each tick:

1. Iterates all registered characters (`SoulCoordinator::tick`)
2. For each character, ticks the idle scheduler (checks for emote/chat/log-off transitions)
3. Every 60 ticks (~5 minutes), runs memory decay and prune
4. Drains the LLM request queue and processes any ready responses

The tick is synchronous. LLM calls are queued, not awaited inline, so a slow LLM provider does not block the game loop tick.

---

## Key Configuration Knobs

### Tick Rate (`idle_tick_secs`)

Controls how often the idle scheduler considers firing a behavior:

```toml
[soul]
idle_tick_secs = 30   # check every 30s (default)
```

- Lower values = more responsive personality behavior, more CPU per tick
- Values below 5 are wasteful (the main loop only runs at 5s)
- Recommended range: **15–60 seconds**

For a 36-character fleet on a modern desktop, `idle_tick_secs = 30` costs roughly 0.1ms per tick total. This is negligible.

### Chat Interval (`min_chat_interval_secs` / `max_chat_interval_secs`)

Controls the random window for how often a character emotes or chats:

```toml
[soul]
min_chat_interval_secs = 120
max_chat_interval_secs = 600
```

A character will only speak if the randomized interval has elapsed since the last utterance. This is per-character. For a 36-box fleet with these settings, expect roughly 4–18 chat events per hour across the whole fleet, which is realistic.

**Performance impact:** Purely in-memory state checks. No I/O involved unless an LLM call is enqueued.

---

## LLM Latency

### Phase 1 (Fallback Responder — No LLM)

Phase 1 uses a deterministic `FallbackResponder` that picks from a table of phrases based on mood, traits, and edginess. Response generation is ~0ms. No network calls.

This is the default if `soul.llm` is not configured or the provider is unavailable.

### Phase 2 (LLM-backed Responses)

Phase 2 routes through a local ollama instance. Expected latency:

| Model       | Hardware               | First-token latency | Full response |
| ----------- | ---------------------- | ------------------- | ------------- |
| gemma3:4b   | RTX 3060 (Frostreaver) | ~200ms              | ~1–3s         |
| gemma3:4b   | CPU only               | ~2–5s               | ~10–30s       |
| llama3.2:3b | RTX 3060               | ~150ms              | ~0.8–2s       |

**Key setting:** `max_tokens` limits response length. Shorter = faster.

```toml
[soul.llm]
model = "gemma3:4b"
max_tokens = 80       # short in-game chat messages; default is higher
temperature = 0.7     # lower = more consistent, higher = more varied
```

The LLM queue (`llm_queue`) is a priority queue. High-priority requests (e.g., player-addressed tells) are processed before low-priority idle chatter. The queue drains on every tick; requests that were not ready (throttled) are re-queued.

### Throttling LLM Usage

To prevent LLM calls from queuing faster than they are processed:

1. Increase `min_chat_interval_secs` — fewer triggers means fewer LLM calls
2. Disable `inter_character_chat` — eliminates the majority of LLM load
3. Disable `player_chat_enabled` if no live players interact with your characters
4. Use `max_tokens = 60–100` for chat messages — in-game chat is short anyway

---

## Memory Database Performance

The memory store is SQLite with WAL mode enabled. For 36 characters running 24/7, expect:

- ~500–2000 memory rows per character per day (depending on activity)
- Decay runs every 60 ticks (~5 minutes), reducing importance scores by `0.995x`
- Prune removes rows with importance below `0.05` (effectively removing memories older than ~1 week of active play)

### Tuning Memory Storage

**Database location:** The path is set when calling `MemoryStore::open`. By default it goes to `data/soul/<character_id>.db`. Put this on an SSD if possible.

**WAL checkpoint:** SQLite's WAL file can grow if checkpoints are infrequent. The engine uses the default SQLite auto-checkpoint (every 1000 pages). For 36 characters under normal load, this is fine.

**Expected file sizes:**

- After 1 week active: ~1–5 MB per character
- After 1 month active: ~5–20 MB per character (with pruning active)

If files grow larger than 20MB per character, check that the orchestrator is running continuously so prune ticks fire. You can also `VACUUM` the SQLite file manually to reclaim space.

---

## Fleet Scaling

### 18-character fleet (typical group setup)

```toml
[soul]
enabled = true
idle_tick_secs = 30
min_chat_interval_secs = 180
max_chat_interval_secs = 600
inter_character_chat = true
player_chat_enabled = true

[soul.llm]
provider = "local"
model = "gemma3:4b"
max_tokens = 80
temperature = 0.7
```

Expected overhead: <1ms per tick, 1–3 LLM calls per minute across the fleet.

### 36-character fleet (full raid setup)

```toml
[soul]
enabled = true
idle_tick_secs = 60            # less frequent checks at full scale
min_chat_interval_secs = 300   # 5 minute minimum — less spam
max_chat_interval_secs = 900   # up to 15 minutes
inter_character_chat = false   # disable to halve LLM load
player_chat_enabled = true     # keep player responses enabled

[soul.llm]
provider = "local"
model = "gemma3:4b"
max_tokens = 60
temperature = 0.65
```

Expected overhead: <2ms per tick, <1 LLM call per minute across the fleet.

---

## Profiling Tips

1. **Enable debug logging** to see tick timing:

   ```
   RUST_LOG=textquest=debug cargo run
   ```

   Look for `"soul tick"` span durations in the log.

2. **Monitor the LLM queue depth** — if the queue is consistently growing and not draining, the LLM is too slow for your chat frequency. Raise `min_chat_interval_secs` or switch to a faster model.

3. **SQLite contention** — the memory store uses a single connection per database. With 36 characters, each has its own DB file, so there is no cross-character contention.

4. **Watch for decode errors** — if `recall_recent` or `recall_about` returns errors in the log, the memory database may be corrupt. The safe fix is to delete the affected character's `.db` file and restart.
