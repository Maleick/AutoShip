-- TextQuest Loot Filter Script
-- Demonstrates: custom loot rules, filtering, sorting, and valuation
-- Usage: Configure loot priorities and let script auto-loot based on rules
--
-- This script evaluates loot against custom rules and automatically loots
-- valuable items while leaving trash. Integrates with the TUI's loot system.

local LootFilter = {}

-- Configuration - define loot rules and priorities
local CONFIG = {
  -- Enable/disable auto-loot
  enable_auto_loot = false,

  -- Price threshold for auto-pickup (in platinum)
  min_value_to_loot = 50,

  -- Loot priorities: higher number = higher priority
  priorities = {
    ["Krono"] = 1000,
    ["Conflagrant Ore"] = 500,
    ["Glowing Silk Robes"] = 400,
    ["Ethereal"] = 300,
    ["Kunark"] = 250,
    ["Velious"] = 200,
    ["Planar"] = 150,
    ["Ornate Plate"] = 100,
    ["Spell"] = 100,
  },

  -- Exclusion list - never loot these
  blacklist = {
    ["Rusty Short Sword"] = true,
    ["Tattered Cloth"] = true,
    ["Broken Pottery"] = true,
  },

  -- Whitelist - always loot if found
  whitelist = {
    ["Bronze Plate Boots"] = true,
    ["Plate Armor"] = true,
  },

  -- Logging
  enable_logging = true,
  show_loot_evaluations = false,
}

-- Internal state
local state = {
  looted_items = {},
  items_seen = {},
  total_value = 0,
  auto_loot_enabled = false,
}

--- Initialize loot filter
function LootFilter.init()
  print("[LootFilter] Initializing loot filter...")

  -- Register event handlers
  if TextQuest and TextQuest.on_loot_available then
    TextQuest.on_loot_available(function(items)
      LootFilter.on_loot_available(items)
    end)
  end

  print("[LootFilter] Ready. Listening for loot events.")
end

--- Called when loot becomes available
function LootFilter.on_loot_available(items)
  if not items or #items == 0 then
    return
  end

  local evaluated = {}

  for _, item in ipairs(items) do
    local evaluation = LootFilter.evaluate_item(item)
    table.insert(evaluated, evaluation)

    if CONFIG.show_loot_evaluations then
      print(string.format("[LootFilter] %s: %s (value: %dp, priority: %d)",
        item.name, evaluation.action, evaluation.value, evaluation.priority))
    end
  end

  -- Sort by priority
  table.sort(evaluated, function(a, b)
    return a.priority > b.priority
  end)

  -- Auto-loot high priority items
  if CONFIG.enable_auto_loot then
    LootFilter.auto_loot_items(evaluated)
  end

  -- Report summary
  if CONFIG.enable_logging then
    LootFilter.print_loot_summary(evaluated)
  end
end

--- Evaluate a single item
function LootFilter.evaluate_item(item)
  local action = "PASS"
  local priority = 0
  local value = item.value or 0

  -- Check whitelist first (always take)
  if CONFIG.whitelist[item.name] then
    action = "LOOT (whitelist)"
    priority = 5000
    return {
      item_name = item.name,
      action = action,
      priority = priority,
      value = value,
      reason = "whitelisted",
    }
  end

  -- Check blacklist
  if CONFIG.blacklist[item.name] then
    action = "SKIP (blacklist)"
    priority = -1000
    return {
      item_name = item.name,
      action = action,
      priority = priority,
      value = value,
      reason = "blacklisted",
    }
  end

  -- Check priority list
  local base_priority = 0
  local reason = "default"

  for keyword, prio in pairs(CONFIG.priorities) do
    if string.find(item.name, keyword, 1, true) then
      base_priority = prio
      reason = string.format("matches '%s'", keyword)
      break
    end
  end

  -- Check minimum value
  if value >= CONFIG.min_value_to_loot then
    action = "LOOT (value)"
    priority = base_priority + (value / 1000)  -- Add value multiplier
    reason = reason .. " + value"
  elseif base_priority > 0 then
    action = "LOOT (priority)"
    priority = base_priority
  else
    action = "SKIP"
    priority = -(CONFIG.min_value_to_loot - value)
  end

  return {
    item_name = item.name,
    action = action,
    priority = priority,
    value = value,
    reason = reason,
  }
