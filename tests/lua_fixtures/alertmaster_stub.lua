-- AlertMaster conformance stub
-- Simulates the initialization of grimmier378/AlertMaster.
-- Real: loads alert config, registers mq.event handlers for named spawns.
-- Conformance gate: alertmaster table defined with non-empty alert list.

require('mq_compat')

local alertmaster = {}
alertmaster._alerts = {}
alertmaster._initialized = false

function alertmaster.add_alert(name, pattern, callback)
    assert(type(name) == "string")
    assert(type(pattern) == "string")
    table.insert(alertmaster._alerts, {
        name = name,
        pattern = pattern,
        callback = callback or function() end,
    })
    mq.event(pattern, callback or function() end)
end

function alertmaster.init()
    -- Real: reads AlertMaster.ini, registers named-spawn alerts
    alertmaster.add_alert(
        "named_spawn",
        "^You see (.+) has spawned",
        function(name) tq.log.warn("Named: " .. name) end
    )
    alertmaster.add_alert(
        "rare_spawn",
        "^(.+) engages (.+)",
        function(attacker, target) end
    )
    -- rgmercs Named module checks this alert table
    alertmaster._initialized = true
end

alertmaster.init()

assert(alertmaster._initialized == true, "alertmaster: init() did not complete")
assert(#alertmaster._alerts >= 1, "alertmaster: alert list is empty")
assert(#mq._event_handlers >= 1, "alertmaster: no events registered")

_G.alertmaster = alertmaster
_G.CONFORMANCE_RESULT = "PASS"
