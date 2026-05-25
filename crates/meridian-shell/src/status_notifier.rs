use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use smithay_client_toolkit::reexports::calloop::channel as cchannel;
use tracing::{debug, error, info, warn};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{connection::Builder, fdo, interface, Connection, Proxy};

const SERVICE_NAME: &str = "org.kde.StatusNotifierWatcher";
const OBJECT_PATH: &str = "/StatusNotifierWatcher";

type DbusMenuProperties = std::collections::HashMap<String, OwnedValue>;
type DbusMenuLayoutNode = (i32, DbusMenuProperties, Vec<OwnedValue>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusNotifierItem {
    pub service: String,
    pub title: Option<String>,
    pub icon_name: Option<String>,
    pub menu_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbusEvent {
    ItemsChanged(Vec<StatusNotifierItem>),
    MenuLayout(StatusNotifierMenuState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusNotifierMenuState {
    pub service: String,
    pub menu_path: String,
    pub point: ActivationPoint,
    pub menu: DbusMenu,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbusMenu {
    revision: u32,
    root_id: i32,
    items: Vec<DbusMenuItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbusMenuItem {
    id: i32,
    label: String,
    enabled: bool,
    kind: DbusMenuItemKind,
    children: Vec<DbusMenuItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbusMenuEntry {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DbusMenuItemKind {
    Standard,
    Separator,
}

impl DbusMenu {
    pub(crate) fn visible_item_count(&self) -> usize {
        self.items.iter().map(DbusMenuItem::subtree_count).sum()
    }

    pub(crate) fn actionable_item_count(&self) -> usize {
        self.items.iter().map(DbusMenuItem::actionable_count).sum()
    }

    pub(crate) fn first_item_label(&self) -> Option<&str> {
        self.items.iter().find_map(DbusMenuItem::first_label)
    }

    pub(crate) fn display_entries(&self) -> Vec<DbusMenuEntry> {
        let mut entries = Vec::new();
        for item in &self.items {
            item.push_display_entries(0, &mut entries);
        }
        entries
    }
}

impl DbusMenuItem {
    fn subtree_count(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(DbusMenuItem::subtree_count)
            .sum::<usize>()
    }

    fn actionable_count(&self) -> usize {
        let self_count = usize::from(self.enabled && self.kind == DbusMenuItemKind::Standard);
        self_count
            + self
                .children
                .iter()
                .map(DbusMenuItem::actionable_count)
                .sum::<usize>()
    }

    fn first_label(&self) -> Option<&str> {
        if !self.label.is_empty() {
            return Some(self.label.as_str());
        }
        self.children.iter().find_map(DbusMenuItem::first_label)
    }

    fn push_display_entries(&self, depth: usize, entries: &mut Vec<DbusMenuEntry>) {
        entries.push(DbusMenuEntry {
            id: self.id,
            label: self.label.clone(),
            enabled: self.enabled,
            separator: self.kind == DbusMenuItemKind::Separator,
            depth,
        });
        for child in &self.children {
            child.push_display_entries(depth + 1, entries);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActivationPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivationKind {
    Activate,
    SecondaryActivate,
    ContextMenu,
}

impl ActivationKind {
    fn method_name(self) -> &'static str {
        match self {
            ActivationKind::Activate => "Activate",
            ActivationKind::SecondaryActivate => "SecondaryActivate",
            ActivationKind::ContextMenu => "ContextMenu",
        }
    }

    fn log_name(self) -> &'static str {
        match self {
            ActivationKind::Activate => "activate",
            ActivationKind::SecondaryActivate => "secondary-activate",
            ActivationKind::ContextMenu => "context-menu",
        }
    }
}

#[derive(Default)]
struct WatcherState {
    items: BTreeMap<String, StatusNotifierItem>,
    hosts: BTreeSet<String>,
}

struct StatusNotifierWatcher {
    state: Arc<Mutex<WatcherState>>,
    tx: cchannel::Sender<DbusEvent>,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    async fn register_status_notifier_item(
        &self,
        #[zbus(connection)] connection: &Connection,
        service: String,
    ) -> fdo::Result<()> {
        let service = normalize_service(service);
        if service.is_empty() {
            return Err(fdo::Error::InvalidArgs(
                "empty notifier service".to_string(),
            ));
        }
        let item = resolve_item_details(connection, &service).await;
        let items = {
            let mut state = self.state.lock().expect("status notifier state lock");
            state.items.insert(service.clone(), item.clone());
            snapshot_items(&state)
        };
        info!(
            service = %service,
            title = item.title.as_deref().unwrap_or(""),
            icon_name = item.icon_name.as_deref().unwrap_or(""),
            "status-notifier: item registered"
        );
        self.send_items(items);
        Ok(())
    }

    async fn register_status_notifier_host(&self, service: String) -> fdo::Result<()> {
        let service = normalize_service(service);
        if service.is_empty() {
            return Err(fdo::Error::InvalidArgs("empty notifier host".to_string()));
        }
        let mut state = self.state.lock().expect("status notifier state lock");
        state.hosts.insert(service.clone());
        info!(service = %service, "status-notifier: host registered");
        Ok(())
    }

    #[zbus(property)]
    async fn registered_status_notifier_items(&self) -> Vec<String> {
        let state = self.state.lock().expect("status notifier state lock");
        state.items.keys().cloned().collect()
    }

    #[zbus(property)]
    async fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    async fn protocol_version(&self) -> i32 {
        0
    }
}

impl StatusNotifierWatcher {
    fn send_items(&self, items: Vec<StatusNotifierItem>) {
        if let Err(e) = self.tx.send(DbusEvent::ItemsChanged(items)) {
            warn!(error = ?e, "status-notifier: main loop channel closed; dropping update");
        }
    }
}

pub(crate) struct StatusNotifierSource {
    pub rx: cchannel::Channel<DbusEvent>,
    pub tx: cchannel::Sender<DbusEvent>,
}

pub fn spawn() -> std::io::Result<StatusNotifierSource> {
    let (tx, rx) = cchannel::channel::<DbusEvent>();
    let watcher_tx = tx.clone();
    std::thread::Builder::new()
        .name("status-notifier-dbus".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    error!(error = %e, "status-notifier: tokio runtime build failed; watcher disabled");
                    return;
                }
            };
            rt.block_on(async move {
                if let Err(e) = run(watcher_tx).await {
                    error!(
                        error = %e,
                        "status-notifier: dbus serve failed; watcher disabled until shell restarts"
                    );
                }
            });
        })?;
    Ok(StatusNotifierSource { rx, tx })
}

pub(crate) fn activate_item(item: StatusNotifierItem, point: ActivationPoint) {
    forward_item_activation(item, point, ActivationKind::Activate);
}

pub(crate) fn secondary_activate_item(item: StatusNotifierItem, point: ActivationPoint) {
    forward_item_activation(item, point, ActivationKind::SecondaryActivate);
}

pub(crate) fn activate_menu_item(service: String, menu_path: String, item_id: i32) {
    let builder = std::thread::Builder::new().name("status-notifier-dbusmenu-event".to_string());
    if let Err(e) = builder.spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                error!(error = %e, service = %service, menu_path = %menu_path, item_id, "status-notifier: dbusmenu event runtime build failed");
                return;
            }
        };
        rt.block_on(async move {
            match send_dbus_menu_event(&service, &menu_path, item_id).await {
                Ok(()) => info!(service = %service, menu_path = %menu_path, item_id, "status-notifier: dbusmenu clicked event sent"),
                Err(e) => warn!(error = %e, service = %service, menu_path = %menu_path, item_id, "status-notifier: dbusmenu clicked event failed"),
            }
        });
    }) {
        warn!(error = %e, "status-notifier: dbusmenu event thread spawn failed");
    }
}

pub(crate) fn context_menu_item(
    item: StatusNotifierItem,
    point: ActivationPoint,
    tx: Option<cchannel::Sender<DbusEvent>>,
) {
    let service = item.service.clone();
    let menu_path = item.menu_path.clone();
    forward_item_activation(item, point, ActivationKind::ContextMenu);
    if let Some(menu_path) = menu_path {
        inspect_dbus_menu(service, menu_path, point, tx);
    }
}

fn forward_item_activation(item: StatusNotifierItem, point: ActivationPoint, kind: ActivationKind) {
    let service = item.service;
    let title = item.title.unwrap_or_default();
    let builder = std::thread::Builder::new().name(format!("status-notifier-{}", kind.log_name()));
    if let Err(e) = builder.spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                error!(
                    error = %e,
                    service = %service,
                    method = kind.method_name(),
                    "status-notifier: activation runtime build failed"
                );
                return;
            }
        };
        rt.block_on(async move {
            match forward_activation(&service, point, kind).await {
                Ok(()) => {
                    info!(
                        service = %service,
                        title = %title,
                        x = point.x,
                        y = point.y,
                        method = kind.method_name(),
                        "status-notifier: activation forwarded"
                    );
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        service = %service,
                        x = point.x,
                        y = point.y,
                        method = kind.method_name(),
                        "status-notifier: activation failed"
                    );
                }
            }
        });
    }) {
        warn!(
            error = %e,
            method = kind.method_name(),
            "status-notifier: activation thread spawn failed"
        );
    }
}