end

--- Auto-loot items above threshold
function LootFilter.auto_loot_items(evaluated_items)
  if not TextQuest or not TextQuest.loot_item then
    return
  end

  for _, evaluation in ipairs(evaluated_items) do
    if string.find(evaluation.action, "LOOT") then
      local success = TextQuest.loot_item(evaluation.item_name)
      if success then
        table.insert(state.looted_items, evaluation.item_name)
        state.total_value = state.total_value + evaluation.value

        if CONFIG.enable_logging then
          print(string.format("[LootFilter] Looted: %s (%dp)",
            evaluation.item_name, evaluation.value))
        end
      end
    end
  end
end

--- Print loot summary
function LootFilter.print_loot_summary(evaluated_items)
  local looted = 0
  local skipped = 0
  local total_value = 0

  for _, eval in ipairs(evaluated_items) do
    if string.find(eval.action, "LOOT") then
      looted = looted + 1
      total_value = total_value + eval.value
    else
      skipped = skipped + 1
    end
  end

  print(string.format("[LootFilter] Summary: %d looted, %d skipped (total: %dp)",
    looted, skipped, total_value))
end

--- Get loot history
function LootFilter.get_history()
  return {
    items = state.looted_items,
    total_value = state.total_value,
    count = #state.looted_items,
  }
end

--- Manually add item to whitelist
function LootFilter.whitelist_item(item_name)
  CONFIG.whitelist[item_name] = true
  print(string.format("[LootFilter] Added to whitelist: %s", item_name))
end

--- Manually add item to blacklist
function LootFilter.blacklist_item(item_name)
  CONFIG.blacklist[item_name] = true
  print(string.format("[LootFilter] Added to blacklist: %s", item_name))
end

--- Set priority for item or keyword
function LootFilter.set_priority(keyword, priority)
  CONFIG.priorities[keyword] = priority
  print(string.format("[LootFilter] Set priority for '%s' to %d", keyword, priority))
end

--- Toggle auto-loot feature
function LootFilter.toggle_auto_loot()
  CONFIG.enable_auto_loot = not CONFIG.enable_auto_loot
  local status = CONFIG.enable_auto_loot and "ENABLED" or "DISABLED"
  print(string.format("[LootFilter] Auto-loot %s", status))
end

--- Get loot statistics
function LootFilter.get_stats()
  return {
    total_looted = #state.looted_items,
    total_value = state.total_value,
    unique_items = LootFilter.count_unique_items(),
    whitelist_count = LootFilter.count_whitelist(),
    blacklist_count = LootFilter.count_blacklist(),
  }
end

--- Count unique items
function LootFilter.count_unique_items()
  local unique = {}
  for _, item in ipairs(state.looted_items) do
    unique[item] = true
  end
  return LootFilter.table_count(unique)
end

--- Count whitelisted items
function LootFilter.count_whitelist()
  return LootFilter.table_count(CONFIG.whitelist)
end

--- Count blacklisted items
function LootFilter.count_blacklist()
  return LootFilter.table_count(CONFIG.blacklist)
end

--- Utility: count table entries
function LootFilter.table_count(tbl)
  local count = 0
  for _ in pairs(tbl) do
    count = count + 1
  end
  return count
end

--- Export loot configuration as JSON
function LootFilter.export_config()
  local json = "{"
  json = json .. '"min_value":' .. CONFIG.min_value_to_loot .. ","
  json = json .. '"whitelist":['

  local first = true
  for item, _ in pairs(CONFIG.whitelist) do
    if not first then json = json .. "," end
    json = json .. '"' .. item .. '"'
    first = false
  end

  json = json .. "],"
  json = json .. '"blacklist":['

  first = true
  for item, _ in pairs(CONFIG.blacklist) do
    if not first then json = json .. "," end
    json = json .. '"' .. item .. '"'
    first = false
  end

  json = json .. "]}"

  return json
end

--- Reset loot history
function LootFilter.reset_history()
  state.looted_items = {}
  state.total_value = 0
  print("[LootFilter] Loot history cleared")
end

return LootFilter
