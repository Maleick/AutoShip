# Soul Engine — Operator Reference

Complete reference for configuration options, monitoring metrics, backup procedures, and configuration best practices.

---

## Configuration Reference

All Soul Engine settings live under `[soul]` in `config/frostreaver.toml`.

### Top-Level `[soul]`

| Field                    | Type  | Default      | Description                                              |
| ------------------------ | ----- | ------------ | -------------------------------------------------------- |
| `enabled`                | bool  | `false`      | Master on/off switch for the Soul Engine                 |
| `edginess`               | enum  | `"moderate"` | Global edginess floor: `"mild"`, `"moderate"`, `"spicy"` |
| `idle_tick_secs`         | u64   | `30`         | Seconds between idle scheduler checks per character      |
| `min_chat_interval_secs` | u64   | `120`        | Minimum seconds between any utterance for one character  |
| `max_chat_interval_secs` | u64   | `600`        | Maximum seconds before the next utterance is considered  |
| `inter_character_chat`   | bool  | `true`       | Whether characters talk to each other                    |
| `player_chat_enabled`    | bool  | `false`      | Whether characters respond to real player tells          |
| `character`              | array | `[]`         | Per-character soul configs (see below)                   |
| `relationship`           | array | `[]`         | Pre-seeded relationship graph entries                    |

### `[[soul.character]]`

One entry per character that should have a distinct personality. Characters without an entry use global defaults.

| Field       | Type         | Default        | Description                                                               |
| ----------- | ------------ | -------------- | ------------------------------------------------------------------------- |
| `name`      | string       | required       | Must match the in-game character name exactly (case-sensitive)            |
| `traits`    | string array | `[]`           | Personality trait tags (e.g., `["battle_hungry", "loyal"]`)               |
| `speech`    | string       | `""`           | Speech style hint passed to LLM (e.g., `"short_sentences"`, `"eloquent"`) |
| `edginess`  | enum         | global default | Per-character override: `"mild"`, `"moderate"`, `"spicy"`                 |
| `backstory` | string       | `""`           | Character backstory passed as LLM system context                          |
| `quirks`    | string array | `[]`           | Specific behavioral quirks passed to LLM as context                       |

**Supported trait tags:**

| Tag             | Effect                                                                   |
| --------------- | ------------------------------------------------------------------------ |
| `battle_hungry` | More excited by kills, aggressive mood shifts                            |
| `loyal`         | Group wipes cause melancholy rather than anger                           |
| `introverted`   | Prefers group channel; less frequent unsolicited chat                    |
| `extraverted`   | Uses `/say`; more frequent chat; positive response to player interaction |
| `conscientious` | Focused mood on kills; calm baseline                                     |
| `neurotic`      | Anxious on deaths/group wipes; more reactive to negative events          |
| `agreeable`     | Melancholy (not angry) on negative relationship events                   |
| `mischievous`   | Playful mood on loot events                                              |
| `wanderlust`    | Excited on zone enter events                                             |
| `greedy`        | Happy mood on loot events                                                |

### `[[soul.relationship]]`

| Field     | Type         | Default  | Description                                                  |
| --------- | ------------ | -------- | ------------------------------------------------------------ |
| `from`    | string       | required | Source character name                                        |
| `to`      | string       | required | Target character name                                        |
| `faction` | f32          | `0.5`    | Relationship warmth: `0.0` (hostile) to `1.0` (best friends) |
| `trust`   | f32          | `0.5`    | Trust level: `0.0` (distrusts) to `1.0` (fully trusts)       |
| `tags`    | string array | `[]`     | Relationship descriptors (e.g., `["groupmate", "rival"]`)    |

### `[soul.llm]`

