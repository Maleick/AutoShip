# Configuration

## Current Config Files

| Path                            | Purpose                                                        |
| ------------------------------- | -------------------------------------------------------------- |
| `config/textquest.toml`         | Main TextQuest app config                                      |
| `config/accounts.toml`          | Account and group-launch metadata                              |
| `config/character-configs.json` | Web UI character tuning and task reward automation rules       |
| `data/credentials.db`           | Encrypted account password store used by `textquest autologin` |
| `config/camps/*.toml`           | Saved camp locations and thresholds                            |
| `config/classes/*.toml`         | Per-class combat reference and parity config                   |
| `config/toons/*.toml`           | Per-toon combat action overrides for the injected DLL          |
| `config/hvt_watchlist.toml`     | High-value target watchlist                                    |
| `config/named_mobs/*.toml`      | Named spawn definitions by zone                                |
| `config/maps/*.txt`             | Brewall-style zone map data                                    |
| `config/offsets.json`           | Checked-in offset data snapshot and schema reference           |
| `data/ghidra.db`                | Local SQLite cache for runtime/debug Ghidra lookups            |
| `data/ghidra-export/`           | Local JSON export cache used by import/debug tooling           |

## Runtime Offset Resolution

The injected DLL loads the compiled offset table at startup, overlays the configured `offsets.json` snapshot when present, and can optionally apply a live shadow scan when `TEXTQUEST_SCAN_OFFSETS=1` is set. The resulting database is installed into both typed function bindings and generic `offsets::rebase(...)` consumers, so scan results win over JSON overrides, which win over compiled constants.

Current behavior:

- `TEXTQUEST_SKIP_SCAN=1` disables runtime offset loading and shadow scanning, leaving compiled constants only.
- Runtime offset snapshots are loaded from configured `offsets.json` sources on each startup and overlaid onto the compiled base.
- `TEXTQUEST_SCAN_OFFSETS=1` enables shadow scanning of `eqgame.exe` and `eqmain.dll`; scan results are merged on top of the runtime snapshot before installation.
- If a runtime snapshot is unavailable or a named offset is missing, consumers still fall back to the compiled constant and continue logging through the normal DLL startup path.

## Main App Config

The main operator config path is `config/textquest.toml`.

`config/frostreaver.toml` is a legacy filename that still appears in older setups and in one compatibility path inside `config check`, but docs should point operators at `config/textquest.toml`.

Current sections include:

- `process_name`
- `max_spawns`
- `backend_url` for the injected DLL outbound WebSocket connection to
  textquest-web. `ws://localhost:3001` is the local development default and is
  normalized to `/ws`; production deployments should use the accepted
  `wss://...` backend URL, including `?token=...` when WebSocket auth is
  enabled. `TEXTQUEST_DLL_BACKEND_URL` overrides the TOML value.
- `[launch]`
- `[server]`
- `[retry]`
- `[log]`
- `[alerts]`
- `[vendor_watch]`
- `[soul]`
- `[[group]]`
- Discord-related options

## Log Retention

`[log]` in `config/textquest.toml` controls TextQuest's rolling log retention.

```toml
[log]
max_size_mb = 100
max_files = 7
max_age_days = 30
```

Current behavior:

- `max_files` is passed into tracing startup on Windows, so daily-rotated `textquest.log` files now honor the configured retained-file count.
- `max_age_days` and `max_size_mb` continue to govern post-write pruning for retained logs.

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

## Vendor Watch

`[vendor_watch]` configures MQ2Vendors-style merchant browse alerts. The web
dashboard is the primary operator surface, but the underlying watch list is
persisted in `config/textquest.toml`.

Example:

```toml
[vendor_watch]
enabled = true

[[vendor_watch.items]]
item_name = "Fungi Covered Scale Tunic"
max_price_copper = 500000
enabled = true

[[vendor_watch.items]]
item_name = "Holgresh Elder Beads"
enabled = true
```

Fields:

- `enabled`: master toggle for live merchant browsing alerts
- `item_name`: case-insensitive item name match against merchant window rows
- `max_price_copper`: optional price cap used for expected-vs-actual comparison
- `enabled` on each item: lets the dashboard keep an entry without actively matching it

Runtime behavior:

