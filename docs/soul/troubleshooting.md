# Soul Engine — Troubleshooting Guide

This guide covers the most common issues operators encounter when running the Soul Engine. Each entry lists symptoms, root cause, and concrete steps to fix it.

---

## 1. Characters never speak or emote

**Symptoms:** Soul Engine is enabled but no `/say`, `/emote`, or group chat messages ever appear from any character.

**Cause:** `enabled = false` in the Soul config, or `idle_tick_secs` is set so high that idle events never fire.

**Fix:**

```toml
[soul]
enabled = true
idle_tick_secs = 30        # default; lower = more frequent checks
min_chat_interval_secs = 60
max_chat_interval_secs = 300
```

Verify the Soul Engine is running by checking orchestrator logs for `"Character logging off to sleep"` or idle transition events.

---

## 2. Characters speak too frequently and look botlike

**Symptoms:** Characters spam chat every few seconds, or emit multiple messages in rapid succession.

**Cause:** `min_chat_interval_secs` is too low, or `idle_tick_secs` is very small.

**Fix:** Raise the minimum and maximum chat intervals to human-realistic ranges.

```toml
[soul]
min_chat_interval_secs = 120    # at least 2 minutes between utterances
max_chat_interval_secs = 600    # up to 10 minutes
idle_tick_secs = 30
```

---

## 3. Characters do not respond to player tells

**Symptoms:** A real player sends a tell to one of your characters and gets no response.

**Cause:** `player_chat_enabled = false`, or the LLM queue is full / the fallback responder is being used and it is misconfigured.

**Fix:**

1. Enable player chat response in config:
   ```toml
   [soul]
   player_chat_enabled = true
   ```
2. If using Phase 2 (LLM), check that the LLM provider is reachable. See issue #8 below.
3. Check orchestrator logs for `on_player_message_ignores_*` events. Empty or whitespace-only messages are silently dropped.

---

## 4. Inter-character conversation never happens

**Symptoms:** Characters have personality relationships configured but never talk to each other.

**Cause:** `inter_character_chat = false`, or the characters are in different zones so the group-member detection finds zero nearby peers.

**Fix:**

```toml
[soul]
inter_character_chat = true
```

Ensure the characters you want chatting together are actually in the same zone and within the nearby-spawn radius. Group members are inferred from `nearby_spawns` with `spawn_type == 0`.

---

## 5. Memory database grows without bound

**Symptoms:** The SQLite memory file at `data/soul/<character_id>.db` keeps growing; disk usage climbs over time.

**Cause:** Memory decay is not running, or `prune_low_importance` threshold is too high.

**Root cause details:** The coordinator runs `decay_tick` and `prune_low_importance` every 60 ticks (~5 minutes at the default 5-second tick). If the orchestrator is frequently restarted, accumulated memories never decay.

**Fix:**

1. Let the orchestrator run continuously so decay ticks fire.
2. If the database is already large, run a one-time prune via the debug CLI (future tooling) or delete and let the engine rebuild from scratch — memories are not critical for functionality.
3. Decrease the importance threshold for pruning (lower = prune more aggressively):
   ```toml
   # Not yet a config knob — see docs/soul/operator-reference.md for defaults
   ```
   Currently decay uses a factor of `0.995` per 5-minute tick and prunes rows below `0.05` importance. These values are compile-time defaults in `coordinator.rs`.

---

## 6. LLM responses contain out-of-character content

**Symptoms:** Characters say things completely unrelated to EverQuest, use modern slang, or reveal the bot's nature.

**Cause:** The `system_prompt` for the character or bot personality is too permissive, or the `edginess` level is set to `spicy` with an untested custom prompt.

**Fix:**

1. Tighten the character's `backstory` field to anchor the persona:
   ```toml
   [[soul.character]]
   name = "Throgg"
   backstory = "A grizzled troll warrior who has patrolled Innothule Swamp for decades. Speaks in short sentences. Never breaks character."
   ```
