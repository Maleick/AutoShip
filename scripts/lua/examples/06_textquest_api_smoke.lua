-- TextQuest Lua API smoke script.
-- Demonstrates the current lower-case `textquest` API exposed by mlua.

local tq = require("textquest")

local smoke = {}

function smoke.report_player()
  local player = tq.get_player()
  tq.log("info", string.format(
    "Lua player snapshot: %s level %d hp %.1f%% at %.1f %.1f %.1f",
    player.name,
    player.level,
    player.hp_percent,
    player.x,
    player.y,
    player.z
  ))
end

function smoke.register_events()
  tq.events.on("hp_change", function(data)
    local delta = data and data.delta or 0
    tq.log("debug", string.format("hp_change event received: %s", tostring(delta)))
  end)
end

function smoke.queue_actions()
  tq.nav["goto"](10.0, 20.0, 5.0)
  tq.commands.execute("/sit")
end

smoke.report_player()
smoke.register_events()

return smoke
