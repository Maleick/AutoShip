# Setting up Clickies

This page explains how TextQuest runs automatic clicky items, what each clicky field means, and how to enable/override rules per character.

## Where clicky configs live

TextQuest reads clicky rules from a directory such as `config/clickies/`.

- `global.toml` (optional): shared clicky rules for all characters.
- `<CharacterName>.toml` (optional): character-specific rules merged after `global.toml`.

## Clicky evaluation order

1. A character snapshot is evaluated against each configured item.
2. The item must be enabled and match the combat window.
3. All conditions must pass.
4. The item must be off cooldown for that character.
5. If all checks pass, TextQuest dispatches `/useitem <slot>`.

Items are also sorted by `priority` (low number first) before evaluation, and cooldown is tracked per `(character_id, item_name)`.

## Configuration format

A config file is a TOML list of `[[clicky]]` entries.

```toml
[[clicky]]
name = "Name in logs"
slot = 5
cooldown_ms = 30000
priority = 10
enabled = true
scenario = "any"          # any | combat | downtime
character_id = 1002        # optional override to one character ID

[[clicky.conditions]]
hp_pct_max = 35.0
mana_pct_max = 30.0
missing_buffs = ["Haste", "Swift Like the Wind"]
```

### Scenario and legacy fields

`scenario` is the modern field:
- `any`
- `combat`
- `downtime`

Older compatibility examples may still use `combat_only` / `ooc_only`. Both are supported when `scenario` is not set.

## Example config files

- `config/clickies/global.toml` (shared defaults)
- `config/clickies/example_warrior.toml`
- `config/clickies/example_enchanter.toml`

Those files are intentionally small and safe to copy/rename for per-character customization.

## Lua examples

There is no dedicated `textquest.clicky` API yet, so script callers use IPC command dispatch.

### Example: emergency heal when low HP

```lua
local hp_pct = textquest.player.get_hp_percent()
if hp_pct <= 30 then
  textquest.ipc.send("/useitem 5")
end
```

### Example: one cooldown-gated mana clicky helper

```lua
local mana_pct = textquest.player.get_mana_percent()
local last_use = 0

if mana_pct < 50 and os.time() - last_use >= 45 then
  textquest.ipc.send("/useitem 3")
  last_use = os.time()
end
```

## Validation checklist

- Confirm the TOML parser can load your chosen file.
- Confirm legacy filenames are copied/renamed correctly (for example `Warrior01.toml`).
- Confirm one rule fires at a time in dry conditions before enabling broad production usage.
- Check logs for `/useitem` activity when combat state changes.
