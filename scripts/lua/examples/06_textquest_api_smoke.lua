-- TextQuest Lua core API smoke example.
-- Demonstrates combat, navigation, state, config, IPC, events, and logging.

local function run()
  local player_hp_pct = textquest.player.get_hp_percent()
  textquest.log.info("player hp%: " .. player_hp_pct)

  local group_count = textquest.group.get_member_count()
  local tank = textquest.group.get_tank()
  textquest.log.debug("group members: " .. tostring(group_count) .. ", tank: " .. tostring(tank))

  textquest.nav['goto'](10.0, 20.0, 30.0)
  textquest.nav.add_waypoint(10.0, 20.0, 30.0, "spawn")
  textquest.nav.stop()
  local waypoints = textquest.nav.get_waypoints()
  local stuck = textquest.nav.is_stuck()
  if stuck then
    textquest.log.warn("navigation stuck: " .. tostring(textquest.nav.stuck_reason()))
  end

  local ok_cast = textquest.combat.cast("Fire Bolt", nil)
  local ok_target = textquest.combat.set_target("Rathyl")
  local has_buff = textquest.combat.has_buff("Haste")
  textquest.log.debug("combat cast=" .. tostring(ok_cast) .. ", target=" .. tostring(ok_target) .. ", haste=" .. tostring(has_buff))

  local spawns = textquest.state.get_spawns()
  local nearest = textquest.state.find_spawns("orc")
  local target = textquest.state.get_target()
  textquest.log.info("spawns=" .. tostring(#spawns) .. ", filtered=" .. tostring(#nearest) .. ", target=" .. tostring(target and target.name))

  local ok_set = textquest.config.set("combat.mode", "assist")
  local combat_mode = textquest.config.get("combat.mode")
  local saved = textquest.config.save()
  local reloaded = textquest.config.reload()
  textquest.log.debug("config ok=" .. tostring(ok_set) .. ", mode=" .. tostring(combat_mode) .. ", saved=" .. tostring(saved) .. ", reload=" .. tostring(reloaded))

  textquest.ipc.broadcast("/say hello")
  textquest.ipc.send("/follow", "box-7")

  local unsubscribed = 0
  local handler = textquest.events.on("combat", function(payload)
    unsubscribed = unsubscribed + 1
    textquest.log.debug("combat event: " .. tostring(payload.type))
  end)
  unsubscribed = textquest.events.emit("combat", { type = "assist", target = "Rathyl" })
  unsubscribed = textquest.events.off("combat")
end

run()

