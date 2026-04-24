use std::{
    collections::HashMap,
    error::Error,
    fmt::{Display, Formatter},
    sync::{Mutex, OnceLock},
};

#[cfg(all(windows, not(test)))]
use retour::RawDetour;

use textquest_common::offsets;

#[derive(Debug, Clone, Copy)]
struct DetourError(&'static str);

impl Display for DetourError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl Error for DetourError {}

#[derive(Clone, Copy)]
struct DetourCandidate {
    name: &'static str,
    preferred_addr: u64,
    hook_fn: *const (),
}

#[cfg(all(windows, not(test)))]
enum DetourRecord {
    Active(RawDetour),
}

#[cfg(any(not(windows), test))]
#[derive(Debug)]
enum DetourRecord {
    Mock { _target: usize, _hook: usize },
}

pub struct DetourManager {
    detours: Mutex<HashMap<String, DetourRecord>>,
}

impl Default for DetourManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DetourManager {
    pub fn new() -> Self {
        Self {
            detours: Mutex::new(HashMap::new()),
        }
    }

    pub fn is_installed(&self, name: &str) -> bool {
        self.detours
            .lock()
            .map(|map| map.contains_key(name))
            .unwrap_or(false)
    }

    pub fn install(
        &self,
        name: &str,
        target_addr: usize,
        hook_fn: *const (),
    ) -> Result<(), Box<dyn Error>> {
        let mut map = self
            .detours
            .lock()
            .map_err(|_| DetourError("detour manager lock poisoned while installing"))?;

        if map.contains_key(name) {
            return Err(DetourError("detour already installed").into());
        }

        #[cfg(all(windows, not(test)))]
        let record = {
            let mut detour = unsafe { RawDetour::new(target_addr as *const (), hook_fn)? };
            unsafe {
                detour.enable()?;
            }
            DetourRecord::Active(detour)
        };

        #[cfg(any(not(windows), test))]
        let record = DetourRecord::Mock {
            _target: target_addr,
            _hook: hook_fn as usize,
        };

        map.insert(name.to_string(), record);
        Ok(())
    }

    pub fn uninstall(&self, name: &str) -> Result<(), Box<dyn Error>> {
        let mut map = self
            .detours
            .lock()
            .map_err(|_| DetourError("detour manager lock poisoned while uninstalling"))?;

        let Some(record) = map.remove(name) else {
            return Err(DetourError("detour not installed").into());
        };

        match record {
            #[cfg(all(windows, not(test)))]
            DetourRecord::Active(mut detour) => unsafe {
                detour.disable()?;
            },
            #[cfg(any(not(windows), test))]
            DetourRecord::Mock { .. } => {}
        }

        Ok(())
    }

    pub fn remove_all(&self) {
        let mut map = match self.detours.lock() {
            Ok(value) => value,
            Err(err) => err.into_inner(),
        };

        for (_, record) in map.drain() {
            match record {
                #[cfg(all(windows, not(test)))]
                DetourRecord::Active(mut detour) => {
                    let _ = unsafe { detour.disable() };
                }
                #[cfg(any(not(windows), test))]
                DetourRecord::Mock { .. } => {}
            }
        }
    }
}

static DETOUR_MANAGER: OnceLock<DetourManager> = OnceLock::new();

pub fn get_manager() -> &'static DetourManager {
    DETOUR_MANAGER.get_or_init(DetourManager::new)
}

pub fn install(name: &str, target_addr: usize, hook_fn: *const ()) -> Result<(), Box<dyn Error>> {
    get_manager().install(name, target_addr, hook_fn)
}

pub fn uninstall(name: &str) -> Result<(), Box<dyn Error>> {
    get_manager().uninstall(name)
}

pub fn remove_all() {
    get_manager().remove_all()
}

