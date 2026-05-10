use meridian_portal::start_dbus_service;
use tracing::{info, warn};

fn main() {
    tracing_subscriber::fmt::init();

    info!("meridian-portal starting");
    let service = match start_dbus_service() {
        Ok(service) => service,
        Err(err) => {
            warn!("portal backend scaffold failed to initialize: {}", err);
            return;
        }
    };

    if service.state.ready {
        info!("portal backend scaffold ready");
    }

    service.run();
}
