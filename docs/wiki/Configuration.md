# Configuration

## Current Config Files

| Path | Purpose |
| --- | --- |
| `config/frostreaver.toml` | Main TextQuest app config |
| `config/accounts.toml` | Account and group-launch metadata |
| `config/camps/*.toml` | Saved camp locations and thresholds |
| `config/classes/*.toml` | Per-class combat and ability config |
| `config/toons/*.toml` | Per-toon combat action overrides for the injected DLL |
| `config/hvt_watchlist.toml` | High-value target watchlist |
| `config/named_mobs/*.toml` | Named spawn definitions by zone |
| `config/maps/*.txt` | Brewall-style zone map data |
| `config/offsets.json` | Hot-updatable offsets database |

## Main App Config

The current main app file is still named `config/frostreaver.toml`. That filename is historical, but it is the active TextQuest config path today.

Current sections include:

- `process_name`
- `max_spawns`
- `[launch]`
- `[server]`
- `[retry]`
- `[soul]`
- `[[group]]`
- Discord-related options

## Accounts

`config/accounts.toml` maps login accounts to:

- account name
- server
- character
- class
- group

Passwords are intentionally not stored here.

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
- `config/offsets.json` is the hot-updatable offsets store that complements the compiled constants in `textquest-common/src/offsets.rs`.

## Internals

- App config types are defined in `textquest/src/config.rs`.
- Accounts config loads into `AccountsConfig` and is used by TUI `:login` flows.
- Soul config types are defined in `textquest/src/soul/config.rs`.

## Current Behavior vs Roadmap

### Current behavior

- The current configuration surface is already large enough that docs should point to specific files, not only describe it abstractly.
- The legacy filename `frostreaver.toml` is still current and should not be silently renamed in docs unless the code changes too.

### Validation notes

- When features change, update both the code and the relevant sample config comments so the wiki does not drift from what operators actually edit.
