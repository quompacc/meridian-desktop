use meridian_compositor::backend::drm::{layer_role, render_stack_order, RenderStackRole};
use smithay::wayland::shell::wlr_layer::Layer as WlrLayer;

#[test]
fn layer_surface_maps_correctly() {
    assert_eq!(layer_role(WlrLayer::Overlay), RenderStackRole::TopLayer);
    assert_eq!(layer_role(WlrLayer::Top), RenderStackRole::TopLayer);
    assert_eq!(layer_role(WlrLayer::Bottom), RenderStackRole::BottomLayer);
    assert_eq!(
        layer_role(WlrLayer::Background),
        RenderStackRole::BottomLayer
    );
}

#[test]
fn render_elements_include_layer_surfaces() {
    let order = render_stack_order(0, 2, 0, 0, 1, 0);

    assert_eq!(
        order,
        vec![
            RenderStackRole::TopLayer,
            RenderStackRole::TopLayer,
            RenderStackRole::BottomLayer,
        ]
    );
}