async fn run(tx: cchannel::Sender<DbusEvent>) -> zbus::Result<()> {
    let watcher = StatusNotifierWatcher {
        state: Arc::new(Mutex::new(WatcherState::default())),
        tx,
    };
    let _conn = Builder::session()?
        .name(SERVICE_NAME)?
        .serve_at(OBJECT_PATH, watcher)?
        .build()
        .await?;
    info!(
        service = SERVICE_NAME,
        path = OBJECT_PATH,
        "status-notifier: watcher ready"
    );
    std::future::pending::<()>().await;
    Ok(())
}

async fn forward_activation(
    service: &str,
    point: ActivationPoint,
    kind: ActivationKind,
) -> zbus::Result<()> {
    let connection = Connection::session().await?;
    let proxy = Proxy::new(
        &connection,
        service,
        "/StatusNotifierItem",
        "org.kde.StatusNotifierItem",
    )
    .await?;
    proxy
        .call_method(kind.method_name(), &(point.x, point.y))
        .await?;
    Ok(())
}

fn inspect_dbus_menu(
    service: String,
    menu_path: String,
    point: ActivationPoint,
    tx: Option<cchannel::Sender<DbusEvent>>,
) {
    let builder = std::thread::Builder::new().name("status-notifier-dbusmenu".to_string());
    if let Err(e) = builder.spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                error!(
                    error = %e,
                    service = %service,
                    menu_path = %menu_path,
                    "status-notifier: dbusmenu runtime build failed"
                );
                return;
            }
        };
        rt.block_on(async move {
            match fetch_dbus_menu_layout(&service, &menu_path).await {
                Ok(menu) => {
                    info!(
                        service = %service,
                        menu_path = %menu_path,
                        revision = menu.revision,
                        root_id = menu.root_id,
                        visible_items = menu.visible_item_count(),
                        actionable_items = menu.actionable_item_count(),
                        first_label = menu.first_item_label().unwrap_or(""),
                        "status-notifier: dbusmenu layout parsed"
                    );
                    if let Some(tx) = tx.as_ref() {
                        let event = DbusEvent::MenuLayout(StatusNotifierMenuState {
                            service: service.clone(),
                            menu_path: menu_path.clone(),
                            point,
                            menu,
                        });
                        if let Err(e) = tx.send(event) {
                            warn!(
                                error = ?e,
                                service = %service,
                                menu_path = %menu_path,
                                "status-notifier: main loop channel closed; dropping dbusmenu layout"
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        service = %service,
                        menu_path = %menu_path,
                        "status-notifier: dbusmenu layout fetch failed"
                    );
                }
            }
        });
    }) {
        warn!(error = %e, "status-notifier: dbusmenu thread spawn failed");
    }
}

