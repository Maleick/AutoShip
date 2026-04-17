# Configuration

## Current Config Files

| Path | Purpose |
| --- | --- |
| `config/textquest.toml` | Main TextQuest app config |
| `config/accounts.toml` | Account and group-launch metadata |
| `config/character-configs.json` | Web UI character tuning and task reward automation rules |
| `data/credentials.db` | Encrypted account password store used by `textquest autologin` |
| `config/camps/*.toml` | Saved camp locations and thresholds |
| `config/classes/*.toml` | Per-class combat and ability config |
| `config/toons/*.toml` | Per-toon combat action overrides for the injected DLL |
| `config/hvt_watchlist.toml` | High-value target watchlist |
| `config/named_mobs/*.toml` | Named spawn definitions by zone |
| `config/maps/*.txt` | Brewall-style zone map data |
| `config/offsets.json` | Checked-in offset data snapshot and schema reference |
| `data/ghidra.db` | Local SQLite cache for runtime/debug Ghidra lookups |
| `data/ghidra-export/` | Local JSON export cache used by import/debug tooling |

## Main App Config

The main operator config path is `config/textquest.toml`.

`config/frostreaver.toml` is a legacy filename that still appears in older setups and in one compatibility path inside `config check`, but docs should point operators at `config/textquest.toml`.

Current sections include:

- `process_name`
- `max_spawns`
- `[launch]`
- `[server]`
- `[retry]`
- `[alerts]`
- `[soul]`
- `[[group]]`
- Discord-related options

## Operational Alerts

Operational alert defaults are loaded from the main app config and persisted at runtime in
`data/alerts.db`.

Use the `[alerts]` table in `config/textquest.toml` for bootstrap values:

```toml
[alerts]
enable_discord = true
discord_webhook_url = "https://discord.com/api/webhooks/..."
enable_email = false
smtp_server = "smtp.example.com"
smtp_port = 587
smtp_username = "operator"
smtp_password = "__SET_SMTP_PASSWORD__"
email_from = "alerts@example.com"
email_recipients = ["ops@example.com"]
email_subject_prefix = "[TextQuest] "
warning_batch_window_secs = 300

[alerts.thresholds]
death_alert = true
stuck_alert = true
memory_warning_mb = 200
ipc_latency_warning_ms = 10
error_rate_warning_per_min = 5
dps_drop_warning_pct = 20
zone_timeout_secs = 60
```

Current behavior:

- Critical alerts are delivered immediately.
- Warning alerts are batched over `warning_batch_window_secs`.
- Info alerts stay in the alert history and daily-summary path.
- The checked-in `config/frostreaver.toml` template now includes this section; copy it into `config/textquest.toml` for active operator configs.
- The web dashboard can edit the live alert config at runtime, but those edits are currently process-local and are recorded as audit alerts instead of being written back to TOML automatically.

## Accounts

`config/accounts.toml` maps login accounts to:

- account name
- server
- character
- class
- group

Passwords are intentionally not stored here.

The encrypted password store for those accounts lives separately at
`data/credentials.db`.

Example:

```toml
[[accounts]]
name = "frostreaver01"
server = "Firiona Vie"
character = "TBD"
class = "WAR"
group = 1
```

## Camps

Camp configs in `config/camps/*.toml` define:

- zone
- camp center
- pull point
- pull radius
- camp radius
- leash radius
- mana thresholds
- optional next/previous camp links

These files drive `:camp start`, `:camp next`, and `:camp prev`.

## Class Configs

`config/classes/*.toml` holds class-specific combat behavior and ability choices.

Current repo coverage includes classes such as:

- bard
- beastlord
- berserker
- cleric
- druid
- enchanter
- magician
- monk
- necromancer
- paladin
- ranger
- rogue
- shadowknight
- shaman
- warrior
- wizard

## Per-Toon Combat Configs

`config/toons/<toon>.toml` can override the injected DLL combat action tables for one specific character.

Supported sections are:

- `[[spells]]`
- `[[disciplines]]`
- `[[holyshit_rules]]`
- `[[rotation_groups]]` with nested `[[rotation_groups.entries]]`

These files are optional. When no per-toon file exists, TextQuest keeps using the existing built-in class strategy and any already-supplied combat config data.

## Maps and Offsets

- `config/maps/*.txt` supplies zone linework and labels for the TUI map.
- `config/offsets.json` is a maintained offset-data snapshot that matches the `textquest-common/src/offset_db.rs` schema. The compiled defaults still live in `textquest-common/src/offsets.rs`.

## Local Ghidra Caches

- `data/ghidra.db` and `data/ghidra-export/` are local runtime/debug caches only.
- `data/alerts.db` is a local runtime SQLite log for operational alerts and acknowledgments.
- Canonical manifests, snapshot variants, baseline selection, and copied evidence live in the sibling `Maleick/TextQuest-Ghidra` repo under `snapshots/` and `baseline-selection/current.json`.
- If a runbook needs durable evidence, link to the canonical `TextQuest-Ghidra` snapshot path rather than copying payload into this repo.

## Internals

- App config types are defined in `textquest/src/config.rs`.
- Accounts config loads into `AccountsConfig` and is used by TUI `:login` flows.
- Soul config types are defined in `textquest/src/soul/config.rs`.

## Current Behavior vs Roadmap

### Current behavior

- The current configuration surface is already large enough that docs should point to specific files, not only describe it abstractly.
- `config/textquest.toml` is the active config path for operators.
- `config/frostreaver.toml` should only be treated as legacy fallback/history unless a command explicitly calls it out.

### Validation notes

- When features change, update both the code and the relevant sample config comments so the wiki does not drift from what operators actually edit.
