use smithay::wayland::shell::wlr_layer::Layer as WlrLayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStackRole {
    Cursor,
    TopLayer,
    Decoration,
    Window,
    BottomLayer,
    Wallpaper,
}

pub fn layer_role(layer: WlrLayer) -> RenderStackRole {
    match layer {
        WlrLayer::Top | WlrLayer::Overlay => RenderStackRole::TopLayer,
        WlrLayer::Background | WlrLayer::Bottom => RenderStackRole::BottomLayer,
    }
}

pub fn render_stack_order(
    cursor_count: usize,
    top_layer_count: usize,
    decoration_count: usize,
    window_count: usize,
    bottom_layer_count: usize,
    wallpaper_count: usize,
) -> Vec<RenderStackRole> {
    let mut roles = Vec::with_capacity(
        cursor_count
            + top_layer_count
            + decoration_count
            + window_count
            + bottom_layer_count
            + wallpaper_count,
    );
    roles.extend(std::iter::repeat(RenderStackRole::Cursor).take(cursor_count));
    roles.extend(std::iter::repeat(RenderStackRole::TopLayer).take(top_layer_count));
    roles.extend(std::iter::repeat(RenderStackRole::Decoration).take(decoration_count));
    roles.extend(std::iter::repeat(RenderStackRole::Window).take(window_count));
    roles.extend(std::iter::repeat(RenderStackRole::BottomLayer).take(bottom_layer_count));
    roles.extend(std::iter::repeat(RenderStackRole::Wallpaper).take(wallpaper_count));
    roles
}

#[cfg(test)]
mod tests {
    use super::{layer_role, render_stack_order, RenderStackRole};
    use smithay::wayland::shell::wlr_layer::Layer as WlrLayer;

    #[test]
    fn element_order_is_front_to_back() {
        assert_eq!(
            render_stack_order(1, 1, 1, 1, 1, 1),
            vec![
                RenderStackRole::Cursor,
                RenderStackRole::TopLayer,
                RenderStackRole::Decoration,
                RenderStackRole::Window,
                RenderStackRole::BottomLayer,
                RenderStackRole::Wallpaper,
            ]
        );
    }

    #[test]
    fn render_stack_without_cursor_has_no_cursor_role() {
        let order = render_stack_order(0, 1, 1, 1, 1, 1);
        assert!(!order.contains(&RenderStackRole::Cursor));
    }

    #[test]
    fn render_stack_empty_returns_empty_vec() {
        assert_eq!(
            render_stack_order(0, 0, 0, 0, 0, 0),
            Vec::<RenderStackRole>::new()
        );
    }

    #[test]
    fn cursor_role_is_always_first_when_present() {
        let order = render_stack_order(2, 3, 1, 4, 2, 1);
        assert_eq!(order[0], RenderStackRole::Cursor);
        assert_eq!(order[1], RenderStackRole::Cursor);
        assert_eq!(order[2], RenderStackRole::TopLayer);
    }

    #[test]
    fn layer_role_overlay_and_top_are_top_layer() {
        assert_eq!(layer_role(WlrLayer::Overlay), RenderStackRole::TopLayer);
        assert_eq!(layer_role(WlrLayer::Top), RenderStackRole::TopLayer);
    }

    #[test]
    fn layer_role_background_and_bottom_are_bottom_layer() {
        assert_eq!(
            layer_role(WlrLayer::Background),
            RenderStackRole::BottomLayer
        );
        assert_eq!(layer_role(WlrLayer::Bottom), RenderStackRole::BottomLayer);
    }
}
