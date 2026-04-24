//! Lua sandbox — restricts script capabilities to a safe, auditable subset.
//!
//! # What is blocked
//! - `os`, `io` — filesystem and shell access
//! - `debug` — introspection / escape hatches
//! - `load`, `loadstring`, `loadfile`, `dofile` — dynamic code loading
//! - general `require` — native C modules and arbitrary packages
//! - `package` — not loaded at all
//!
//! # What is allowed
//! `math`, `string`, `table`, `utf8`, `coroutine` — all pure computation.
//! After TextQuest APIs are registered, `require("textquest")` is restored as
//! the only allowed module import.
//!
//! # Resource limits
//! - Memory: 64 MiB hard cap via `Lua::set_memory_limit`.
//! - CPU: instruction-count hook; aborts after 10 million instructions.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use mlua::{Error as LuaError, HookTriggers, Lua, Result as LuaResult, StdLib, VmState};

/// Returns the safe library set for every sandboxed Lua context.
///
/// Omits `IO`, `OS`, `PACKAGE`, `DEBUG`.
pub fn sandbox_libs() -> StdLib {
    StdLib::COROUTINE | StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH
}

/// Memory limit for a sandboxed Lua state (64 MiB).
pub const SANDBOX_MEMORY_LIMIT: usize = 64 * 1024 * 1024;

/// Max instructions before CPU abort (10 million).
pub const SANDBOX_INSTRUCTION_LIMIT: u64 = 10_000_000;

/// Instruction granularity for the hook (check every N instructions).
const HOOK_INTERVAL: u32 = 10_000;

/// Applies sandbox restrictions to an already-created Lua state.
///
/// Call this immediately after constructing the VM, before loading any scripts.
///
/// # What this does
/// 1. Nils out dangerous globals that may have been pre-loaded.
/// 2. Sets a 64 MiB memory limit.
/// 3. Installs an instruction-count hook that aborts after 10 million instructions.
pub fn apply(lua: &Lua) -> LuaResult<Arc<AtomicU64>> {
    remove_dangerous_globals(lua)?;
    lua.set_memory_limit(SANDBOX_MEMORY_LIMIT)
        .map_err(|e| LuaError::RuntimeError(format!("sandbox: memory limit failed: {e}")))?;
    let counter = install_cpu_hook(lua)?;
    tracing::debug!(
        memory_limit_mb = SANDBOX_MEMORY_LIMIT / (1024 * 1024),
        instruction_limit = SANDBOX_INSTRUCTION_LIMIT,
        "Lua sandbox applied"
    );
    Ok(counter)
}

/// Remove globals that provide dangerous capabilities.
///
/// Even when not loaded via `StdLib`, these names might be set by future code
/// paths or in tests. Explicit nil-out is defense-in-depth.
fn remove_dangerous_globals(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    for name in &[
        "os",
        "io",
        "debug",
        "load",
        "loadstring",
        "loadfile",
        "dofile",
        "require",
        "package",
    ] {
        globals.set(*name, mlua::Value::Nil)?;
    }

    Ok(())
}

/// Install a Lua debug hook that aborts execution after `SANDBOX_INSTRUCTION_LIMIT`
/// instructions have been counted.
///
/// Returns the shared counter so callers can reset it between invocations
/// (e.g. before each event-handler call).
fn install_cpu_hook(lua: &Lua) -> LuaResult<Arc<AtomicU64>> {
    let counter: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let counter_hook = Arc::clone(&counter);

    lua.set_hook(
        HookTriggers::new().every_nth_instruction(HOOK_INTERVAL),
        move |_lua, _debug| {
            let count = counter_hook.fetch_add(HOOK_INTERVAL as u64, Ordering::Relaxed);
            if count >= SANDBOX_INSTRUCTION_LIMIT {
                Err(LuaError::RuntimeError(
                    "sandbox: CPU limit exceeded (execution aborted)".into(),
                ))
            } else {
                Ok(VmState::Continue)
            }
        },
    )?;

    Ok(counter)
}

