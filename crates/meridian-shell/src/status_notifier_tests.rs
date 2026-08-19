use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Structure, Value};

use super::{
    normalize_menu_label, normalize_service, parse_dbus_menu_layout, snapshot_items,
    DbusMenuItemKind, DbusMenuLayoutNode, DbusMenuProperties, StatusNotifierItem, WatcherState,
};

fn text_value(value: &'static str) -> OwnedValue {
    OwnedValue::try_from(Value::from(value)).expect("test string value converts")
}

fn item_node(id: i32, properties: DbusMenuProperties, children: Vec<OwnedValue>) -> OwnedValue {
    OwnedValue::try_from(Structure::from((id, properties, children)))
        .expect("test menu node converts")
}

fn props(entries: &[(&'static str, OwnedValue)]) -> DbusMenuProperties {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.try_clone().unwrap()))
        .collect::<HashMap<_, _>>()
}

#[test]
fn normalize_service_trims_input() {
    assert_eq!(
        normalize_service("  org.example.Tray  ".to_string()),
        "org.example.Tray"
    );
}

#[test]
fn snapshot_items_is_sorted_and_stable() {
    let mut state = WatcherState::default();
    state.items.insert(
        "z".to_string(),
        StatusNotifierItem {
            service: "z".to_string(),
            title: None,
            icon_name: None,
            menu_path: None,
        },
    );
    state.items.insert(
        "a".to_string(),
        StatusNotifierItem {
            service: "a".to_string(),
            title: Some("A".to_string()),
            icon_name: Some("a-icon".to_string()),
            menu_path: Some("/Menu".to_string()),
        },
    );
    assert_eq!(
        snapshot_items(&state),
        vec![
            StatusNotifierItem {
                service: "a".to_string(),
                title: Some("A".to_string()),
                icon_name: Some("a-icon".to_string()),
                menu_path: Some("/Menu".to_string())
            },
            StatusNotifierItem {
                service: "z".to_string(),
                title: None,
                icon_name: None,
                menu_path: None
            }
        ]
    );
}

#[test]
fn normalize_menu_label_removes_mnemonics() {
    assert_eq!(normalize_menu_label("_Open"), "Open");
    assert_eq!(normalize_menu_label("Save __As"), "Save _As");
    assert_eq!(normalize_menu_label("  E_xit  "), "Exit");
}

#[test]
fn parse_dbus_menu_layout_filters_hidden_items() {
    let root: DbusMenuLayoutNode = (
        0,
        DbusMenuProperties::new(),
        vec![
            item_node(
                1,
                props(&[("label", text_value("_Open")), ("enabled", true.into())]),
                Vec::new(),
            ),
            item_node(
                2,
                props(&[("label", text_value("Hidden")), ("visible", false.into())]),
                Vec::new(),
            ),
        ],
    );
    let menu = parse_dbus_menu_layout(7, root);
    assert_eq!(menu.revision, 7);
    assert_eq!(menu.root_id, 0);
    assert_eq!(menu.items.len(), 1);
    assert_eq!(menu.items[0].id, 1);
    assert_eq!(menu.items[0].label, "Open");
    assert!(menu.items[0].enabled);
    assert_eq!(menu.visible_item_count(), 1);
    assert_eq!(menu.actionable_item_count(), 1);
    assert_eq!(menu.first_item_label(), Some("Open"));
}

#[test]
fn parse_dbus_menu_layout_keeps_separators_and_children() {
    let child = item_node(
        11,
        props(&[("label", text_value("Child")), ("enabled", false.into())]),
        Vec::new(),
    );
    let root: DbusMenuLayoutNode = (
        0,
        DbusMenuProperties::new(),
        vec![
            item_node(10, props(&[("label", text_value("Parent"))]), vec![child]),
            item_node(12, props(&[("type", text_value("separator"))]), Vec::new()),
        ],
    );
    let menu = parse_dbus_menu_layout(1, root);
    assert_eq!(menu.items.len(), 2);
    assert_eq!(menu.items[0].children.len(), 1);
    assert_eq!(menu.items[0].children[0].label, "Child");
    assert!(!menu.items[0].children[0].enabled);
    assert_eq!(menu.items[1].kind, DbusMenuItemKind::Separator);
    assert_eq!(menu.visible_item_count(), 3);
    assert_eq!(menu.actionable_item_count(), 1);
}

#[test]
fn display_entries_flattens_children_with_depth() {
    let child = item_node(2, props(&[("label", text_value("Child"))]), Vec::new());
    let root: DbusMenuLayoutNode = (
        0,
        DbusMenuProperties::new(),
        vec![item_node(
            1,
            props(&[("label", text_value("Parent"))]),
            vec![child],
        )],
    );
    let entries = parse_dbus_menu_layout(1, root).display_entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, 1);
    assert_eq!(entries[0].depth, 0);
    assert_eq!(entries[1].id, 2);
    assert_eq!(entries[1].depth, 1);
}
