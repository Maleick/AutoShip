-- TextQuest Buff Manager Script
-- Demonstrates: buff tracking, duration monitoring, smart recasts
-- Usage: Configure buff timers and spell names, let script maintain buffs
--
-- This script tracks active buffs, monitors their durations, and automatically
-- casts replacement buffs before they expire. Useful for maintaining utility
-- buffs and stance buffs during extended gameplay.

local BuffManager = {}

-- Configuration - customize per class
local CONFIG = {
  -- Enable/disable autocast
  enable_auto_recast = true,

  -- Buff definitions: {name, spell, duration_seconds, recast_before_expire}
  buffs_to_maintain = {
    {
      name = "haste",
      spell = "Spirit of the Cheetah",
      duration = 2400,     -- 40 minutes
      recast_margin = 300, -- Recast 5 minutes before expiry
    },
    {
      name = "strength",
      spell = "Strength of the Kunark",
      duration = 1800,
      recast_margin = 200,
    },
    {
      name = "mana_regen",
      spell = "Mana Regen",
      duration = 2400,
      recast_margin = 300,
    },
    {
      name = "shield",
      spell = "Shielding",
      duration = 3600,
      recast_margin = 400,
    },
  },

  -- Prevention - don't cast if these conditions exist
  prevent_cast_in_combat = false,
  prevent_cast_in_spell_gem_cooldown = true,

  -- Logging
  enable_logging = true,
  log_recasts = true,
}

-- Internal state
local state = {
  active_buffs = {},        -- {spell_name, duration, cast_time, expires_at}
  last_recast_time = {},    -- Prevent spam-casting same buff
  spell_gem_cooldown = 0,
}

--- Initialize buff manager
function BuffManager.init()
  print("[BuffManager] Initializing buff manager...")

  -- Register event handlers
  if TextQuest and TextQuest.on_buff_added then
    TextQuest.on_buff_added(function(buff_info)
      BuffManager.on_buff_added(buff_info)
    end)
  end

  if TextQuest and TextQuest.on_buff_removed then
    TextQuest.on_buff_removed(function(buff_name)
      BuffManager.on_buff_removed(buff_name)
    end)
  end

  if TextQuest and TextQuest.on_spell_cast_complete then
    TextQuest.on_spell_cast_complete(function()
      BuffManager.on_spell_complete()
    end)
  end

  print("[BuffManager] Ready. Will maintain buffs every 30 seconds.")
end

--- Called when a buff is detected
function BuffManager.on_buff_added(buff_info)
  state.active_buffs[buff_info.name] = {
    spell_name = buff_info.name,
    duration = buff_info.duration,
    cast_time = os.time(),
    expires_at = os.time() + buff_info.duration,
  }

  if CONFIG.enable_logging then
    print(string.format("[BuffManager] Buff detected: %s (expires in %d seconds)",
      buff_info.name, buff_info.duration))
  end
end

--- Called when a buff expires
function BuffManager.on_buff_removed(buff_name)
  state.active_buffs[buff_name] = nil

  if CONFIG.enable_logging then
    print(string.format("[BuffManager] Buff expired: %s", buff_name))
  end
end

--- Called when a spell finishes casting
function BuffManager.on_spell_complete()
  -- Clear cooldown state
  state.spell_gem_cooldown = os.time() + 2
end

--- Periodically check buffs and recast if needed
function BuffManager.maintain_buffs()
  local now = os.time()

  -- Check spell gem cooldown
  if state.spell_gem_cooldown > now then
    return  -- Still in global cooldown
  end

  -- Iterate through configured buffs
  for _, buff_config in ipairs(CONFIG.buffs_to_maintain) do
    local active = state.active_buffs[buff_config.name]
    local should_recast = false

    if not active then
      -- Buff not active, need to cast
      should_recast = true
    else
      -- Check if expiring soon
      local time_until_expire = active.expires_at - now
      if time_until_expire <= buff_config.recast_margin then
        should_recast = true
      end
    end

    if should_recast then
      BuffManager.recast_buff(buff_config)
    end
  end
