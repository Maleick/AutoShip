-- TextQuest Combat Helper Script
-- Demonstrates: target switching, heal management, combat monitoring
-- Usage: Load via TUI script loader and configure in CHARACTER_CONFIG
--
-- This script manages healing rotation and target switching during combat,
-- serving as a template for custom combat logic beyond the core DLL rotation.

local CombatHelper = {}

-- Configuration
local CONFIG = {
  -- Heal thresholds (percentage)
  self_heal_threshold = 40,
  tank_heal_threshold = 35,
  group_heal_threshold = 50,

  -- Target switching rules
  max_assistable_adds = 2,
  ignore_named_for_cc = false,

  -- Spell casting
  fast_heal_spell = "Complete Heal",
  group_heal_spell = "Group Heal",
  cure_spell = "Cure Poison",

  -- Monitoring
  enable_combat_log = true,
  enable_damage_tracking = true,
}

-- Internal state
local state = {
  in_combat = false,
  current_target = nil,
  group_members = {},
  active_heals = {},
  damage_log = {},
}

--- Initialize combat helper
function CombatHelper.init()
  print("[CombatHelper] Initializing combat helper...")

  -- Register event handlers with TextQuest API
  -- (API methods shown for documentation; actual binding depends on implementation)
  if TextQuest and TextQuest.on_combat_start then
    TextQuest.on_combat_start(function()
      CombatHelper.on_combat_start()
    end)
  end

  if TextQuest and TextQuest.on_combat_end then
    TextQuest.on_combat_end(function()
      CombatHelper.on_combat_end()
    end)
  end

  if TextQuest and TextQuest.on_damage_taken then
    TextQuest.on_damage_taken(function(damage_info)
      CombatHelper.on_damage_received(damage_info)
    end)
  end

  print("[CombatHelper] Ready. Listening for combat events.")
end

--- Called when entering combat
function CombatHelper.on_combat_start()
  state.in_combat = true
  print("[CombatHelper] Combat started")
end

--- Called when leaving combat
function CombatHelper.on_combat_end()
  state.in_combat = false
  state.active_heals = {}
  state.damage_log = {}
  print("[CombatHelper] Combat ended")
end

--- Track damage taken
function CombatHelper.on_damage_received(damage_info)
  if CONFIG.enable_damage_tracking then
    table.insert(state.damage_log, {
      timestamp = os.time(),
      amount = damage_info.amount,
      source = damage_info.source,
    })
  end

  CombatHelper.evaluate_heals()
end

--- Evaluate current health and cast appropriate heals
function CombatHelper.evaluate_heals()
  if not state.in_combat then
    return
  end

  -- Get self health percentage
  local self_hp = TextQuest.get_self_hp_percent()
  if self_hp and self_hp < CONFIG.self_heal_threshold then
    CombatHelper.cast_heal("self", CONFIG.fast_heal_spell)
    return
  end

  -- Get tank health and heal if needed
  local tank = TextQuest.get_group_tank()
  if tank then
    local tank_hp = TextQuest.get_group_member_hp_percent(tank.name)
    if tank_hp and tank_hp < CONFIG.tank_heal_threshold then
      CombatHelper.cast_heal(tank.name, CONFIG.fast_heal_spell)
      return
    end
  end

  -- Evaluate group heals
  local low_hp_members = CombatHelper.get_low_hp_members()
  if #low_hp_members >= 2 then
    CombatHelper.cast_heal("group", CONFIG.group_heal_spell)
    return
  end
end

--- Get list of group members below threshold
function CombatHelper.get_low_hp_members()
  local members = {}
  local group = TextQuest.get_group_members()

  if not group then
    return members
  end

  for _, member in ipairs(group) do
    local hp = TextQuest.get_group_member_hp_percent(member.name)
    if hp and hp < CONFIG.group_heal_threshold then
      table.insert(members, member)
    end
  end

  return members
end

--- Cast heal spell on target
function CombatHelper.cast_heal(target, spell_name)
  if not TextQuest or not TextQuest.cast_spell then
    print("[CombatHelper] TextQuest API not available")
    return false
  end

  local success = TextQuest.cast_spell(spell_name, target)

  if success then
    state.active_heals[target] = {
      spell = spell_name,
      timestamp = os.time(),
    }
    if CONFIG.enable_combat_log then
      print(string.format("[CombatHelper] Cast %s on %s", spell_name, target))
    end
  end

  return success
end

--- Switch to secondary target if primary is dead/gone
function CombatHelper.smart_target_switch()
  local current = TextQuest.get_current_target()

  if not current or current.hp <= 0 then
    -- Target dead, switch to assist
    local assist_target = TextQuest.get_group_leader()
    if assist_target then
      TextQuest.set_target(assist_target.name)
      print(string.format("[CombatHelper] Switching to %s", assist_target.name))
    end
  end
end

--- Get combat statistics
function CombatHelper.get_stats()
  local total_damage = 0
  for _, entry in ipairs(state.damage_log) do
    total_damage = total_damage + entry.amount
  end

  return {
    total_damage_taken = total_damage,
    hits_taken = #state.damage_log,
    avg_damage = #state.damage_log > 0 and (total_damage / #state.damage_log) or 0,
  }
end

return CombatHelper
