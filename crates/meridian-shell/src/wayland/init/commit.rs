use smithay_client_toolkit::shell::WaylandSurface;
use tracing::info;

use super::super::{CommitReason, CommitSurfaceKind, MeridianShell};

pub(super) fn commit_initial_surfaces(shell: &mut MeridianShell) {
    shell.desktop_layer.commit();
    info!("Desktop background surface committed without input buffer");
    shell.desktop_menu_layer.commit();
    info!("Desktop menu surface created and committed");
    shell.commit_surface(CommitSurfaceKind::Panel, CommitReason::InitialCreate);
    info!("Panel surface created and committed");
    shell.commit_surface(CommitSurfaceKind::Launcher, CommitReason::InitialCreate);
    info!("Launcher surface created and committed");
    shell.calendar_layer.commit();
    info!("Calendar popup surface created and committed");
    shell.workspace_layer.commit();
    info!("Workspace popup surface created and committed");
    shell.network_layer.commit();
    info!("Network popup surface created and committed");
    shell.notification_layer.commit();
    info!("Notification surface created and committed");
    shell.thumbnail_layer.commit();
    shell.consent_layer.commit();
    info!("Screenshot consent surface initial commit");
    shell.wifi_modal_layer.commit();
    info!("Wi-Fi password modal surface initial commit");
    shell.region_picker_layer.commit();
    info!("Screenshot region picker surface initial commit");
}