2. Lower edginess to `mild` or `moderate` if content is inappropriate:
   ```toml
   [soul]
   edginess = "moderate"
   ```
3. For the Discord bot personality, use a named preset (`fippy_darkpaw`, `druzzil_ro`, `bristlebane`) rather than a custom prompt until you have tested it.

---

## 7. Characters all have the same personality / sound identical

**Symptoms:** All characters emit similar phrases regardless of their configured traits.

**Cause:** Per-character `[[soul.character]]` entries are missing, so all characters fall back to global defaults.

**Fix:** Add a dedicated config entry for each character with distinct traits:

```toml
[[soul.character]]
name = "Throgg"
traits = ["battle_hungry", "loyal"]
speech = "short_sentences"
edginess = "spicy"

[[soul.character]]
name = "Aria"
traits = ["introverted", "conscientious"]
speech = "eloquent"
edginess = "mild"
```

---

## 8. LLM provider unavailable / fallback only responses

**Symptoms:** Orchestrator logs show `"is_available: false"` for the LLM provider; characters use short canned phrases instead of dynamic responses.

**Cause:** `base_url` is wrong, the local ollama service is not running, or an API key is missing/expired.

**Fix:**

1. Verify ollama is running on the target host: `curl http://localhost:11434/api/tags`
2. Confirm the model is pulled: `ollama pull gemma3:4b`
3. Update the config to point to the correct host:
   ```toml
   [soul.llm]
   provider = "local"
   model = "gemma3:4b"
   base_url = "http://localhost:11434"
   ```
4. If using a remote provider, check that `api_key` is set and valid.

---

## 9. Characters log off to sleep at wrong times

**Symptoms:** Characters disconnect during active raid hours, or never log off overnight.

**Cause:** The idle scheduler `LogOffToSleep` transition fires based on wall-clock time. If the machine running TextQuest is in a different timezone than expected, or `idle_tick_secs` is very large, the log-off window may be missed or triggered prematurely.

**Fix:**

1. Ensure the system clock on the orchestrator machine is set correctly.
2. Adjust `idle_tick_secs` to be smaller (e.g., 15–30 seconds) so the scheduler checks more frequently and does not miss the log-off window by a large margin.
3. Review the idle behavior definitions in `config/classes/` for the relevant character class. The `LogOffToSleep` behavior is class-agnostic and driven by the idle FSM, not by class config.

---

## 10. Social graph relationships not applied

**Symptoms:** Characters with defined relationships (trust, faction) do not behave differently toward each other.

**Cause:** `[[soul.relationship]]` entries use character names that don't exactly match `[[soul.character]]` `name` fields, or the social graph was not seeded because `inter_character_chat = false`.

**Fix:**

```toml
[[soul.relationship]]
from = "Throgg"    # must match soul.character[].name exactly (case-sensitive)
to   = "Aria"
faction = 0.8
trust   = 0.9
tags    = ["groupmate", "friend"]
```

Run the orchestrator with debug logging to confirm the relationship seeds are applied at startup.

---

## 11. Memory recall returns stale / irrelevant context

**Symptoms:** LLM responses reference old events that are no longer relevant, or the character remembers things from a different zone.

**Cause:** Memory importance scores have not decayed enough, or `recall_about` is pulling on a subject that matches too broadly.

**Fix:**

1. Trigger a manual prune by restarting the orchestrator — the prune fires on the first decay tick.
2. If the problem persists, delete the character's `.db` file under `data/soul/` and let the engine rebuild. This clears all memory; use only when the memory is clearly corrupted.

---

## 12. High CPU usage when Soul Engine is active

**Symptoms:** Orchestrator process pegs a CPU core whenever the Soul Engine is ticking.

**Cause:** `idle_tick_secs` is set to 0 or 1, causing the scheduler to fire every second for all registered characters.

**Fix:** Set `idle_tick_secs` to at least 15 seconds:

```toml
[soul]
idle_tick_secs = 30
```

The main orchestrator tick is 5000ms; the Soul Engine tick is aligned to that. Values below 5 provide no additional benefit and waste CPU.