- `textquest-web` polls visible merchant windows from injected clients and records alerts when a watched item appears during normal vendor browsing.
- Alerts are deduped per live merchant listing until the merchant window closes or the observed listing changes.
- When EQ exposes a merchant price column, the dashboard stores both the observed price and the delta against the configured cap.

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
- optional `return_no_aggro`
- optional next/previous camp links

These files drive `:camp start`, `:camp next`, and `:camp prev`.

The runtime loader only consumes the baseline TOML fields above. When a camp
needs richer operator metadata than the schema can carry, document richer waypoint and restriction notes
in a companion wiki page instead of inventing new untracked TOML keys.

Current examples:

- `config/camps/sebilis_disco.toml` holds the loader-safe runtime values.
- `docs/wiki/Sebilis-Disco-Camp.md` documents richer waypoint and restriction notes because the
  runtime loader only consumes the baseline TOML fields.
- [Sebilis Disco Camp](Sebilis-Disco-Camp.md) is the companion page for those notes.
- `config/camps/dreadlands_primary.toml` holds the loader-safe runtime values.
- [Dreadlands Primary Camp](Dreadlands-Primary-Camp.md) carries the planning waypoint lattice,
  pull controls, and blocked validation notes for that camp.
- `config/camps/velketors_labyrinth_frenzy.toml` holds the loader-safe runtime
  values.
- `docs/wiki/Velketors-Labyrinth-Frenzy-Camp.md` documents richer waypoint and restriction notes because the runtime loader
  only consumes the baseline TOML fields.
