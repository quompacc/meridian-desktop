// meridian-login — display manager for Meridian.
//
// Phase 1: scaffold only. Logs start/exit and returns 0.
// No DRM, no PAM, no IPC, no rendering yet — see docs/MERIDIAN_LOGIN.md
// for the phased plan.

use tracing::info;

fn main() {
    tracing_subscriber::fmt::init();

    info!("meridian-login starting");
    info!("meridian-login exiting");
}
