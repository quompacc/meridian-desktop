use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use smithay_client_toolkit::reexports::calloop::channel as cchannel;
use tracing::{debug, error, info, warn};
use zbus::{connection::Builder, fdo, interface, Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

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
            return Err(fdo::Error::InvalidArgs("empty notifier service".to_string()));
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

pub fn spawn() -> std::io::Result<cchannel::Channel<DbusEvent>> {
    let (tx, rx) = cchannel::channel::<DbusEvent>();
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
                if let Err(e) = run(tx).await {
                    error!(
                        error = %e,
                        "status-notifier: dbus serve failed; watcher disabled until shell restarts"
                    );
                }
            });
        })?;
    Ok(rx)
}

pub(crate) fn activate_item(item: StatusNotifierItem, point: ActivationPoint) {
    forward_item_activation(item, point, ActivationKind::Activate);
}

pub(crate) fn secondary_activate_item(item: StatusNotifierItem, point: ActivationPoint) {
    forward_item_activation(item, point, ActivationKind::SecondaryActivate);
}

pub(crate) fn context_menu_item(item: StatusNotifierItem, point: ActivationPoint) {
    let service = item.service.clone();
    let menu_path = item.menu_path.clone();
    forward_item_activation(item, point, ActivationKind::ContextMenu);
    if let Some(menu_path) = menu_path {
        inspect_dbus_menu(service, menu_path);
    }
}

fn forward_item_activation(
    item: StatusNotifierItem,
    point: ActivationPoint,
    kind: ActivationKind,
) {
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

fn inspect_dbus_menu(service: String, menu_path: String) {
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
                Ok(summary) => {
                    info!(
                        service = %service,
                        menu_path = %menu_path,
                        revision = summary.revision,
                        root_id = summary.root_id,
                        children = summary.child_count,
                        "status-notifier: dbusmenu layout fetched"
                    );
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct DbusMenuLayoutSummary {
    revision: u32,
    root_id: i32,
    child_count: usize,
}

async fn fetch_dbus_menu_layout(
    service: &str,
    menu_path: &str,
) -> zbus::Result<DbusMenuLayoutSummary> {
    let connection = Connection::session().await?;
    let proxy = Proxy::new(&connection, service, menu_path, "com.canonical.dbusmenu").await?;
    let reply = proxy
        .call_method("GetLayout", &(0i32, 1i32, Vec::<&str>::new()))
        .await?;
    let body = reply.body();
    let (revision, root): (u32, DbusMenuLayoutNode) = body.deserialize()?;
    Ok(DbusMenuLayoutSummary {
        revision,
        root_id: root.0,
        child_count: root.2.len(),
    })
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
    use super::{normalize_service, snapshot_items, StatusNotifierItem, WatcherState};

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
}
