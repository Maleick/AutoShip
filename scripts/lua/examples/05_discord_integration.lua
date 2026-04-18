-- TextQuest Discord Integration Script
-- Demonstrates: webhook posting, status updates, alerts, raid notifications
-- Usage: Configure webhook URL and let script send updates to Discord
--
-- This script sends real-time updates to Discord about game events,
-- allowing operators to monitor automation from Discord even when away
-- from the TUI dashboard.

local DiscordIntegration = {}

-- Configuration
local CONFIG = {
  -- Discord webhook URL (configure via TextQuest API)
  webhook_url = nil,

  -- Bot username shown in Discord
  bot_name = "TextQuest",

  -- Color codes (Discord embed colors in decimal)
  colors = {
    info = 3447003,        -- Blue
    warning = 15158332,    -- Orange
    success = 3066993,     -- Green
    error = 15746113,      -- Red
  },

  -- What events to report
  report_combat_start = true,
  report_death = true,
  report_levelup = true,
  report_loot = true,
  report_raid_events = true,
  report_errors = true,

  -- Throttling - don't spam the same event type
  throttle_seconds = 5,
  throttle_events = {},
}

-- Internal state
local state = {
  last_message_time = 0,
  queue = {},           -- Message queue for batch sending
  connected = false,
}

--- Initialize Discord integration
function DiscordIntegration.init()
  print("[Discord] Initializing Discord integration...")

  -- Get webhook URL from TextQuest config
  if TextQuest and TextQuest.get_config then
    CONFIG.webhook_url = TextQuest.get_config("discord_webhook_url")
    if CONFIG.webhook_url then
      state.connected = true
      print("[Discord] Webhook configured. Ready to post updates.")
    else
      print("[Discord] WARNING: No webhook URL configured. Discord features disabled.")
    end
  end

  -- Register event handlers
  if TextQuest then
    if TextQuest.on_combat_start then
      TextQuest.on_combat_start(function()
        DiscordIntegration.report_combat_start()
      end)
    end

    if TextQuest.on_death then
      TextQuest.on_death(function(death_info)
        DiscordIntegration.report_death(death_info)
      end)
    end

    if TextQuest.on_loot then
      TextQuest.on_loot(function(item)
        DiscordIntegration.report_loot(item)
      end)
    end
  end

  print("[Discord] Ready.")
end

--- Send a message to Discord
function DiscordIntegration.send_message(title, description, color, fields)
  if not state.connected or not CONFIG.webhook_url then
    return false
  end

  local now = os.time()

  -- Check throttling to prevent spam
  if now - state.last_message_time < 1 then
    table.insert(state.queue, {
      title = title,
      description = description,
      color = color,
      fields = fields,
    })
    return true  -- Queued
  end

  local embed = {
    title = title,
    description = description,
    color = color or CONFIG.colors.info,
    timestamp = os.date("!%Y-%m-%dT%H:%M:%SZ"),
  }

  if fields and #fields > 0 then
    embed.fields = fields
  end

  local payload = {
    username = CONFIG.bot_name,
    embeds = { embed },
  }

  -- Send via TextQuest API
  if TextQuest and TextQuest.http_post then
    local success = TextQuest.http_post(CONFIG.webhook_url, payload)
    if success then
      state.last_message_time = now
      print("[Discord] Message sent: " .. title)
    else
      print("[Discord] Failed to send message")
    end
    return success
  end

  return false
end

--- Report combat start
function DiscordIntegration.report_combat_start()
  if not CONFIG.report_combat_start then
    return
  end

  if DiscordIntegration.is_throttled("combat") then
    return
  end

  local current_target = TextQuest.get_current_target()
  local target_name = current_target and current_target.name or "Unknown"

  DiscordIntegration.send_message(
    "Combat Started",
    string.format("Engaging **%s**", target_name),
    CONFIG.colors.warning,
    {
      {
        name = "Target",
        value = target_name,
        inline = true,
      },
    }
  )

  DiscordIntegration.set_throttle("combat")
end

--- Report character death
function DiscordIntegration.report_death(death_info)
  if not CONFIG.report_death then
    return
  end

  local location = death_info.zone or "Unknown Zone"
  local killer = death_info.killer_name or "Unknown"

  DiscordIntegration.send_message(
    "Character Died",
    string.format("Death in **%s**", location),
    CONFIG.colors.error,
    {
      {
        name = "Location",
        value = location,
        inline = true,
      },
      {
        name = "Killer",
        value = killer,
        inline = true,
      },
    }
  )

  DiscordIntegration.set_throttle("death")
