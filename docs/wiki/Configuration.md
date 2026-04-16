# Configuration

## Current Config Files

| Path | Purpose |
| --- | --- |
| `config/textquest.toml` | Main TextQuest app config |
| `config/accounts.toml` | Account and group-launch metadata |
| `data/credentials.db` | Encrypted account password store used by `textquest autologin` |
| `data/trade_prices.db` | Local SQLite store for passive `/ooc` and `/auction` Krono price observations |
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
- `[discovery]`
- `[box_chat]`
- `[soul]`
- `[[group]]`
- Discord-related options

## Discord Webhooks

TextQuest supports Discord webhook delivery for both category feeds and
operational alert routes.

The canonical operator surface is:

- `config/textquest.toml` for static config
- the web dashboard `Security Wards` view for webhook URL management and
  per-event delivery policy

The Discord config model is split into three parts:

- `[discord]`
  - `webhook_url` is the fallback webhook used when no more specific route is set
  - `alert_hvt`, `alert_crashes`, `alert_mass_failures`, and `alert_status`
    gate whether those alert families are emitted at all
- `[discord.channels]`
  - category feeds keyed by `kills`, `loot`, `timers`, `feats`, and `status`
  - blank values fall back to `discord.webhook_url`
- `[discord.notification_routes.<route>]`
  - stable route keys: `death`, `status`, `hvt`, `crash`, `mass_failure`
  - each route can override:
    - `enabled`
    - `webhook_url`
    - `level`
    - `message_mode`
    - `mention_policy`

Accepted route values are:

- `level`: `INFO`, `WARNING`, `ERROR`, `CRITICAL`
- `message_mode`: `rich_embed`, `plain_text`
- `mention_policy`: `none`, `everyone`

Current defaults are intentionally opinionated:

- `death` defaults to `CRITICAL` with `mention_policy = "everyone"`
- `status` defaults to `INFO` with `mention_policy = "none"`
- all other operational routes default to `CRITICAL`

The webhook sender enforces Discord's per-webhook delivery ceiling of 30
requests per minute, so splitting high-volume feeds across different webhook
URLs also increases available throughput.

### `[box_chat]`

`[box_chat]` enables the EQBC-style TCP relay used for cross-machine `/bc`,
`/bca`, `/bcaa`, and `/bct` command delivery.

Example:

```toml
[box_chat]
enabled = true
host = "192.168.1.25"
port = 2112
auto_connect = true
```

Fields:

- `enabled`: start the local box-chat runtime and listen on `port`
- `host`: relay server host to connect to when `auto_connect = true`
- `port`: shared TCP port used by the listener and connector
- `auto_connect`: maintain an outbound connection to `host:port`

Runtime behavior:

- TextQuest long-lived processes poll `config/textquest.toml` and apply
  `[box_chat]` changes without restart.
- One machine can run as the relay server with `enabled = true` and
  `auto_connect = false`.
- Other machines can point `host` at that relay server and set
  `auto_connect = true`.

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

## Web Strategy Tuning

The Strategy Tuning panel and `/api/config/characters` expose a per-character web configuration surface.

Current fields include:

- heal, mana-sit, and nuke thresholds
- ordered rotation entries
- class-specific strategy parameters
- group override metadata
- auto-rez policy

The auto-rez policy currently covers:

- `enabled`
- `min_xp_pct`
- `trusted_casters`
- `decline_if_untrusted`
- `delay_ms`

Tribute automation is also exposed per character. The dashboard currently carries:

- tribute auto-activate toggle
- tribute warning lead time in seconds
- preferred tribute list
- live tribute status snapshot:
  - active/inactive state
  - time remaining
  - current tribute point balance
  - current active tribute list
  - alert state (`ok`, `expiring`, or `expired`)

The live demo/API shape for those settings is served from `textquest-web/src/api.rs` via `/api/config/characters` and `/api/config/characters/:name`.

This state currently lives in memory inside `textquest-web`. It is available to the running dashboard process, but it is not yet persisted across backend restarts.

## Maps and Offsets

- `config/maps/*.txt` supplies zone linework and labels for the TUI map.
- `config/offsets.json` is a maintained offset-data snapshot that matches the `textquest-common/src/offset_db.rs` schema. The compiled defaults still live in `textquest-common/src/offsets.rs`.

## Local Ghidra Caches

- `data/ghidra.db` and `data/ghidra-export/` are local runtime/debug caches only.
- Canonical manifests, snapshot variants, baseline selection, and copied evidence live in the sibling `Maleick/TextQuest-Ghidra` repo under `snapshots/` and `baseline-selection/current.json`.
- If a runbook needs durable evidence, link to the canonical `TextQuest-Ghidra` snapshot path rather than copying payload into this repo.

## Passive Trade Price Cache

- `data/trade_prices.db` is a repo-local SQLite database populated from the DLL's existing passive chat capture path.
- The orchestrator polls accumulated `PollChat` batches, filters `/ooc` and `/auction` messages to trade hub zones, and stores Krono-denominated item sightings for later trend analysis.
- This cache is mutable runtime state, not canonical evidence. Treat it like other local operator data stores and rebuild it from live captures when needed.

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