fn detour_candidates() -> [DetourCandidate; 6] {
    [
        DetourCandidate {
            name: "hook_sidl_screen_wnd_init",
            preferred_addr: offsets::SIDL_SCREEN_WND_INIT,
            hook_fn: hook_sidl_screen_wnd_init as *const (),
        },
        DetourCandidate {
            name: "hook_cxwnd_manager_remove_wnd",
            preferred_addr: offsets::CXWND_MANAGER_REMOVE_WND,
            hook_fn: hook_cxwnd_manager_remove_wnd as *const (),
        },
        DetourCandidate {
            name: "hook_cmerchantwnd_purchasepagehandler_update_list",
            preferred_addr: offsets::CMERCHANTWND_PURCHASEPAGEHANDLER_UPDATELIST,
            hook_fn: hook_cmerchantwnd_purchasepagehandler_update_list as *const (),
        },
        DetourCandidate {
            name: "hook_process_mouse_events",
            preferred_addr: offsets::PROCESS_MOUSE_EVENTS,
            hook_fn: hook_process_mouse_events as *const (),
        },
        DetourCandidate {
            name: "hook_process_keyboard_events",
            preferred_addr: offsets::PROCESS_KEYBOARD_EVENTS,
            hook_fn: hook_process_keyboard_events as *const (),
        },
        DetourCandidate {
            name: "hook_crender_reset_device",
            preferred_addr: offsets::CRENDER_RESET_DEVICE,
            hook_fn: hook_crender_reset_device as *const (),
        },
    ]
}

pub fn install_all(eq_base: u64) -> Result<(), Box<dyn Error>> {
    let mut failures = Vec::new();

    for candidate in detour_candidates() {
        let Some(target_addr) = offsets::rebase(candidate.preferred_addr, eq_base) else {
            tracing::warn!(
                name = candidate.name,
                preferred_addr = format!("{:#x}", candidate.preferred_addr),
                "Skipping detour because preferred address is not valid"
            );
            continue;
        };

        if let Err(err) = install(candidate.name, target_addr, candidate.hook_fn) {
            failures.push(format!("{}: {err}", candidate.name));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("detour install failures: {}", failures.join(", ")).into())
    }
}

#[cfg(windows)]
#[allow(non_snake_case)]
pub unsafe extern "system" fn hook_sidl_screen_wnd_init(_this: *mut std::ffi::c_void) {
    tracing::trace!("csidl screen wnd init detour hook invoked");
}

#[cfg(windows)]
pub unsafe extern "system" fn hook_cxwnd_manager_remove_wnd(
    _this: *mut std::ffi::c_void,
    _wnd: *mut std::ffi::c_void,
) {
    tracing::trace!("cxwnd manager remove wnd detour hook invoked");
}

#[cfg(windows)]
pub unsafe extern "system" fn hook_cmerchantwnd_purchasepagehandler_update_list(
    _this: *mut std::ffi::c_void,
) {
    tracing::trace!("merchant purchase page update list detour hook invoked");
}

#[cfg(windows)]
pub unsafe extern "system" fn hook_process_mouse_events() {
    tracing::trace!("process mouse events detour hook invoked");
}

#[cfg(windows)]
pub unsafe extern "system" fn hook_process_keyboard_events() {
    tracing::trace!("process keyboard events detour hook invoked");
}

#[cfg(windows)]
#[allow(non_snake_case)]
pub unsafe extern "system" fn hook_crender_reset_device() {
    tracing::trace!("crender reset device detour hook invoked");
}

#[cfg(not(windows))]
pub fn hook_sidl_screen_wnd_init() {
    tracing::trace!("stub hook_sidl_screen_wnd_init");
}

#[cfg(not(windows))]
pub fn hook_cxwnd_manager_remove_wnd() {
    tracing::trace!("stub hook_cxwnd_manager_remove_wnd");
}

#[cfg(not(windows))]
pub fn hook_cmerchantwnd_purchasepagehandler_update_list() {
    tracing::trace!("stub hook_cmerchantwnd_purchasepagehandler_update_list");
}

