//! Typed function call bindings for EQUILIB/FX style function offsets.
//!
//! The [`eq_fn!`] macro creates a named, callable binding value that resolves
//! its address at call time using a preferred-base offset and runtime EQ base
//! address.
//!
//! Example:
//! ```ignore
//! use textquest_common::eq_fn;
//! use textquest_common::offsets;
//! use core::ffi::c_void;
//!
//! eq_fn!(interpret_cmd(this: *mut c_void, cmd: *const u8) -> i32 = offsets::INTERPRET_CMD);
//! let _ = unsafe { interpret_cmd.call(eq_base, cmd_ptr) };
//! ```

use core::sync::atomic::Ordering;
use std::sync::{OnceLock, RwLock};
use std::{fmt, sync::atomic::AtomicBool};

use crate::offset_db::OffsetDatabase;
use crate::offsets;

static FALLBACK_DB: OnceLock<RwLock<Option<OffsetDatabase>>> = OnceLock::new();
static BINDING_LOG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Install a runtime function-offset database used when a preferred-base constant appears
/// to be stale.
pub fn install_fallback_database(db: OffsetDatabase) {
    let lock = FALLBACK_DB.get_or_init(|| RwLock::new(None));
    let mut guard = lock.write().expect("fallback database lock poisoned");
    *guard = Some(db);
}

/// Remove any installed fallback database.
pub fn clear_fallback_database() {
    let lock = FALLBACK_DB.get_or_init(|| RwLock::new(None));
    let mut guard = lock.write().expect("fallback database lock poisoned");
    *guard = None;
}

fn with_fallback_database<F, T>(f: F) -> Option<T>
where
    F: FnOnce(&OffsetDatabase) -> T,
{
    FALLBACK_DB
        .get()
        .and_then(|lock| lock.read().ok())
        .and_then(|db| db.as_ref().map(f))
}

/// Toggle debug logging emitted from [`eq_fn!`] call sites.
pub fn set_binding_debug_enabled(enabled: bool) {
    BINDING_LOG_ENABLED.store(enabled, Ordering::Release);
}

pub fn resolve_function_address(preferred_addr: u64, eq_base: u64, function_key: &str) -> usize {
    let function_key = to_camel_case(function_key);
    let preferred_addr = offsets::rebase(preferred_addr, eq_base);
    let fallback_addr = with_fallback_database(|db| {
        db.get_function(&function_key)
            .and_then(|v| db.rebase(v, eq_base))
    })
    .flatten();

    match (preferred_addr, fallback_addr) {
        (Some(preferred), Some(fallback)) if preferred == fallback => preferred,
        (None, Some(fallback)) => fallback,
        (Some(preferred), Some(fallback)) => {
            tracing::warn!(
                function = function_key,
                preferred = format!("{:#x}", preferred),
                fallback = format!("{:#x}", fallback),
                "eq_fn address mismatch: preferred and fallback diverge; using fallback"
            );
            fallback
        }
        (Some(preferred), None) => preferred,
        (None, None) => {
            tracing::error!(
                function = function_key,
                "eq_fn could not resolve function address from preferred offset or fallback DB"
            );
            panic!("eq_fn unresolved address for {function_key}");
        }
    }
}