async fn send_dbus_menu_event(service: &str, menu_path: &str, item_id: i32) -> zbus::Result<()> {
    let connection = Connection::session().await?;
    let proxy = Proxy::new(&connection, service, menu_path, "com.canonical.dbusmenu").await?;
    proxy
        .call_method("Event", &(item_id, "clicked", Value::from(0i32), 0u32))
        .await?;
    Ok(())
}

async fn fetch_dbus_menu_layout(service: &str, menu_path: &str) -> zbus::Result<DbusMenu> {
    let connection = Connection::session().await?;
    let proxy = Proxy::new(&connection, service, menu_path, "com.canonical.dbusmenu").await?;
    let reply = proxy
        .call_method("GetLayout", &(0i32, 1i32, Vec::<&str>::new()))
        .await?;
    let body = reply.body();
    let (revision, root): (u32, DbusMenuLayoutNode) = body.deserialize()?;
    Ok(parse_dbus_menu_layout(revision, root))
}

fn parse_dbus_menu_layout(revision: u32, root: DbusMenuLayoutNode) -> DbusMenu {
    let root_id = root.0;
    let items = root
        .2
        .into_iter()
        .filter_map(parse_dbus_menu_item)
        .collect();
    DbusMenu {
        revision,
        root_id,
        items,
    }
}

