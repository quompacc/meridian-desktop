use meridian_ui::{Widget, WidgetPath};

#[allow(dead_code)]
pub(crate) fn find_widget_at_path<'a>(
    root: &'a dyn Widget,
    path: &WidgetPath,
) -> Option<&'a dyn Widget> {
    let mut current = root;
    for &index in path.as_slice() {
        let children = current.children();
        if index >= children.len() {
            return None;
        }
        current = &*children[index];
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use meridian_ui::{
        style::Palette,
        widget::{Button, Container},
        Widget, WidgetPath,
    };

    use super::find_widget_at_path;

    #[test]
    fn find_widget_at_path_empty_returns_root() {
        let root = Container::leaf(Default::default());
        let path = WidgetPath::from_vec(vec![]);
        let found = find_widget_at_path(&root, &path);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), None);
    }

    #[test]
    fn find_widget_at_path_out_of_bounds_returns_none() {
        let root = Container::leaf(Default::default());
        let path = WidgetPath::from_vec(vec![0]);
        let found = find_widget_at_path(&root, &path);
        assert!(found.is_none());
    }

    #[test]
    fn find_widget_at_path_smoke_finds_apps_switch() {
        let pal = Palette::TOKYO_NIGHT_METRO;
        let footer_left = vec![
            Box::new(Button::with_id("apps-switch", "Apps", pal.accent, 144, 48))
                as Box<dyn Widget>,
        ];
        let footer_right = vec![
            Box::new(Button::with_id("power-off", "Off", pal.error, 48, 48)) as Box<dyn Widget>,
        ];
        let footer = Container::footer_row(880, 56, 28, 8, footer_left, footer_right);
        let root = Container::centered_viewport(480, 360, vec![Box::new(footer)]);
        // Tree: root (centered_viewport) -> footer (footer_row) -> left_cluster -> apps-switch button
        // path: [0, 0, 0]
        let path = WidgetPath::from_vec(vec![0, 0, 0]);
        let found = find_widget_at_path(&root, &path);
        assert!(found.is_some());
        assert_eq!(found.and_then(|w| w.id()), Some("apps-switch"));
    }
}