| Field         | Type   | Default       | Description                                                          |
| ------------- | ------ | ------------- | -------------------------------------------------------------------- |
| `provider`    | enum   | `"local"`     | LLM provider kind: `"local"` (ollama-compatible)                     |
| `api_key`     | string | `""`          | Auth token for operator-managed endpoints; unused for default ollama |
| `model`       | string | `"gemma3:4b"` | Model name as it appears in `ollama list`                            |
| `base_url`    | string | `""`          | Base URL override; defaults to `http://localhost:11434`              |
| `max_tokens`  | u32    | `200`         | Max tokens per LLM response (keep at 60–120 for game chat)           |
| `temperature` | f32    | `0.8`         | Creativity: `0.0` = deterministic, `1.0` = maximum variation         |

### `[soul.bot_personality]`

Controls the Discord fleet commentary bot personality.

| Field                | Type   | Default           | Description                                               |
| -------------------- | ------ | ----------------- | --------------------------------------------------------- |
| `preset`             | enum   | `"fippy_darkpaw"` | Named personality preset                                  |
| `system_prompt`      | string | `""`              | Custom system prompt (only used when `preset = "custom"`) |
| `name`               | string | `"Kira"`          | Bot display name in Discord                               |
| `commentary_enabled` | bool   | `true`            | Whether the bot posts fleet commentary                    |

**Preset options:**

| Preset          | Description                                                 |
| --------------- | ----------------------------------------------------------- |
| `fippy_darkpaw` | Eternally optimistic gnoll; keeps charging despite setbacks |
| `druzzil_ro`    | Aloof goddess of magic; speaks in riddles and abstractions  |
| `bristlebane`   | Trickster god; loves puns, pranks, and chaos                |
| `custom`        | Uses `system_prompt` field directly                         |

---

## Memory Database Schema

The Soul Engine stores per-character memory in SQLite databases. Understanding the schema helps with monitoring and backup planning.

### Tables

**`memories`** — Game events the character witnessed or participated in:

- `character_id` — internal integer ID for the character
- `event_type` — event category (e.g., `"kill"`, `"death"`, `"zone_enter"`, `"loot"`)
- `event_json` — full event payload as JSON
- `zone` — zone name at time of event
- `mood_at_time` — character's mood when the event was recorded
- `importance` — float 0.0–10.0; decays over time; rows below 0.05 are pruned
- `decayed` — boolean; marks pruned rows without deleting them

**`conversations`** — Chat history:

- `speaker` — name of who spoke
- `is_player` — 1 if a real player, 0 if a bot character
- `channel` — chat channel (`say`, `group`, `tell`, etc.)
- `sentiment` — float sentiment score attached by the responder

**`memory_summaries`** — Periodic summaries of activity periods (used for LLM context compression)

**`shared_references`** — Events that two characters both witnessed (enables shared storytelling)

**`speech_patterns`** — Per-character learned speech habits:

- `vocabulary_level`, `emote_frequency`, `typing_speed`
- `catchphrases`, `adopted_slang` — JSON arrays of strings

### Decay Model

- Every 60 coordinator ticks (~5 minutes), `decay_tick` multiplies all importance scores for a character by `0.995`
- `prune_low_importance` then marks rows with `importance < 0.05` as `decayed = 1`
- Rows are soft-deleted (marked decayed) rather than hard-deleted to allow future export
- `rehearse` can increase importance (capped at 10.0) when a memory is actively recalled

---

## Monitoring Metrics

### Key Log Events

All log output uses `tracing`. Filter on `textquest::soul` for Soul Engine events.

| Log Event                                    | Level | Meaning                                             |
| -------------------------------------------- | ----- | --------------------------------------------------- |
| `"Character logging off to sleep"`           | INFO  | Idle FSM triggered a log-off transition             |
| `"soul tick"` span                           | DEBUG | One full tick across all characters; check duration |
| `from_llm: true`                             | DEBUG | LLM generated the response (not fallback)           |
| `from_llm: false`                            | DEBUG | Fallback responder was used                         |
| `"Failed to open memory store"`              | ERROR | SQLite database could not be opened                 |
| `"Failed to initialize memory store schema"` | ERROR | Schema migration failed                             |

### Health Checks

**LLM availability:** Check that `soul.llm.provider.is_available()` returns true at startup. This is logged during orchestrator initialization.

