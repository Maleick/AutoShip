-- Minimal MacroQuest2 / TextQuest API shim for conformance fixture testing.
-- Provides just enough surface for each Tier 1 script stub to reach its main loop.

local mq = {}

mq._event_handlers = {}
mq._binds = {}

function mq.event(pattern, handler)
    table.insert(mq._event_handlers, { pattern = pattern, handler = handler })
end

function mq.bind(cmd, handler)
    mq._binds[cmd] = handler
end

function mq.delay(ms, fn)
    -- No-op in test context; real impl yields for ms milliseconds
end

function mq.imgui(name, fn)
    -- No-op in test context; registers ImGui render callback
end

function mq.exit()
    -- No-op in test context
end

-- textquest / tq global table (mirrors TextQuest's registered Lua API)
local tq = {}

tq.me = {
    name = function() return "TestChar" end,
    class = function() return "WAR" end,
    level = function() return 60 end,
    hp = function() return 100 end,
    hp_pct = function() return 100.0 end,
    mana = function() return 0 end,
    mana_pct = function() return 0.0 end,
    x = function() return 0.0 end,
    y = function() return 0.0 end,
    z = function() return 0.0 end,
    in_combat = function() return false end,
    is_moving = function() return false end,
    zone_id = function() return 1 end,
}

tq.target = {
    id = function() return 0 end,
    name = function() return "" end,
    hp_pct = function() return 0.0 end,
    distance = function() return 0.0 end,
    is_npc = function() return false end,
}

tq.group = {
    size = function() return 1 end,
    member = function(i) return nil end,
}

tq.nav = {
    to = function(x, y, z) end,
    to_spawn = function(id) end,
    stop = function() end,
    is_active = function() return false end,
    mesh_loaded = function() return false end,
    current_zone = function() return "" end,
}

tq.spell = {
    cast = function(name) end,
    is_ready = function(name) return false end,
    mem = function(name, gem) end,
}

tq.event = {
    register = function(name, pattern, handler)
        table.insert(mq._event_handlers, { name = name, pattern = pattern, handler = handler })
    end,
    emit = function(name, ...) end,
}

tq.loot = {
    set_rule = function(item, action) end,
    get_rule = function(item) return "ignore" end,
}

tq.spawn = {
    get = function(id) return nil end,
    find = function(name) return nil end,
}

tq.actors = {
    register = function(name, handler) end,
    send = function(target, msg) end,
}

tq.dannet = {
    peer_count = function() return 0 end,
    peer = function(i) return nil end,
    query = function(peer, tlo, timeout_ms) return "" end,
}

tq.log = {
    info = function(...) end,
    warn = function(...) end,
    error = function(...) end,
    debug = function(...) end,
}

tq.execute_string = function(code)
    -- In-game: executes Lua string in the current VM context
    local fn, err = load(code)
    if fn then fn() end
    return err
end

-- Expose as globals (mirrors MQ2 / TextQuest environment)
_G.mq = mq
_G.tq = tq
_G.textquest = tq

return { mq = mq, tq = tq }
