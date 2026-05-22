// Notification state: the wire data model we receive from dbus and the
// queue the shell main loop renders. Kept deliberately small for v1 —
// just title, body, app, urgency, expire timeout. Actions, action-icons,
// inline images, replaces_id are future work.

use std::time::{Duration, Instant};

/// Default time a notification stays on screen if the sender did not
/// specify an `expire_timeout`. The freedesktop spec says -1 means
/// "server-default", 0 means "never expire". We treat positive ms as
/// authoritative.
pub const DEFAULT_EXPIRE_MS: i32 = 5_000;

/// `urgency` hint from the freedesktop spec.
/// 0 = low, 1 = normal, 2 = critical. Anything else maps to Normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

impl Urgency {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => Urgency::Low,
            2 => Urgency::Critical,
            _ => Urgency::Normal,
        }
    }
}

/// One in-flight notification, populated from a successful
/// `org.freedesktop.Notifications.Notify` call.
#[derive(Debug, Clone)]
pub struct Notification {
    /// Server-assigned ID, monotonically increasing from 1. 0 is reserved
    /// by the spec as "not a valid id". We use this for CloseNotification
    /// lookups and for the NotificationClosed signal.
    pub id: u32,
    /// `app_name` from the caller — usually the sending app's binary
    /// or freedesktop ID. Used as a small label above the title.
    pub app: String,
    /// `summary` field — the one-line title. Rendered prominently.
    pub title: String,
    /// `body` field — multi-line message body. Optional (may be empty).
    pub body: String,
    pub urgency: Urgency,
    pub created_at: Instant,
    /// Effective expiry duration after which the popup auto-dismisses.
    /// `None` means "never auto-expire" (= the sender passed 0).
    pub expires_in: Option<Duration>,
}

impl Notification {
    /// True if `created_at + expires_in` is in the past.
    pub fn is_expired(&self, now: Instant) -> bool {
        match self.expires_in {
            Some(ttl) => now.saturating_duration_since(self.created_at) >= ttl,
            None => false,
        }
    }
}

/// Compute the effective `expires_in` from a freedesktop `expire_timeout`
/// (signed ms, with -1 = server-default and 0 = never).
pub fn expires_in_from_timeout(timeout_ms: i32) -> Option<Duration> {
    match timeout_ms {
        0 => None,
        t if t < 0 => Some(Duration::from_millis(DEFAULT_EXPIRE_MS as u64)),
        t => Some(Duration::from_millis(t as u64)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_zero_means_never() {
        assert_eq!(expires_in_from_timeout(0), None);
    }

    #[test]
    fn timeout_negative_means_default() {
        assert_eq!(
            expires_in_from_timeout(-1),
            Some(Duration::from_millis(DEFAULT_EXPIRE_MS as u64))
        );
    }

    #[test]
    fn timeout_positive_used_as_is() {
        assert_eq!(
            expires_in_from_timeout(3_000),
            Some(Duration::from_millis(3_000))
        );
    }

    #[test]
    fn urgency_byte_mapping() {
        assert_eq!(Urgency::from_byte(0), Urgency::Low);
        assert_eq!(Urgency::from_byte(1), Urgency::Normal);
        assert_eq!(Urgency::from_byte(2), Urgency::Critical);
        // Anything else => Normal (defensive).
        assert_eq!(Urgency::from_byte(7), Urgency::Normal);
    }

    #[test]
    fn expired_when_ttl_elapsed() {
        let now = Instant::now();
        let n = Notification {
            id: 1,
            app: "test".into(),
            title: "t".into(),
            body: "b".into(),
            urgency: Urgency::Normal,
            created_at: now - Duration::from_millis(10_000),
            expires_in: Some(Duration::from_millis(5_000)),
        };
        assert!(n.is_expired(now));
    }

    #[test]
    fn never_expires_with_none() {
        let now = Instant::now();
        let n = Notification {
            id: 1,
            app: "test".into(),
            title: "t".into(),
            body: "b".into(),
            urgency: Urgency::Normal,
            created_at: now - Duration::from_secs(3600),
            expires_in: None,
        };
        assert!(!n.is_expired(now));
    }
}
