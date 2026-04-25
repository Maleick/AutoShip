# Adding Custom Hotkeys and Commands

TextQuest supports script and plugin-defined slash commands plus hotkey bindings through shared registries in `textquest::registry`.

## Hotkey syntax and format

Hotkeys are parsed as a `+`-separated list of optional modifiers plus one primary key:

- Modifiers: `Ctrl`, `Alt`, `Shift`
- Primary key examples: `F1`..`F12`, `A`..`Z`, `0`..`9`, `Enter`, `Tab`, `Esc`, `Home`, `End`, arrow keys, `PageUp`, `PageDown`
- Parsing is case-insensitive and canonicalized when stored

Examples:

```lua
"ctrl+f5"
"Alt+Shift+F10"
"shift+ctrl+f12"
"Enter"
```

Invalid examples:

- missing key: `"ctrl+alt"`
- duplicate modifier: `"ctrl+Ctrl+f5"`
- unsupported modifier: `"meta+f5"`

`ScriptHotkeyRegistry` dispatch prefers an exact character scope match, then global entries.

## Lua hotkey API

`textquest.hotkeys.register(combo, callback)` stores a registry ID and fires `callback` when the combo is pressed.

```lua
-- Example: simple hotkey callback
local hotkey_id = textquest.hotkeys.register("ctrl+f5", function()
  textquest.commands.execute("/assist")
  textquest.log.info("assist hotkey triggered")
end)

-- Optional cleanup
textquest.hotkeys.unregister(hotkey_id)
```

For non-game testing, Lua also has a local debug helper:

```lua
textquest.hotkeys.fire("ctrl+f5")
```

## Lua command registration API

`textquest.commands.register(path, callback)` registers a local callback for one slash command path.

```lua
local command_id = textquest.commands.register("/farm", function(args)
  -- args is the normalized tail after "/farm"
  textquest.log.info("farm command args: " .. tostring(args))
end)

-- Returns true/false if a matching registration exists
textquest.commands.dispatch("/farm pull")

-- Send a slash command to the orchestrator for execution
textquest.commands.execute("/sit")
textquest.execute_command("/all /stand")
```

Unregistering by ID removes the callback immediately:

```lua
textquest.commands.unregister(command_id)
```

## Plugin API

MQ2-style plugins use the same underlying registry through the plugin ABI:

```c
uint64_t tq_register_hotkey(const char* combo, void (*callback)(void));
uint64_t tq_register_command(const char* path, void (*callback)(const char*));
int      tq_unregister_hotkey(uint64_t id);
int      tq_unregister_command(uint64_t id);
```

Plugin registrations are shared with Lua and can be cleaned up together when a plugin unloads.

- `tq_register_command` paths must start with `/`
- command callbacks receive the argument tail as a C string
- both APIs validate conflicts against scope-aware registry state

## Example script source

See `scripts/lua/examples/06_textquest_api_smoke.lua` for a runnable example using `textquest.hotkeys` and `textquest.commands`.
