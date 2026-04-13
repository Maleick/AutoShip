# Soul Engine — Migration Guide

## Phase 1 → Phase 2: Deterministic to LLM-Backed Personalities

The Soul Engine is designed in two phases:

- **Phase 1 (Fallback/Deterministic):** Characters react using pre-defined phrase tables keyed on mood, traits, and edginess. No external dependencies. Zero latency. Works out of the box.
- **Phase 2 (LLM-backed):** Characters respond using a local LLM (ollama) with full backstory context, memory recall, and dynamic text generation.

This guide walks through migrating from Phase 1 to Phase 2, and covers how to roll back if needed.

---

## Prerequisites

Before enabling Phase 2:

1. **Install ollama** on the machine running the TextQuest orchestrator (Frostreaver or equivalent Windows host).
   ```
   https://ollama.com/download
   ```
2. **Pull the model:**
   ```
   ollama pull gemma3:4b
   ```
3. **Verify ollama is running and the model is available:**

   ```
   curl http://localhost:11434/api/tags
   ```

   You should see `gemma3:4b` (or your chosen model) in the response.

4. **Confirm TextQuest is on a build that includes the LLM provider** — M11 milestone or later.

---

## Step 1: Enable Phase 1 First

If you have not already used the Soul Engine in Phase 1, enable it and validate that personalities work before adding LLM complexity:

```toml
# config/frostreaver.toml

[soul]
enabled = true
edginess = "moderate"
idle_tick_secs = 30
min_chat_interval_secs = 120
max_chat_interval_secs = 600
inter_character_chat = true
player_chat_enabled = false   # keep false until Phase 2 is confirmed working

# Do NOT configure [soul.llm] yet — Phase 1 fallback will be used automatically
```

Run the orchestrator and confirm in `logs/textquest.log` that you see soul tick events and characters emoting/chatting. Fix any personality issues (see `troubleshooting.md`) before proceeding.

---

## Step 2: Add Character Configs

For Phase 2 to generate meaningful responses, each character needs a backstory and traits. Add entries before switching to LLM mode:

```toml
[[soul.character]]
name = "Throgg"
traits = ["battle_hungry", "loyal"]
speech = "short_sentences"
edginess = "spicy"
backstory = """
A veteran troll warrior who has patrolled Innothule Swamp for two decades.
Throgg respects strength above all else. He speaks in clipped sentences and
never wastes words. He is fiercely loyal to his current group and distrusts
all Humans on sight.
"""
quirks = ["says 'smash' when excited", "never admits weakness"]

[[soul.character]]
name = "Aria"
traits = ["introverted", "conscientious"]
speech = "eloquent"
edginess = "mild"
backstory = """
A high elf enchantress who studied at the Academy of Arcane Sciences in Felwithe.
Aria prefers books to people but is deeply committed to her role as support.
She speaks formally and dislikes unnecessary violence.
"""
quirks = ["quotes elvish proverbs", "fusses over mana efficiency"]
```

Iterate on backstory text during Phase 1 testing. The same backstory is used by the Phase 2 LLM as its character grounding.

---

## Step 3: Add Relationship Seeds (Optional but Recommended)

Define how characters relate to each other. These influence Phase 1 mood shifts and will be provided to the LLM as context in Phase 2:

```toml
[[soul.relationship]]
from    = "Throgg"
to      = "Aria"
faction = 0.6     # 0.0 = enemy, 1.0 = best friends; default 0.5
trust   = 0.8
tags    = ["groupmate", "battle-tested"]
```

---

## Step 4: Configure the LLM Provider

Add the `[soul.llm]` section to your config:

```toml
[soul.llm]
provider  = "local"           # "local" = ollama-compatible endpoint
model     = "gemma3:4b"       # model name as it appears in `ollama list`
base_url  = "http://localhost:11434"
max_tokens = 80               # short is better for in-game chat
temperature = 0.7             # 0.0 = deterministic, 1.0 = creative
```

**Do not** set `provider = "remote"` unless you have a private endpoint. There is no Anthropic/OpenAI support by design (no external API calls from live game clients).

---

## Step 5: Enable Player Chat Response

Once the LLM is confirmed working, enable player chat responses:

```toml
[soul]
player_chat_enabled = true
```

Test by sending a tell to one of your characters from a separate account. You should see a response within a few seconds (dependent on LLM latency).

---

## Step 6: Validate Phase 2 Operation

Check the orchestrator log for these indicators of successful Phase 2 operation:

- `"LLM response generated"` or similar — indicates the LLM returned a result
- `from_llm: true` in LLM response log fields — confirms live model was used (not fallback)
- No `"is_available: false"` errors for the LLM provider

If you see fallback responses being used despite LLM config being present, the provider is failing its availability check. See `troubleshooting.md` issue #8.

---

## Step 7: Tune and Monitor

After Phase 2 is stable for 24–48 hours:

1. Review chat logs to ensure character voices are consistent and in-character
2. Adjust `temperature` down if responses feel too random
3. Adjust `max_tokens` to keep messages brief (80–120 tokens is typical for game chat)
4. Monitor the LLM queue — if it is consistently backlogged, reduce chat frequency

---

## Rolling Back to Phase 1

To revert to Phase 1 at any time, simply remove or comment out the `[soul.llm]` section:

```toml
# [soul.llm]
# provider = "local"
# ...
```

The engine detects that the LLM provider is unavailable and falls back to the deterministic responder automatically. No restart is required if the config is hot-reloaded; otherwise restart the orchestrator.

Memory databases are preserved and remain valid for Phase 2 re-enablement later.

---

## Migrating Memory Databases

Memory databases are stored per-character as SQLite files. They are forward- and backward-compatible between Phase 1 and Phase 2 — the schema does not change between phases.

If you need to move memory databases (e.g., to a new machine):

1. Stop the orchestrator
2. Copy `data/soul/*.db` to the new machine's equivalent path
3. Start the orchestrator — it will open the existing databases and continue accumulating memories

If a database is corrupt or from an incompatible older schema, delete the `.db` file and the engine will create a fresh one on next start.

---

## Summary Checklist

- [ ] ollama installed and running
- [ ] Model pulled (`gemma3:4b` or equivalent)
- [ ] Phase 1 validated (characters emote without LLM)
- [ ] Per-character backstory and traits defined
- [ ] Relationship seeds added (optional)
- [ ] `[soul.llm]` section added to config
- [ ] LLM availability confirmed in logs (`from_llm: true`)
- [ ] `player_chat_enabled = true` after LLM confirmed working
- [ ] Chat frequency tuned to realistic levels
