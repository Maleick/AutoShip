-- luaconsole conformance stub
-- Simulates the initialization of aquietone/luaconsole.
-- Real: ImGui window with a Lua REPL; uses loadstring/pcall eval loop.
-- Conformance gate: luaconsole table defined, open() registers ImGui callback.

require('mq_compat')

local luaconsole = {}
luaconsole._history = {}
luaconsole._imgui_registered = false
luaconsole._open = false

function luaconsole.eval(code)
    local fn, compile_err = load(code)
    if not fn then
        return nil, compile_err
    end
    local ok, result = pcall(fn)
    if not ok then
        return nil, result
    end
    table.insert(luaconsole._history, { code = code, result = tostring(result) })
    return result, nil
end

function luaconsole.open()
    mq.imgui("LuaConsole", function()
        -- Real: renders input box + history list via ImGui
    end)
    luaconsole._imgui_registered = true
    luaconsole._open = true
end

function luaconsole.close()
    luaconsole._open = false
end

-- Bind /luaconsole command
mq.bind("/luaconsole", function(cmd)
    if luaconsole._open then
        luaconsole.close()
    else
        luaconsole.open()
    end
end)

luaconsole.open()

-- Test eval path
local result, err = luaconsole.eval("return 2 + 2")
assert(err == nil, "luaconsole: eval errored: " .. tostring(err))
assert(result == 4, "luaconsole: eval returned wrong value")
assert(luaconsole._imgui_registered == true, "luaconsole: ImGui not registered")
assert(mq._binds["/luaconsole"] ~= nil, "luaconsole: /luaconsole bind not registered")

_G.luaconsole = luaconsole
_G.CONFORMANCE_RESULT = "PASS"