fn parse_dbus_menu_item(value: OwnedValue) -> Option<DbusMenuItem> {
    let (id, properties, children): DbusMenuLayoutNode = value.try_into().ok()?;
    if !property_bool(&properties, "visible", true) {
        return None;
    }
    let kind = match property_string(&properties, "type").as_deref() {
        Some("separator") => DbusMenuItemKind::Separator,
        _ => DbusMenuItemKind::Standard,
    };
    let label = property_string(&properties, "label")
        .map(|label| normalize_menu_label(&label))
        .unwrap_or_default();
    let children = children
        .into_iter()
        .filter_map(parse_dbus_menu_item)
        .collect();
    Some(DbusMenuItem {
        id,
        label,
        enabled: property_bool(&properties, "enabled", true),
        kind,
        children,
    })
}

fn property_string(properties: &DbusMenuProperties, key: &str) -> Option<String> {
    properties
        .get(key)
        .and_then(|value| <&str>::try_from(value).ok())
        .map(ToString::to_string)
}

fn property_bool(properties: &DbusMenuProperties, key: &str, default: bool) -> bool {
    properties
        .get(key)
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(default)
}

fn normalize_menu_label(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '_' {
            if chars.peek() == Some(&'_') {
                out.push('_');
                chars.next();
            }
            continue;
        }
        out.push(ch);
    }
    out.trim().to_string()
}

fn normalize_service(service: String) -> String {
    service.trim().to_string()
}

async fn resolve_item_details(connection: &Connection, service: &str) -> StatusNotifierItem {
    let mut item = StatusNotifierItem {
        service: service.to_string(),
        title: None,
        icon_name: None,
        menu_path: None,
    };
    let proxy = match Proxy::new(
        connection,
        service,
        "/StatusNotifierItem",
        "org.kde.StatusNotifierItem",
    )
    .await
    {
        Ok(proxy) => proxy,
        Err(e) => {
            debug!(service, error = %e, "status-notifier: item proxy unavailable");
            return item;
        }
    };

    item.title = read_string_property(&proxy, service, "Title").await;
    item.icon_name = read_string_property(&proxy, service, "IconName").await;
    item.menu_path = read_object_path_property(&proxy, service, "Menu").await;
    item
}

async fn read_string_property(proxy: &Proxy<'_>, service: &str, property: &str) -> Option<String> {
    match proxy.get_property::<String>(property).await {
        Ok(value) if !value.trim().is_empty() => Some(value),
        Ok(_) => None,
        Err(e) => {
            debug!(
                service,
                property,
                error = %e,
                "status-notifier: item property unavailable"
            );
            None
        }
    }
}

async fn read_object_path_property(
    proxy: &Proxy<'_>,
    service: &str,
    property: &str,
) -> Option<String> {
    match proxy.get_property::<OwnedObjectPath>(property).await {
        Ok(value) => Some(value.to_string()),
        Err(e) => {
            debug!(
                service,
                property,
                error = %e,
                "status-notifier: item object-path property unavailable"
            );
            None
        }
    }
}

fn snapshot_items(state: &WatcherState) -> Vec<StatusNotifierItem> {
    state.items.values().cloned().collect()
}

#[cfg(test)]
mod tests {
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
}
