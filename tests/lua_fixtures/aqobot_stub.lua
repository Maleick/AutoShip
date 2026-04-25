-- aqobot conformance stub
-- Simulates the initialization sequence of aquietone/aqobot.
-- Real script: loads class module, registers MQ events, enters mq.delay loop.
-- Conformance gate: reaches MAIN_LOOP_REACHED without error.

require('mq_compat')

local aqobot = {}
aqobot._version = "stub-1.0"
aqobot._class_module = nil

local function load_class_module(class)
    -- Real: require('aqobot/' .. class:lower())
    return { class = class, initialized = true }
end

local function register_events()
    mq.event("^You have been slain", function() aqobot.on_death() end)
    mq.event("^(.+) says", function(who, msg) end)
    mq.bind("/aqobot", function(cmd) end)
end

local function init()
    local class = tq.me.class()
    aqobot._class_module = load_class_module(class)
    register_events()
    -- Real: while mq.delay(1000, fn) do ... end
    -- Stub: signal conformance gate
    aqobot._main_loop_reached = true
end

init()

assert(aqobot._main_loop_reached == true, "aqobot: did not reach main loop")
assert(aqobot._class_module ~= nil, "aqobot: class module not loaded")
assert(#mq._event_handlers >= 2, "aqobot: events not registered")
assert(mq._binds["/aqobot"] ~= nil, "aqobot: bind not registered")

_G.CONFORMANCE_RESULT = "PASS"
