use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Rectangle},
    wayland::{
        fractional_scale::FractionalScaleHandler,
        input_method::{InputMethodHandler, PopupSurface},
        xdg_activation::{
            XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
        },
    },
};

use crate::state::MeridianState;

impl XdgActivationHandler for MeridianState {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.xdg_activation_state
    }

    fn token_created(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        tracing::debug!(
            "xdg-activation: token created (client_id={:?}, has_surface={})",
            data.client_id,
            data.surface.is_some()
        );
        true
    }

    fn request_activation(
        &mut self,
        _token: XdgActivationToken,
        _token_data: XdgActivationTokenData,
        _surface: WlSurface,
    ) {
        tracing::debug!("xdg-activation: activation requested");
    }
}

impl FractionalScaleHandler for MeridianState {
    fn new_fractional_scale(&mut self, _surface: WlSurface) {}
}

impl InputMethodHandler for MeridianState {
    fn new_popup(&mut self, _surface: PopupSurface) {}

    fn popup_repositioned(&mut self, _surface: PopupSurface) {}

    fn dismiss_popup(&mut self, _surface: PopupSurface) {}

    fn parent_geometry(&self, _parent: &WlSurface) -> Rectangle<i32, Logical> {
        Rectangle::default()
    }
}
