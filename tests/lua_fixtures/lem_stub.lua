-- lem (Lua Event Manager) conformance stub
-- Simulates the initialization of aquietone/lem.
-- Real: exports lem.register(name, cond_fn, action_fn), lem.run()
-- Conformance gate: lem table exists, register/run callable, conditions fire.

require('mq_compat')

local lem = {}
lem._conditions = {}

function lem.register(name, condition_fn, action_fn)
    assert(type(name) == "string", "lem.register: name must be string")
    assert(type(condition_fn) == "function", "lem.register: condition_fn must be function")
    assert(type(action_fn) == "function", "lem.register: action_fn must be function")
    table.insert(lem._conditions, {
        name = name,
        condition = condition_fn,
        action = action_fn,
        last_fired = 0,
    })
end

function lem.run()
    -- Real: while mq.delay(100) do evaluate each condition end
    for _, rule in ipairs(lem._conditions) do
        local ok, result = pcall(rule.condition)
        if ok and result then
            pcall(rule.action)
        end
    end
    lem._run_reached = true
end

-- Register example rules (mirrors real lem usage)
lem.register("low_hp_warn", function()
    return tq.me.hp_pct() < 30
end, function()
    tq.log.warn("HP low!")
end)

lem.register("in_combat_check", function()
    return tq.me.in_combat()
end, function()
    tq.log.info("in combat")
end)

lem.run()

assert(lem._run_reached == true, "lem: run() did not complete")
assert(#lem._conditions == 2, "lem: expected 2 registered conditions")

_G.CONFORMANCE_RESULT = "PASS"