/// Reset the instruction counter returned by `apply()`.
///
/// Call this before each top-level script invocation to prevent accumulated
/// ticks from erroneously killing the next call.
pub fn reset_instruction_counter(counter: &AtomicU64) {
    counter.store(0, Ordering::Relaxed);
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::LuaOptions;

    /// Create a sandboxed Lua VM the same way production code does:
    /// load only safe libs, then apply the sandbox.
    fn sandboxed_lua() -> (Lua, Arc<AtomicU64>) {
        let lua =
            Lua::new_with(sandbox_libs(), LuaOptions::default()).expect("create sandboxed Lua");
        let counter = apply(&lua).expect("apply sandbox");
        (lua, counter)
    }

    // ── Blocked globals ────────────────────────────────────────────────────────

    #[test]
    fn os_execute_is_blocked() {
        let (lua, _) = sandboxed_lua();
        let result = lua.load("os.execute('echo pwned')").exec();
        assert!(result.is_err(), "os.execute must be blocked; got Ok(())");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("nil") || msg.contains("attempt to index"),
            "expected nil-index error, got: {msg}"
        );
    }

    #[test]
    fn io_open_is_blocked() {
        let (lua, _) = sandboxed_lua();
        let result = lua.load("io.open('/etc/passwd', 'r')").exec();
        assert!(result.is_err(), "io.open must be blocked");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("nil") || msg.contains("attempt to index"),
            "expected nil-index error, got: {msg}"
        );
    }

    #[test]
    fn debug_getfenv_is_blocked() {
        let (lua, _) = sandboxed_lua();
        let result = lua.load("debug.getfenv(function() end)").exec();
        assert!(result.is_err(), "debug.getfenv must be blocked");
    }

    #[test]
    fn load_is_blocked() {
        let (lua, _) = sandboxed_lua();
        let result = lua.load("load('return 1')").eval::<i64>();
        assert!(result.is_err(), "load() must be blocked");
    }

    #[test]
    fn loadstring_is_blocked() {
        let (lua, _) = sandboxed_lua();
        let result = lua.load("loadstring('return 1')").eval::<i64>();
        assert!(result.is_err(), "loadstring() must be blocked");
    }

    #[test]
    fn dofile_is_blocked() {
        let (lua, _) = sandboxed_lua();
        let result = lua.load("dofile('/etc/passwd')").exec();
        assert!(result.is_err(), "dofile() must be blocked");
    }

    #[test]
    fn require_is_blocked() {
        let (lua, _) = sandboxed_lua();
        let result = lua.load("require('os')").exec();
        assert!(result.is_err(), "require() must be blocked");
    }

    // ── Allowed globals ────────────────────────────────────────────────────────

    #[test]
    fn math_is_allowed() {
        let (lua, _) = sandboxed_lua();
        let result: f64 = lua
            .load("return math.sqrt(16)")
            .eval()
            .expect("math.sqrt should work");
        assert!((result - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn string_is_allowed() {
        let (lua, _) = sandboxed_lua();
        let result: String = lua
            .load("return string.upper('hello')")
            .eval()
            .expect("string.upper should work");
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn table_is_allowed() {
        let (lua, _) = sandboxed_lua();
        let result: i64 = lua
            .load("local t = {1,2,3}; table.insert(t, 4); return #t")
            .eval()
            .expect("table ops should work");
        assert_eq!(result, 4);
    }

    // ── Resource limits ────────────────────────────────────────────────────────

    #[test]
    fn infinite_loop_is_killed() {
        let (lua, _) = sandboxed_lua();
        let result = lua.load("while true do end").exec();
        assert!(result.is_err(), "infinite loop must be killed by CPU hook");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("CPU limit") || msg.contains("sandbox"),
            "expected sandbox CPU error, got: {msg}"
        );
    }

    #[test]
    fn memory_limit_fails_gracefully() {
        let (lua, _) = sandboxed_lua();
        // Attempt to allocate ~500 MB via string concatenation — well above the 64 MB cap.
        let result = lua
            .load(
                r#"
local s = "x"
for _ = 1, 30 do
    s = s .. s
end
return #s
"#,
            )
            .eval::<i64>();
        assert!(
            result.is_err(),
            "allocation beyond 64 MiB limit must fail gracefully"
        );
    }

    // ── Additional blocked globals ─────────────────────────────────────────────

    #[test]
    fn loadfile_is_blocked() {
        let (lua, _) = sandboxed_lua();
        let result = lua.load("loadfile('/etc/passwd')").eval::<mlua::Value>();
        assert!(result.is_err(), "loadfile() must be blocked");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("nil") || msg.contains("attempt") || msg.contains("global"),
            "expected nil-index error, got: {msg}"
        );
    }

    #[test]
    fn package_is_blocked() {
        let (lua, _) = sandboxed_lua();
        // package is nil-ed out; accessing package.path should index nil.
        let result = lua.load("return package.path").eval::<mlua::Value>();
        assert!(result.is_err(), "package global must be blocked");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("nil") || msg.contains("attempt") || msg.contains("package"),
            "expected nil-index error, got: {msg}"
        );
    }

    // ── Additional allowed globals ─────────────────────────────────────────────

    #[test]
    fn utf8_is_allowed() {
        let (lua, _) = sandboxed_lua();
        let result: i64 = lua
            .load(r#"return utf8.len("hello")"#)
            .eval()
            .expect("utf8.len should be available");
        assert_eq!(result, 5);
    }

    #[test]
    fn instruction_counter_reset_works() {
        let (lua, counter) = sandboxed_lua();

        // Run a short script close to (but not exceeding) the limit.
        lua.load("local x = 0; for i = 1, 100 do x = x + i end; return x")
            .eval::<i64>()
            .expect("short loop should run");

        // Reset counter and run again — must not trip the limit.
        reset_instruction_counter(&counter);

        let result: i64 = lua
            .load("local x = 0; for i = 1, 100 do x = x + i end; return x")
            .eval()
            .expect("second run after reset should succeed");
        assert_eq!(result, 5050);
    }
}
