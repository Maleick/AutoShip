//! Periodic hook rotation for anti-detection.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
#[cfg(windows)]
use std::time::{Duration, Instant};

type HookFn = Box<dyn Fn() + Send + Sync + 'static>;

/// A single hook that can be temporarily removed and then reinstalled.
#[derive(Clone)]
pub struct RotatableHook {
    pub name: String,
    pub unhook_fn: Arc<HookFn>,
    pub rehook_fn: Arc<HookFn>,
    pub target_addr: usize,
}

impl RotatableHook {
    fn new(
        name: String,
        unhook_fn: impl Fn() + Send + Sync + 'static,
        rehook_fn: impl Fn() + Send + Sync + 'static,
        target_addr: usize,
    ) -> Self {
        Self {
            name,
            unhook_fn: Arc::new(Box::new(unhook_fn)),
            rehook_fn: Arc::new(Box::new(rehook_fn)),
            target_addr,
        }
    }
}

type ThreadHandle = thread::JoinHandle<()>;

/// Periodically toggles registered hooks by unhooking then rehooking them.
pub struct HookRotationManager {
    hooks: Arc<Mutex<Vec<RotatableHook>>>,
    stop_requested: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    rotation_count: Arc<AtomicU64>,
    thread_handle: Arc<Mutex<Option<ThreadHandle>>>,
}

impl Default for HookRotationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRotationManager {
    /// Create a new manager with no hooks and no running thread.
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(Mutex::new(Vec::new())),
            stop_requested: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
            rotation_count: Arc::new(AtomicU64::new(0)),
            thread_handle: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg_attr(not(windows), allow(unused_variables))]
    pub fn register_with_addr(
        &self,
        name: &str,
        unhook_fn: impl Fn() + Send + Sync + 'static,
        rehook_fn: impl Fn() + Send + Sync + 'static,
        target_addr: usize,
    ) {
        if let Ok(mut hooks) = self.hooks.lock() {
            hooks.push(RotatableHook::new(
                name.to_string(),
                unhook_fn,
                rehook_fn,
                target_addr,
            ));
        }
    }

    pub fn register(
        &self,
        name: &str,
        unhook_fn: impl Fn() + Send + Sync + 'static,
        rehook_fn: impl Fn() + Send + Sync + 'static,
    ) {
        self.register_with_addr(name, unhook_fn, rehook_fn, 0);
    }

    #[cfg(windows)]
    pub fn start(&self, interval_ms: u64) {
        let should_start = self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        if !should_start {
            return;
        }

        self.stop_requested.store(false, Ordering::Release);

        let hooks = Arc::clone(&self.hooks);
        let stop_requested = Arc::clone(&self.stop_requested);
        let running = Arc::clone(&self.running);
        let rotation_count = Arc::clone(&self.rotation_count);

        let thread_handle = thread::Builder::new()
            .name("textquest-hook-rotation".into())
            .spawn(move || {
                while !stop_requested.load(Ordering::Acquire) {
                    let start = Instant::now();
                    let hooks_snapshot = hooks.lock().ok().map(|hooks| hooks.clone());
                    if let Some(hooks_snapshot) = hooks_snapshot {
                        for hook in hooks_snapshot.iter() {
                            (hook.unhook_fn)();
                        }

                        thread::sleep(Duration::from_millis(50));

                        for hook in hooks_snapshot.iter() {
                            (hook.rehook_fn)();
                        }
                    }

                    rotation_count.fetch_add(1, Ordering::AcqRel);

                    if interval_ms == 0 {
                        continue;
                    }

                    let elapsed = start.elapsed();
                    if elapsed < Duration::from_millis(interval_ms) {
                        thread::sleep(Duration::from_millis(interval_ms) - elapsed);
                    }
                }
                running.store(false, Ordering::Release);
            })
            .ok();

        let spawn_failed = thread_handle.is_none();
        if let Ok(mut handle_slot) = self.thread_handle.lock() {
            *handle_slot = thread_handle;
        }

        if spawn_failed {
            self.running.store(false, Ordering::Release);
        }
    }

    #[cfg(not(windows))]
    pub fn start(&self, _interval_ms: u64) {
        self.running.store(false, Ordering::Release);
    }

    #[cfg(windows)]
    pub fn stop(&self) {
        self.stop_requested.store(true, Ordering::Release);

        if let Ok(mut handle_slot) = self.thread_handle.lock()
            && let Some(handle) = handle_slot.take()
        {
            let _ = handle.join();
        }

        self.running.store(false, Ordering::Release);
    }

    #[cfg(not(windows))]
    pub fn stop(&self) {
        self.stop_requested.store(true, Ordering::Release);
        self.running.store(false, Ordering::Release);
    }

    pub fn rotation_count(&self) -> u64 {
        self.rotation_count.load(Ordering::Acquire)
    }
}

#[cfg(windows)]
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use super::HookRotationManager;

    #[test]
    fn start_register_stop() {
        let manager = HookRotationManager::new();
        let unhooks = std::sync::Arc::new(AtomicU64::new(0));
        let rehooks = std::sync::Arc::new(AtomicU64::new(0));
        let unhooks_clone = std::sync::Arc::clone(&unhooks);
        let rehooks_clone = std::sync::Arc::clone(&rehooks);

        manager.register(
            "unit-test",
            move || {
                unhooks_clone.fetch_add(1, Ordering::Relaxed);
            },
            move || {
                rehooks_clone.fetch_add(1, Ordering::Relaxed);
            },
        );

        manager.start(100);
        std::thread::sleep(Duration::from_millis(350));
        manager.stop();

        assert!(unhooks.load(Ordering::Acquire) > 0);
        assert!(rehooks.load(Ordering::Acquire) > 0);
        assert!(manager.rotation_count() > 0);
    }
}

#[cfg(not(windows))]
#[cfg(test)]
mod tests {
    use super::HookRotationManager;

    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn start_register_stop_stubbed() {
        let manager = HookRotationManager::new();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        let called_clone2 = Arc::clone(&called);

        manager.register(
            "stubbed",
            move || {
                called_clone.store(true, Ordering::Release);
            },
            move || {
                called_clone2.store(true, Ordering::Release);
            },
        );

        manager.start(100);
        manager.stop();
        assert!(!called.load(Ordering::Acquire));
        assert_eq!(manager.rotation_count(), 0);
    }
}
