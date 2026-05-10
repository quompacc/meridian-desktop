use smithay::wayland::shm::{ShmHandler, ShmState};

use super::super::super::MeridianState;

impl ShmHandler for MeridianState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}
