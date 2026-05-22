// Phase A1 — notification daemon (`org.freedesktop.Notifications`).
//
// Split:
//   state.rs — wire data model (Notification, Urgency, expiry helpers)
//   dbus.rs  — zbus interface implementation + the dbus thread
//
// Entry point: call `notifications::spawn()` once from shell main; it
// returns a calloop `Channel` you register with the loop. The
// resulting events ([`dbus::DbusEvent`]) are processed inline.

pub mod dbus;
pub mod state;

pub use dbus::{spawn, DbusEvent};
pub use state::Notification;
