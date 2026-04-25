-- lootnscoot conformance stub
-- Simulates the initialization of aquietone/lootnscoot.
-- Real: LootHelper table with per-item rules, integrates with rgmercs loot hook.
-- Conformance gate: LootHelper table defined, init() completes without error.

require('mq_compat')

local LootHelper = {}
LootHelper._rules = {}
LootHelper._initialized = false

function LootHelper.set_rule(item_name, action)
    assert(type(item_name) == "string")
    assert(action == "keep" or action == "sell" or action == "ignore" or action == "destroy")
    LootHelper._rules[item_name] = action
end

function LootHelper.get_rule(item_name)
    return LootHelper._rules[item_name] or "ignore"
end

function LootHelper.init()
    -- Real: reads lootnscoot.ini, registers /loot command, hooks OnLoot event
    LootHelper.set_rule("Rubicite Breastplate", "keep")
    LootHelper.set_rule("Plat Coin", "keep")
    mq.bind("/loot", function(cmd) end)
    mq.event("^(.+) loots (.+) from (.+)", function(who, item, mob) end)
    LootHelper._initialized = true
end

LootHelper.init()

assert(LootHelper._initialized == true, "lootnscoot: init() did not complete")
assert(LootHelper.get_rule("Rubicite Breastplate") == "keep", "lootnscoot: loot rule not set")
assert(mq._binds["/loot"] ~= nil, "lootnscoot: /loot bind not registered")
assert(#mq._event_handlers >= 1, "lootnscoot: loot event not registered")

_G.LootHelper = LootHelper
_G.CONFORMANCE_RESULT = "PASS"
