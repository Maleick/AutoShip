//! Toast notification system — ephemeral messages for achievements, warnings,
//! and status.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

// ─── Toast level ────────────────────────────────────────────────────────────

/// Severity / category of a toast notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToastLevel {
    /// Informational — neutral status updates.
    Info,
    /// Success — something good happened.
    Success,
    /// Warning — attention needed but not critical.
    Warning,
    /// Achievement unlocked — special celebratory display.
    Achievement,
}

// ─── Toast ──────────────────────────────────────────────────────────────────

/// A single toast notification with a message, level, and expiry.
#[derive(Debug, Clone)]
pub struct Toast {
    /// The message body displayed to the user.
    pub message: String,
    /// Severity / display category.
    pub level: ToastLevel,
    /// When this toast was created.
    pub created_at: Instant,
    /// How long the toast should remain visible.
    pub duration: Duration,
    /// Optional icon or emoji prefix (e.g. "trophy" for achievements).
    pub icon: Option<String>,
}

impl Toast {
    /// Creates a new toast with the given message and level.
    /// Default duration is 5 seconds.
    #[must_use]
    pub fn new(message: impl Into<String>, level: ToastLevel) -> Self {
        Self {
            message: message.into(),
            level,
            created_at: Instant::now(),
            duration: Duration::from_secs(5),
            icon: None,
        }
    }

    /// Sets a custom duration for this toast.
    #[must_use]
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Sets an icon/emoji prefix for this toast.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Whether this toast has expired based on elapsed time since creation.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }
}

// ─── Manager ────────────────────────────────────────────────────────────────

/// Manages a queue of active toast notifications.
#[derive(Debug, Default)]
pub struct ToastManager {
    toasts: Vec<Toast>,
}

impl ToastManager {
    /// Creates an empty toast manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a new toast onto the queue.
    pub fn push(&mut self, toast: Toast) {
        self.toasts.push(toast);
    }

    /// Removes expired toasts from the queue. Call this every tick.
    pub fn tick(&mut self) {
        self.toasts.retain(|t| !t.is_expired());
    }

    /// Returns currently active (non-expired) toasts, oldest first.
    #[must_use]
    pub fn active(&self) -> &[Toast] {
        &self.toasts
    }

    /// Number of active toasts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.toasts.len()
    }

    /// Whether there are no active toasts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    /// Removes all toasts.
    pub fn clear(&mut self) {
        self.toasts.clear();
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_toast_defaults() {
        let toast = Toast::new("Test message", ToastLevel::Info);
        assert_eq!(toast.message, "Test message");
        assert_eq!(toast.level, ToastLevel::Info);
        assert_eq!(toast.duration, Duration::from_secs(5));
        assert!(toast.icon.is_none());
        assert!(!toast.is_expired());
    }

    #[test]
    fn toast_builder_methods() {
        let toast = Toast::new("Achievement!", ToastLevel::Achievement)
            .with_duration(Duration::from_secs(10))
            .with_icon("trophy");

        assert_eq!(toast.level, ToastLevel::Achievement);
        assert_eq!(toast.duration, Duration::from_secs(10));
        assert_eq!(toast.icon.as_deref(), Some("trophy"));
    }

    #[test]
    fn toast_expiry_with_zero_duration() {
        let toast = Toast::new("Gone", ToastLevel::Warning).with_duration(Duration::ZERO);
        assert!(toast.is_expired());
    }

    #[test]
    fn manager_starts_empty() {
        let manager = ToastManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
        assert!(manager.active().is_empty());
    }

    #[test]
    fn push_and_active() {
        let mut manager = ToastManager::new();
        manager.push(Toast::new("First", ToastLevel::Info));
        manager.push(Toast::new("Second", ToastLevel::Success));

        assert_eq!(manager.len(), 2);
        assert!(!manager.is_empty());
        assert_eq!(manager.active()[0].message, "First");
        assert_eq!(manager.active()[1].message, "Second");
    }

    #[test]
    fn tick_removes_expired() {
        let mut manager = ToastManager::new();
        manager.push(Toast::new("Expired", ToastLevel::Warning).with_duration(Duration::ZERO));
        manager.push(Toast::new("Alive", ToastLevel::Info).with_duration(Duration::from_secs(60)));

        manager.tick();
        assert_eq!(manager.len(), 1);
        assert_eq!(manager.active()[0].message, "Alive");
    }

    #[test]
    fn tick_removes_all_expired() {
        let mut manager = ToastManager::new();
        manager.push(Toast::new("A", ToastLevel::Info).with_duration(Duration::ZERO));
        manager.push(Toast::new("B", ToastLevel::Warning).with_duration(Duration::ZERO));

        manager.tick();
        assert!(manager.is_empty());
    }

    #[test]
    fn clear_removes_all() {
        let mut manager = ToastManager::new();
        manager.push(Toast::new("A", ToastLevel::Info));
        manager.push(Toast::new("B", ToastLevel::Success));
        manager.clear();
        assert!(manager.is_empty());
    }

    #[test]
    fn queue_ordering_preserved() {
        let mut manager = ToastManager::new();
        for i in 0..5 {
            manager.push(Toast::new(format!("Toast {i}"), ToastLevel::Info));
        }
        let messages: Vec<&str> = manager
            .active()
            .iter()
            .map(|t| t.message.as_str())
            .collect();
        assert_eq!(
            messages,
            vec!["Toast 0", "Toast 1", "Toast 2", "Toast 3", "Toast 4"]
        );
    }
}
