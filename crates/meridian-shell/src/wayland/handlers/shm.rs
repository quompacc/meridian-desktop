use smithay_client_toolkit::shm::{Shm, ShmHandler};

use crate::wayland::MeridianShell;

impl ShmHandler for MeridianShell {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