- [Velketor's Labyrinth Frenzy Camp](Velketors-Labyrinth-Frenzy-Camp.md) is the companion page for those notes.

## Class Configs

`config/classes/*.toml` documents class-specific combat behavior and operator-facing default ability choices.

Runtime loading still happens through `config/toons/*.toml` overrides plus the built-in class strategies in `textquest-dll/src/combat/classes/`.
The class TOML files are the reference surface for those built-in defaults.
For Bard, unit tests keep the live-safe runtime rotation data synchronized with `config/classes/bard.toml`.

Rogue ships explicit level breakpoints for 60, 61, 62, and 65 so the operator-facing TOML stays aligned with the injected DLL's burn and utility tuning.

Class configs can also define `[[level_overrides]]` with nested
`combat_abilities`, `buff_abilities`, `emergency_abilities`, `cc_abilities`,
and `debuff_abilities` sections when a class needs explicit breakpoint tuning.
The Berserker profile now uses level-gated 60/61/62/65 combat overrides to
keep its endurance-aware burn order aligned with the runtime rotation.

Class configs can also define `[[level_overrides]]` with nested
`combat_abilities`, `buff_abilities`, `emergency_abilities`, `cc_abilities`,
and `debuff_abilities` sections when a class needs explicit breakpoint tuning.
The Berserker profile now uses level-gated 60/61/62/65 combat overrides to
keep its endurance-aware burn order aligned with the runtime rotation.

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

Class configs can carry more than the legacy `command` / `cooldown_secs` / `priority` triplet when a class needs tighter tuning. Recent Live-safe profiles also document:

- `[resource_thresholds]` for HP, mana, endurance, and stop-cast floors
- `[[level_overrides]]` for explicit level breakpoints
- extra per-ability metadata such as `line` and `surface`

The operator-facing file should stay aligned with the runtime strategy. When a class rotation changes in code, update the corresponding `config/classes/*.toml` file in the same PR.

## Per-Toon Combat Configs

`config/toons/<toon>.toml` can override the injected DLL combat action tables for one specific character.

Supported sections are:

- `[[spells]]`
- `[[disciplines]]`
- `[[holyshit_rules]]`
- `[[rotation_groups]]` with nested `[[rotation_groups.entries]]`

These files are optional. When no per-toon file exists, TextQuest keeps using the existing built-in class strategy and any already-supplied combat config data.

## Web Character Configs

`config/character-configs.json` stores the per-character settings edited from the
web dashboard strategy tuning panel.

Current operator-facing fields include:

- combat thresholds such as `heal_at_pct`, `mana_sit_pct`, and `nuke_at_pct`
- ordered rotation entries and class-specific strategy parameters
- resurrection-offer policy under `auto_rez`
- task reward automation rules under `reward_automation`
- tribute automation preferences and live tribute status
- `window_title_format` for EQ window title customization

`window_title_format` accepts MQ2WinTitle-style tokens, including:

- `{server}`
- `{character}`
- `{level}`
- `{class}`
- `{class_short}`
- `{zone}`
- `{zone_long}`
- `{zone_short}`

The DLL refreshes the configured title when the character finishes loading and
again after each zone change. A typical format looks like:

```text
[{server}] {character} ({level} {class_short})
```

## Runtime-Only Dashboard Controls

Some operator controls currently live only in the running `textquest-web`
process and are applied directly to connected clients instead of being written
back into a checked-in config file.

Current runtime-only controls include:

- `Auto-Accept Wards`: per-request prompt acceptance plus trust-list policy
- `Tradeskill Trophy`: master enable plus the exact trophy item name to equip
  around supported crafting stations

Current behavior:

- dashboard edits are applied to every live injected client reachable through
  named-pipe IPC
- the trophy panel also exposes live telemetry for active crafting sessions,
  chosen equip slot, displaced item restore target, and remaining charges when
  the client can observe them
- these settings reset to defaults when the web process restarts
  unless a future persistence layer is added

## Loot Scoring

The loot dashboard exposes an **Item Score** tab that applies
MQ2ItemScore-style stat weighting when TextQuest compares a looted item against
the currently equipped item in the same slot.

Operator surface:

- `GET /api/loot/item-score`
- `PUT /api/loot/item-score`
- the web dashboard **Loot Config** panel under the **Item Score** tab

Current behavior:

- stat weights are configured per EverQuest class
- weights can include core stats such as `STR`, `AGI`, `STA`, `DEX`, `WIS`,
  `INT`, `CHA`, `HP`, `MANA`, and any additional stat keys exposed by item data
- `min_upgrade_delta` defines how much positive weighted score a candidate item
  needs before the loot engine treats it as a keep-worthy upgrade
- explicit loot rules still win first; item scoring is the fallback path when
  no hard keep, sell, or destroy rule matches

The persisted backend shape lives in `textquest-web/src/api/loot.rs`, and the
comparison engine used by the loot module lives in
`textquest/src/loot/item_score.rs`.

## Inventory Utility Parity

The loot dashboard also exposes an **Inventory Utilities** tab that groups the
remaining RedGuides item-knowledge, cursor, reward, collection, vendor-watch,
consumption, trophy, and relocation adapters behind one shared config model.

Operator surface:

- `GET /api/loot/inventory-utility`
- `PUT /api/loot/inventory-utility`
- the web dashboard **Loot Config** panel under the **Inventory Utilities** tab

Current behavior:

- explicit plugin mappings, owners, and status badges are shown for every
  plugin in the item-knowledge and inventory-utility parity pack
- provenance and unsupported-field warnings are displayed for adapted legacy
  settings such as `MQ2LinkDB`, `MQ2Cursor`, and `MQ2PortalSetter`
- cursor handling, collection routing, reward routing, food and drink
  consumption, vendor watch alerts, relocation-item retention, tradeskill
  trophy preferences, and auto-claim policy are editable in the local web UI
- the config persists alongside item-score weights in `config/textquest.toml`
  under the `[inventory_utility]` table

See [Inventory Utility Parity](Inventory-Utility-Parity) for the per-plugin
native versus adapted classification and ownership table.

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

## Chat Log Settings

The `[chat_log]` section in `config/textquest.toml` controls in-game chat logging.

```toml
[chat_log]
enabled = true
rotation = "daily"       # "none", "daily", or "by_size:<bytes>"
level = "info"           # "info" or "debug"
channels = ["say", "tell", "tell_out", "group", "guild", "raid", "shout", "ooc", "auction"]
```

- Settings can also be read and written via the web API at `GET/PUT /api/chat_log/settings`.
- `rotation = "by_size:10485760"` rotates when the log file exceeds 10 MB.
- `level = "debug"` includes additional diagnostic output in log entries.
- Toon-specific combat overrides live in `config/toons/<ToonName>.toml` (see `Class-Combat-Rotations.md`).
- The live orchestrator polls each registered client with `PollChat`, writes any returned `ChatBatch` messages through `ChatLogManager`, and updates per-character files under the configured `log_dir`.
- The chat writer uses a shared 8KB `BufWriter` per file. Messages are appended to the file buffer and flushed on rotation or explicit close, not on every message write.
- Log files are named `<server_charactername>.log` and stored in the configured log directory (default `logs/`).