**Memory database size:** Monitor `data/soul/*.db` file sizes. Alert if any file exceeds 50MB — this suggests prune ticks are not firing.

**Tick throughput:** Under normal load, the soul tick should complete in under 5ms for 36 characters (Phase 1) or under 10ms (Phase 2 with LLM queue draining). Log the `"soul tick"` span duration at DEBUG level to baseline.

**LLM queue depth:** If requests accumulate faster than the LLM processes them, responses will be delayed. There is no current metric for queue depth; use log volume (number of `"LLM response generated"` events per minute) as a proxy.

---

## Backup and Recovery Procedures

### What to Back Up

| Path                      | Content                                           | Criticality                                                            |
| ------------------------- | ------------------------------------------------- | ---------------------------------------------------------------------- |
| `config/frostreaver.toml` | All Soul config including personality definitions | High — loss means reconfiguring all personalities                      |
| `data/soul/*.db`          | Per-character memory databases                    | Medium — loss means characters forget history but continue functioning |
| `config/accounts.toml`    | Character name to account mapping                 | High — needed for character registration                               |

### Backup Procedure

The orchestrator must be stopped before copying SQLite files to avoid WAL corruption:

```
# 1. Stop the orchestrator process (Ctrl+C or task manager)

# 2. Copy config files
xcopy /E config\ backup\config\

# 3. Copy memory databases
xcopy /E data\soul\ backup\soul\

# 4. Restart the orchestrator
cargo run
```

For unattended nightly backups, use a scheduled task:

```powershell
# PowerShell — run as scheduled task at 3am
Stop-Process -Name textquest -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 5
Copy-Item -Recurse "C:\TextQuest\data\soul" "D:\Backups\soul\$(Get-Date -Format 'yyyy-MM-dd')"
Copy-Item -Recurse "C:\TextQuest\config" "D:\Backups\config\$(Get-Date -Format 'yyyy-MM-dd')"
# Restart orchestrator via your preferred mechanism
```

### Recovery Procedure

**Config loss:**

1. Restore `config/frostreaver.toml` from backup
2. Start orchestrator — Soul Engine will re-register all characters on startup

**Memory database loss or corruption:**

1. Stop orchestrator
2. If backup available: copy backup `.db` files to `data/soul/`
3. If no backup: delete corrupt `.db` files — the engine will create fresh databases on next start. Characters will lose their history but will function normally.
4. Start orchestrator

**Corrupt single character database:**

```
# Stop orchestrator, then:
del data\soul\<character_id>.db
# Restart — engine recreates from scratch for that character
```

**Full wipe and rebuild:**

```
# Stop orchestrator
del /Q data\soul\*.db
# Restart — all characters start with empty memories
```

---

## Configuration Best Practices

**1. Start with `enabled = false`, build configs, then enable.**
Validate all character names, traits, and relationships in config before first run. Typos in character names mean no personality is applied.

**2. Set `player_chat_enabled = false` until LLM is confirmed stable.**
Real players getting no response is better than getting a nonsensical or broken response.

**3. Keep backstory under 200 words per character.**
Longer backstories cost more LLM tokens per request and may exceed context windows on smaller models.

**4. Use named presets for `bot_personality` before writing custom prompts.**
The three presets (`fippy_darkpaw`, `druzzil_ro`, `bristlebane`) are tested and in-character for EverQuest. Custom prompts should be validated in a test session before live use.

**5. Configure `edginess = "mild"` globally and override per-character for spicy personalities.**
This prevents a misconfigured character from producing inappropriate content fleet-wide.

**6. Relationship seeds are one-directional — add both directions for symmetric relationships.**
`from = "Throgg", to = "Aria"` does not imply `from = "Aria", to = "Throgg"`. Add both entries if you want symmetry.

**7. Back up `data/soul/*.db` nightly.**
Memory databases are rebuilt from zero if lost, but a week of accumulated speech patterns and conversation history is valuable for personality consistency.

**8. Do not share `api_key` in committed config files.**
If using a non-default LLM endpoint that requires authentication, set `api_key` via environment variable or use the credential store. Do not check it into the repo.