#[cfg(not(windows))]
pub fn hook_process_mouse_events() {
    tracing::trace!("stub hook_process_mouse_events");
}

#[cfg(not(windows))]
pub fn hook_process_keyboard_events() {
    tracing::trace!("stub hook_process_keyboard_events");
}

#[cfg(not(windows))]
pub fn hook_crender_reset_device() {
    tracing::trace!("stub hook_crender_reset_device");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detour_install_uninstall_bookkeeping() {
        let manager = DetourManager::new();
        assert!(!manager.is_installed("ui_init"));

        manager
            .install(
                "ui_init",
                0x1111_2222,
                hook_sidl_screen_wnd_init as *const (),
            )
            .expect("install should record detour");
        assert!(manager.is_installed("ui_init"));

        assert!(
            manager
                .install(
                    "ui_init",
                    0x3333_4444,
                    hook_crender_reset_device as *const ()
                )
                .is_err()
        );

        manager
            .uninstall("ui_init")
            .expect("uninstall should remove an installed detour");
        assert!(!manager.is_installed("ui_init"));
    }

    #[test]
    fn uninstall_nonexistent_returns_error() {
        let manager = DetourManager::new();
        let result = manager.uninstall("does_not_exist");
        assert!(result.is_err());
    }

    #[test]
    fn remove_all_clears_everything() {
        let manager = DetourManager::new();
        manager
            .install("a", 0x1000, hook_sidl_screen_wnd_init as *const ())
            .unwrap();
        manager
            .install("b", 0x2000, hook_crender_reset_device as *const ())
            .unwrap();
        assert!(manager.is_installed("a"));
        assert!(manager.is_installed("b"));

        manager.remove_all();
        assert!(!manager.is_installed("a"));
        assert!(!manager.is_installed("b"));
    }

    #[test]
    fn remove_all_on_empty_is_noop() {
        let manager = DetourManager::new();
        manager.remove_all();
        // Should not panic
    }

    #[test]
    fn install_after_uninstall_succeeds() {
        let manager = DetourManager::new();
        manager
            .install("hook", 0x1000, hook_sidl_screen_wnd_init as *const ())
            .unwrap();
        manager.uninstall("hook").unwrap();

        // Re-install at same name should succeed
        let result = manager.install("hook", 0x2000, hook_crender_reset_device as *const ());
        assert!(result.is_ok());
        assert!(manager.is_installed("hook"));
    }

    #[test]
    fn multiple_detours_independent() {
        let manager = DetourManager::new();
        manager
            .install("first", 0x1000, hook_sidl_screen_wnd_init as *const ())
            .unwrap();
        manager
            .install("second", 0x2000, hook_crender_reset_device as *const ())
            .unwrap();

        // Uninstalling one doesn't affect the other
        manager.uninstall("first").unwrap();
        assert!(!manager.is_installed("first"));
        assert!(manager.is_installed("second"));
    }

    #[test]
    fn default_creates_new_manager() {
        let manager = DetourManager::default();
        assert!(!manager.is_installed("anything"));
    }

    #[cfg(windows)]
    #[test]
    fn detour_install_noop_when_offsets_are_placeholder() {
        assert!(install_all(0x1400_0000).is_ok());
    }

    #[test]
    fn detour_candidates_cover_parity_hooks() {
        let names = detour_candidates()
            .iter()
            .map(|candidate| candidate.name)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(names.len(), 6);
        assert!(names.contains("hook_sidl_screen_wnd_init"));
        assert!(names.contains("hook_cxwnd_manager_remove_wnd"));
        assert!(names.contains("hook_cmerchantwnd_purchasepagehandler_update_list"));
        assert!(names.contains("hook_process_mouse_events"));
        assert!(names.contains("hook_process_keyboard_events"));
        assert!(names.contains("hook_crender_reset_device"));
    }
}