end

--- Recast a single buff
function BuffManager.recast_buff(buff_config)
  if not TextQuest or not TextQuest.cast_spell then
    return false
  end

  -- Check last recast time (prevent spam)
  local now = os.time()
  if state.last_recast_time[buff_config.name] then
    local time_since_last = now - state.last_recast_time[buff_config.name]
    if time_since_last < 30 then  -- 30 second throttle
      return false
    end
  end

  -- Check combat status if configured
  if CONFIG.prevent_cast_in_combat then
    if TextQuest.in_combat and TextQuest.in_combat() then
      return false
    end
  end

  -- Check spell gem cooldown
  if CONFIG.prevent_cast_in_spell_gem_cooldown then
    if state.spell_gem_cooldown > os.time() then
      return false
    end
  end

  -- Cast the buff
  local success = TextQuest.cast_spell(buff_config.spell, "self")

  if success then
    state.last_recast_time[buff_config.name] = now
    if CONFIG.log_recasts then
      print(string.format("[BuffManager] Recasting %s", buff_config.spell))
    end
  end

  return success
end

--- Get list of currently active buffs
function BuffManager.get_active_buffs()
  local buffs = {}
  local now = os.time()

  for name, buff in pairs(state.active_buffs) do
    local remaining = buff.expires_at - now
    if remaining > 0 then
      table.insert(buffs, {
        name = name,
        remaining_seconds = remaining,
        remaining_minutes = string.format("%.1f", remaining / 60),
      })
    end
  end

  table.sort(buffs, function(a, b)
    return a.remaining_seconds < b.remaining_seconds
  end)

  return buffs
end

--- Check if a specific buff is active
function BuffManager.has_buff(buff_name)
  local buff = state.active_buffs[buff_name]
  if not buff then
    return false
  end

  -- Check if still valid
  if buff.expires_at <= os.time() then
    state.active_buffs[buff_name] = nil
    return false
  end

  return true
end

--- Get time remaining on a buff
function BuffManager.get_buff_remaining(buff_name)
  local buff = state.active_buffs[buff_name]
  if not buff then
    return 0
  end

  local remaining = buff.expires_at - os.time()
  return math.max(0, remaining)
end

--- Print buff status
function BuffManager.print_status()
  print("[BuffManager] Active Buffs:")

  local buffs = BuffManager.get_active_buffs()
  if #buffs == 0 then
    print("  (none)")
    return
  end

  for _, buff in ipairs(buffs) do
    print(string.format("  %s: %.1f min remaining", buff.name, buff.remaining_minutes))
  end
end

--- Emergency buff check (immediate recast all)
function BuffManager.emergency_rebuff()
  print("[BuffManager] EMERGENCY: Recasting all buffs immediately...")

  for _, buff_config in ipairs(CONFIG.buffs_to_maintain) do
    BuffManager.recast_buff(buff_config)
  end
end

--- Get buff statistics
function BuffManager.get_stats()
  local active = 0
  local missing = 0

  for _, buff_config in ipairs(CONFIG.buffs_to_maintain) do
    if BuffManager.has_buff(buff_config.name) then
      active = active + 1
    else
      missing = missing + 1
    end
  end

  return {
    active_buffs = active,
    missing_buffs = missing,
    total_configured = #CONFIG.buffs_to_maintain,
  }
end

--- Customize buff config at runtime
function BuffManager.add_buff_config(buff_config)
  table.insert(CONFIG.buffs_to_maintain, buff_config)
  print(string.format("[BuffManager] Added buff config: %s", buff_config.name))
end

--- Remove buff from tracking
function BuffManager.remove_buff_config(buff_name)
  for i, cfg in ipairs(CONFIG.buffs_to_maintain) do
    if cfg.name == buff_name then
      table.remove(CONFIG.buffs_to_maintain, i)
      print(string.format("[BuffManager] Removed buff config: %s", buff_name))
      return true
    end
  end
  return false
end

return BuffManager