pub fn to_camel_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper = false;
    for c in s.chars() {
        if c == '_' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[inline(always)]
pub fn log_call<Fmt: fmt::Debug>(name: &str, args: Fmt, addr: usize) {
    if BINDING_LOG_ENABLED.load(Ordering::Acquire) {
        tracing::debug!(function = name, address = format!("{:#x}", addr), args = ?args, "eq_fn invoked");
    }
}

/// Descriptor shared by all typed binding callsites.
#[derive(Copy, Clone)]
pub struct EqFn {
    preferred_addr: u64,
    function_key: &'static str,
}

impl EqFn {
    pub const fn new(preferred_addr: u64, function_key: &'static str) -> Self {
        Self {
            preferred_addr,
            function_key,
        }
    }

    #[inline]
    pub fn addr(&self, eq_base: u64) -> usize {
        resolve_function_address(self.preferred_addr, eq_base, self.function_key)
    }
}

#[macro_export]
macro_rules! eq_fn {
    ($fn_name:ident ( $($arg:ident : $arg_ty:ty),* ) -> $ret:ty = $offset:path) => {
        $crate::paste::paste! {
            #[allow(non_camel_case_types)]
            pub struct [<$fn_name _binding>];

            impl [<$fn_name _binding>] {
                const BINDING: $crate::bindings::EqFn =
                    $crate::bindings::EqFn::new($offset, stringify!($fn_name));

                #[inline]
                pub fn addr(&self, eq_base: u64) -> usize {
                    Self::BINDING.addr(eq_base)
                }

                #[allow(clippy::too_many_arguments)]
                #[inline]
                pub unsafe fn call(
                    &self,
                    eq_base: u64,
                    $( $arg: $arg_ty ),*
                ) -> $ret {
                    let addr = self.addr(eq_base);
                    debug_assert_ne!(addr, 0, "eq_fn unresolved address for {}", stringify!($fn_name));

                    #[cfg(debug_assertions)]
                    {
                        $crate::bindings::log_call(stringify!($fn_name), ($($arg,)*), addr);
                    }

                    let func: unsafe extern "C" fn( $( $arg_ty ),* ) -> $ret =
                        unsafe { std::mem::transmute::<usize, _>(addr) };
                    unsafe { func($($arg),*) }
                }
            }

            #[allow(non_upper_case_globals)]
            pub static $fn_name: [<$fn_name _binding>] = [<$fn_name _binding>];
        }
    };
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use crate::offset_db::OffsetDatabase;
    use crate::offsets;

    eq_fn!(test_eq_binding_binding(a: u32, ptr: *const u8) -> u64 = offsets::CAST_SPELL);

    static FALLBACK_DB_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    static mut BINDING_CALL_ARG_A: u32 = 0;
    static mut BINDING_CALL_ARG_PTR: usize = 0;

    fn lock_fallback_db_test() -> std::sync::MutexGuard<'static, ()> {
        FALLBACK_DB_TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("fallback database test mutex poisoned")
    }

    unsafe extern "C" fn test_target(a: u32, ptr: *const u8) -> u64 {
        unsafe {
            BINDING_CALL_ARG_A = a;
            BINDING_CALL_ARG_PTR = ptr as usize;
        }
        0xABCDu64 + a as u64
    }

    #[test]
    fn snake_case_to_camel_case() {
        assert_eq!(super::to_camel_case("interpret_cmd"), "interpretCmd");
        assert_eq!(
            super::to_camel_case("execute_cmd_with_arg"),
            "executeCmdWithArg"
        );
    }

    #[test]
    fn eq_fn_uses_rebased_preferred_when_no_fallback() {
        let _test_guard = lock_fallback_db_test();
        super::clear_fallback_database();
        let expected = offsets::rebase(offsets::CAST_SPELL, 0x1000).expect("expected rebase");
        assert_eq!(test_eq_binding_binding.addr(0x1000), expected);
    }

    #[test]
    fn eq_fn_uses_offset_db_when_preferred_is_stale() {
        let _test_guard = lock_fallback_db_test();
        super::clear_fallback_database();

        let mut db = OffsetDatabase::from_compiled_offsets();
        db.eq_preferred_base = 0;

        let actual = test_target as *const () as usize as u64;
        let key = super::to_camel_case("test_eq_binding_binding");
        db.functions.insert(key, actual);
        super::install_fallback_database(db);

        let bytes = [1u8, 2, 3];
        let result = unsafe { test_eq_binding_binding.call(0, 21, bytes.as_ptr()) };
        assert_eq!(result, 0xABCDu64 + 21);
        assert_eq!(unsafe { BINDING_CALL_ARG_A }, 21);
        assert_eq!(unsafe { BINDING_CALL_ARG_PTR }, bytes.as_ptr() as usize);

        super::clear_fallback_database();
    }

    #[test]
    fn set_binding_debug_flag_is_toggleable() {
        super::set_binding_debug_enabled(false);
        assert!(!super::BINDING_LOG_ENABLED.load(std::sync::atomic::Ordering::Acquire));
        super::set_binding_debug_enabled(true);
        assert!(super::BINDING_LOG_ENABLED.load(std::sync::atomic::Ordering::Acquire));
    }

    // ─── to_camel_case edge cases ────────────────────────────────────────

    #[test]
    fn to_camel_case_empty_string() {
        assert_eq!(super::to_camel_case(""), "");
    }

    #[test]
    fn to_camel_case_no_underscores() {
        assert_eq!(super::to_camel_case("already"), "already");
    }

    #[test]
    fn to_camel_case_single_word() {
        assert_eq!(super::to_camel_case("word"), "word");
    }

    #[test]
    fn to_camel_case_leading_underscore() {
        // Leading underscore means next char uppercased
        assert_eq!(super::to_camel_case("_private"), "Private");
    }

    #[test]
    fn to_camel_case_trailing_underscore() {
        // Trailing underscore is stripped
        assert_eq!(super::to_camel_case("trailing_"), "trailing");
    }

    #[test]
    fn to_camel_case_consecutive_underscores() {
        // Multiple consecutive underscores — only one transition
        assert_eq!(super::to_camel_case("double__under"), "doubleUnder");
    }

    #[test]
    fn to_camel_case_all_underscores() {
        assert_eq!(super::to_camel_case("___"), "");
    }

    #[test]
    fn to_camel_case_single_char_segments() {
        assert_eq!(super::to_camel_case("a_b_c"), "aBC");
    }

    // ─── EqFn struct ─────────────────────────────────────────────────────

    #[test]
    fn eq_fn_new_stores_fields() {
        let binding = super::EqFn::new(0xDEAD, "test_key");
        assert_eq!(binding.preferred_addr, 0xDEAD);
        assert_eq!(binding.function_key, "test_key");
    }

    // ─── install/clear fallback database ─────────────────────────────────

    #[test]
    fn install_and_clear_fallback_database() {
        let _test_guard = lock_fallback_db_test();
        super::clear_fallback_database();

        let db = OffsetDatabase::from_compiled_offsets();
        super::install_fallback_database(db);
        // Verify it's installed by checking the lock is populated
        assert!(super::FALLBACK_DB.get().is_some());

        super::clear_fallback_database();
        // Verify the inner option is None after clearing
        let lock = super::FALLBACK_DB.get().unwrap();
        let guard = lock.read().unwrap();
        assert!(guard.is_none());
    }
}