end

--- Report loot received
function DiscordIntegration.report_loot(item)
  if not CONFIG.report_loot then
    return
  end

  if DiscordIntegration.is_throttled("loot") then
    return
  end

  local value = item.value or 0
  local description = string.format(
    "Looted: **%s** (%dp)",
    item.name, value
  )

  DiscordIntegration.send_message(
    "Loot Received",
    description,
    CONFIG.colors.success,
    {
      {
        name = "Item",
        value = item.name,
        inline = true,
      },
      {
        name = "Value",
        value = string.format("%dp", value),
        inline = true,
      },
    }
  )

  DiscordIntegration.set_throttle("loot")
end

--- Report error/alert
function DiscordIntegration.report_error(error_message, error_type)
  if not CONFIG.report_errors then
    return
  end

  error_type = error_type or "Error"

  DiscordIntegration.send_message(
    error_type,
    error_message,
    CONFIG.colors.error,
    {
      {
        name = "Timestamp",
        value = os.date("%Y-%m-%d %H:%M:%S"),
        inline = false,
      },
    }
  )
end

--- Report raid event (boss kill, wipe, etc)
function DiscordIntegration.report_raid_event(event_type, description, details)
  if not CONFIG.report_raid_events then
    return
  end

  local color = CONFIG.colors.info
  if event_type == "KILL" then
    color = CONFIG.colors.success
  elseif event_type == "WIPE" then
    color = CONFIG.colors.error
  end

  DiscordIntegration.send_message(
    "Raid Event: " .. event_type,
    description,
    color,
    details
  )
end

--- Send fleet status update
function DiscordIntegration.send_fleet_status()
  if not state.connected then
    return false
  end

  local fleet = TextQuest.get_fleet_status()
  if not fleet then
    return false
  end

  local fields = {
    {
      name = "Total Clients",
      value = tostring(fleet.total_clients),
      inline = true,
    },
    {
      name = "In Combat",
      value = tostring(fleet.in_combat),
      inline = true,
    },
    {
      name = "Average Level",
      value = string.format("%.1f", fleet.avg_level),
      inline = true,
    },
  }

  DiscordIntegration.send_message(
    "Fleet Status Update",
    string.format("%d/%d clients active", fleet.online_clients, fleet.total_clients),
    CONFIG.colors.info,
    fields
  )

  return true
end

--- Check if event type is throttled
function DiscordIntegration.is_throttled(event_type)
  local last = CONFIG.throttle_events[event_type]
  if not last then
    return false
  end

  return (os.time() - last) < CONFIG.throttle_seconds
end

--- Set throttle for event type
function DiscordIntegration.set_throttle(event_type)
  CONFIG.throttle_events[event_type] = os.time()
end

--- Configure webhook URL
function DiscordIntegration.set_webhook_url(url)
  CONFIG.webhook_url = url
  state.connected = (url ~= nil and url ~= "")
  print(string.format("[Discord] Webhook URL configured: %s", url and "YES" or "NO"))
end

--- Toggle reporting feature
function DiscordIntegration.toggle_feature(feature_name)
  local key = "report_" .. feature_name
  if CONFIG[key] ~= nil then
    CONFIG[key] = not CONFIG[key]
    print(string.format("[Discord] %s reporting %s",
      feature_name, CONFIG[key] and "ENABLED" or "DISABLED"))
  end
end

--- Get connection status
function DiscordIntegration.get_status()
  return {
    connected = state.connected,
    webhook_configured = (CONFIG.webhook_url ~= nil),
    queued_messages = #state.queue,
    throttle_events = CONFIG.throttle_events,
  }
end

--- Send queued messages
function DiscordIntegration.flush_queue()
  while #state.queue > 0 do
    local msg = table.remove(state.queue, 1)
    DiscordIntegration.send_message(msg.title, msg.description, msg.color, msg.fields)
  end
end

--- Send a test message
function DiscordIntegration.send_test()
  DiscordIntegration.send_message(
    "Test Message",
    "This is a test message from TextQuest Lua integration",
    CONFIG.colors.info
  )
end

return DiscordIntegration
