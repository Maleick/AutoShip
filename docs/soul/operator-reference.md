# Soul Engine — Operator Reference

Operational reference for Soul configuration, persistence, and recovery.

## Config Location

Configure Soul under `[soul]` in `config/textquest.toml`.

`config/frostreaver.toml` is legacy-only and should only matter if you are working through older local setups or the compatibility path in `config check`.

## Top-Level `[soul]`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | `false` | Master on/off switch |
| `edginess` | enum | `"moderate"` | One of `"mild"`, `"moderate"`, `"spicy"` |
| `idle_tick_secs` | u64 | `30` | Idle scheduler interval |
| `min_chat_interval_secs` | u64 | `60` | Minimum gap between utterances |
| `max_chat_interval_secs` | u64 | `300` | Maximum gap before another utterance is considered |
| `inter_character_chat` | bool | `true` | Allow bot-to-bot chatter |
| `player_chat_enabled` | bool | `true` | Allow responses to real players |
| `character` | array | `[]` | Per-character overrides |
| `relationship` | array | `[]` | Seeded social graph entries |
| `llm` | table | see below | Provider settings |
| `bot_personality` | table | see below | Discord commentary bot settings |
| `max_requests_per_character` | u32 | `10` | Valid range: `1..=60` |
| `max_global_requests` | u32 | `60` | Valid range: `1..=200` |
| `memory_decay_days` | u32 | `30` | Must be greater than `0` |
| `mood_decay_rate` | f32 | `0.05` | Valid range: `0.0..=1.0` |

## `[[soul.character]]`

Use one entry per character that needs explicit Soul tuning.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `name` | string | required | Must match the toon name exactly |
| `traits` | table | defaults | Big Five + EQ traits, each `0.0..=1.0` |
| `speech` | table | defaults | Speech style tuning |
| `edginess` | enum | inherits global | `"mild"`, `"moderate"`, `"spicy"` |
| `backstory` | string | `""` | Local context seed |
| `quirks` | string array | `[]` | Flavor hooks and behavior nudges |

### `[soul.character.traits]`

Every field is normalized to `0.0..=1.0`.

| Field |
| --- |
| `openness` |
| `conscientiousness` |
| `extraversion` |
| `agreeableness` |
| `neuroticism` |
| `battle_hunger` |
| `piety` |
| `greed` |
| `wanderlust` |
| `loyalty` |
| `mischief` |

### `[soul.character.speech]`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `vocabulary_level` | f32 | `0.5` | Simpler to more elaborate phrasing |
| `emote_frequency` | f32 | `0.5` | Lower means fewer emotes |
| `typing_speed` | f32 | `1.0` | Higher means faster responses |
| `catchphrases` | string array | `[]` | Repeated stock phrases |
| `adopted_slang` | string array | `[]` | Learned slang and group jargon |

## `[[soul.relationship]]`

Relationship seeds are directional. Add both directions if you want symmetry.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `from` | string | required | Source character |
| `to` | string | required | Target character |
| `faction` | i32 | `0` | Relationship score |
| `trust` | f32 | `0.5` | Trust level |
| `tags` | string array | `[]` | Use enum names such as `"Friend"` or `"Mentor"` |

Supported `tags` values:

- `Friend`
- `Rival`
- `Mentor`
- `Mentee`
- `Sibling`
- `Acquaintance`
- `Nemesis`
- `Crush`

## `[soul.llm]`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `provider` | enum | `"none"` | One of `"ollama"` or `"none"` |
| `api_key` | string | `""` | Optional auth token for operator-managed endpoints |
| `model` | string | `"gemma3:4b"` | Model name |
| `base_url` | string | `""` | Optional override for an ollama-compatible endpoint |
| `max_tokens` | u32 | `100` | Keep short for in-game chat |
| `temperature` | f32 | `0.8` | Creativity tuning |

## `[soul.bot_personality]`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `preset` | enum | `"fippy_darkpaw"` | One of `fippy_darkpaw`, `druzzil_ro`, `bristlebane`, `custom` |
| `system_prompt` | string | `""` | Used when `preset = "custom"` or to augment a preset |
| `name` | string | `"Fippy Darkpaw"` | Discord display name |
| `commentary_enabled` | bool | `true` | Enable fleet commentary output |

## Persistence

Soul stores runtime memory in `data/soul_memory.db`.

The database includes:

- `memories`
- `conversations`
- `memory_summaries`
- `shared_references`
- `speech_patterns`
- `soul_audit_log`

The store is a single SQLite database partitioned by `character_id`.

## Monitoring

Useful log indicators:

- `from_llm: true` means a live LLM response was used
- `from_llm: false` means fallback text handled the response
- `Failed to open memory store` means SQLite could not be opened
- `Failed to initialize memory store schema` means schema setup failed

## Backup and Recovery

Back up:

- `config/textquest.toml`
- `config/accounts.toml`
- `data/soul_memory.db`

Stop the orchestrator before copying the database.

Recovery path:

1. Restore `config/textquest.toml` and `data/soul_memory.db`.
2. Start the orchestrator.
3. If `data/soul_memory.db` is corrupt and no backup exists, remove it and let Soul recreate a fresh database on next start.

## Example

```toml
[soul]
enabled = true
edginess = "moderate"
player_chat_enabled = false

[[soul.character]]
name = "Luminara"
edginess = "mild"
backstory = "A patient cleric who treats every camp like a duty post."
quirks = ["offers a quick blessing before pulls"]

[soul.character.traits]
openness = 0.40
conscientiousness = 0.85
extraversion = 0.35
agreeableness = 0.90
neuroticism = 0.10
battle_hunger = 0.20
piety = 0.95
greed = 0.25
wanderlust = 0.30
loyalty = 0.95
mischief = 0.15

[soul.character.speech]
vocabulary_level = 0.75
emote_frequency = 0.30
typing_speed = 0.95
catchphrases = ["steady now"]
adopted_slang = ["camp locked down"]

[[soul.relationship]]
from = "Luminara"
to = "Grimjaw"
faction = 500
trust = 0.80
tags = ["Friend"]

[soul.llm]
provider = "none"
model = "gemma3:4b"
```
