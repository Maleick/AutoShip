-- TextQuest Navigation Assistant Script
-- Demonstrates: waypoint recording, route planning, safe path navigation
-- Usage: Configure waypoints and routes, then guide group movement
--
-- This script records waypoints during exploration and helps plan safe
-- routes through dangerous zones. Routes can be saved and replayed.

local NavigationAssistant = {}

-- Waypoint storage (persisted via TextQuest API)
local state = {
  waypoints = {},        -- {zone, x, y, z, label, timestamp}
  routes = {},           -- Named routes containing waypoint sequences
  current_route = nil,
  current_waypoint_idx = 1,
  is_recording = false,
  safe_radius = 50,      -- Distance considered "arrived" at waypoint
}

-- Configuration
local CONFIG = {
  enable_auto_pathing = false,
  enable_auto_pull = false,
  path_smoothing = true,
  max_waypoint_distance = 500,  -- Warn if waypoints too far apart
}

--- Initialize navigation assistant
function NavigationAssistant.init()
  print("[NavigationAssistant] Initializing navigation assistant...")

  -- Load saved waypoints and routes from TextQuest persistence
  if TextQuest and TextQuest.load_data then
    local saved = TextQuest.load_data("nav_waypoints")
    if saved then
      state.waypoints = saved
      print(string.format("[NavigationAssistant] Loaded %d waypoints", #saved))
    end

    local saved_routes = TextQuest.load_data("nav_routes")
    if saved_routes then
      state.routes = saved_routes
      print(string.format("[NavigationAssistant] Loaded %d routes", #saved_routes))
    end
  end

  print("[NavigationAssistant] Ready.")
end

--- Start recording waypoints
function NavigationAssistant.start_recording()
  state.is_recording = true
  print("[NavigationAssistant] Recording waypoints...")
end

--- Stop recording waypoints
function NavigationAssistant.stop_recording()
  state.is_recording = false
  print(string.format("[NavigationAssistant] Stopped recording. Total: %d waypoints", #state.waypoints))
end

--- Record current position as waypoint
function NavigationAssistant.record_waypoint(label)
  if not TextQuest or not TextQuest.get_position then
    print("[NavigationAssistant] TextQuest API not available")
    return
  end

  local pos = TextQuest.get_position()
  local zone = TextQuest.get_current_zone()

  if not pos or not zone then
    print("[NavigationAssistant] Could not get position")
    return
  end

  local waypoint = {
    zone = zone,
    x = pos.x,
    y = pos.y,
    z = pos.z,
    label = label or string.format("wp_%d", #state.waypoints + 1),
    timestamp = os.time(),
  }

  -- Check distance from last waypoint
  local last_wp = state.waypoints[#state.waypoints]
  if last_wp then
    local dist = NavigationAssistant.distance(last_wp, waypoint)
    if dist > CONFIG.max_waypoint_distance then
      print(string.format("[NavigationAssistant] WARNING: Large gap between waypoints (%.1f units)", dist))
    end
  end

  table.insert(state.waypoints, waypoint)
  print(string.format("[NavigationAssistant] Recorded waypoint: %s at (%.1f, %.1f, %.1f)",
    waypoint.label, pos.x, pos.y, pos.z))

  -- Auto-save to persistent storage
  if TextQuest and TextQuest.save_data then
    TextQuest.save_data("nav_waypoints", state.waypoints)
  end

  return waypoint
end

--- Create a named route from current waypoints
function NavigationAssistant.save_route(route_name)
  if #state.waypoints == 0 then
    print("[NavigationAssistant] No waypoints to save")
    return false
  end

  state.routes[route_name] = {
    name = route_name,
    waypoints = {},
    created = os.time(),
    zone = state.waypoints[1].zone,
  }

  -- Copy waypoints into route
  for _, wp in ipairs(state.waypoints) do
    table.insert(state.routes[route_name].waypoints, {
      x = wp.x,
      y = wp.y,
      z = wp.z,
      label = wp.label,
    })
  end

  print(string.format("[NavigationAssistant] Saved route: %s (%d waypoints)",
    route_name, #state.routes[route_name].waypoints))

  if TextQuest and TextQuest.save_data then
    TextQuest.save_data("nav_routes", state.routes)
  end

  return true
end

--- Load and start following a route
function NavigationAssistant.load_route(route_name)
  if not state.routes[route_name] then
    print(string.format("[NavigationAssistant] Route not found: %s", route_name))
    return false
  end

  state.current_route = state.routes[route_name]
  state.current_waypoint_idx = 1

  print(string.format("[NavigationAssistant] Loaded route: %s. Starting navigation...",
    route_name))

  return true
end

--- Navigate group to next waypoint
function NavigationAssistant.navigate_to_next()
  if not state.current_route then
    print("[NavigationAssistant] No route loaded")
    return false
  end

  local current_wp = state.current_route.waypoints[state.current_waypoint_idx]
  if not current_wp then
    print("[NavigationAssistant] Route complete!")
    state.current_route = nil
    return false
  end

  if not TextQuest or not TextQuest.move_to then
    print("[NavigationAssistant] TextQuest API not available")
    return false
  end

  -- Move player to waypoint
  local success = TextQuest.move_to(current_wp.x, current_wp.y, current_wp.z)

  if success then
    print(string.format("[NavigationAssistant] Moving to %s (%.1f, %.1f, %.1f)",
      current_wp.label, current_wp.x, current_wp.y, current_wp.z))
  end

  return success
end

--- Check if current waypoint reached
function NavigationAssistant.check_waypoint_reached()
  if not state.current_route then
    return false
  end

  local current_wp = state.current_route.waypoints[state.current_waypoint_idx]
  if not current_wp then
    return false
  end

  local pos = TextQuest.get_position()
  if not pos then
    return false
  end

  local dist = math.sqrt(
    (pos.x - current_wp.x) ^ 2 +
    (pos.y - current_wp.y) ^ 2 +
    (pos.z - current_wp.z) ^ 2
  )

  if dist <= CONFIG.safe_radius then
    print(string.format("[NavigationAssistant] Reached %s", current_wp.label))
    state.current_waypoint_idx = state.current_waypoint_idx + 1
    return true
  end

  return false
end

--- Calculate distance between two points
function NavigationAssistant.distance(p1, p2)
  return math.sqrt(
    (p1.x - p2.x) ^ 2 +
    (p1.y - p2.y) ^ 2 +
    (p1.z - p2.z) ^ 2
  )
end

--- List all saved routes
function NavigationAssistant.list_routes()
  print("[NavigationAssistant] Saved Routes:")
  for name, route in pairs(state.routes) do
    print(string.format("  %s: %d waypoints in %s", name, #route.waypoints, route.zone))
  end
end

--- Delete a waypoint
function NavigationAssistant.delete_waypoint(index)
  if index < 1 or index > #state.waypoints then
    print("[NavigationAssistant] Invalid waypoint index")
    return false
  end

  local removed = table.remove(state.waypoints, index)
  print(string.format("[NavigationAssistant] Deleted waypoint: %s", removed.label))

  if TextQuest and TextQuest.save_data then
    TextQuest.save_data("nav_waypoints", state.waypoints)
  end

  return true
end

--- Export route as JSON for sharing
function NavigationAssistant.export_route(route_name)
  if not state.routes[route_name] then
    print("[NavigationAssistant] Route not found")
    return nil
  end

  local route = state.routes[route_name]
  local json = string.format(
    '{"name":"%s","zone":"%s","waypoints":[',
    route.name, route.zone
  )

  for i, wp in ipairs(route.waypoints) do
    if i > 1 then json = json .. "," end
    json = json .. string.format('{"x":%.1f,"y":%.1f,"z":%.1f,"label":"%s"}',
      wp.x, wp.y, wp.z, wp.label)
  end

  json = json .. "]}"

  return json
end

return NavigationAssistant
