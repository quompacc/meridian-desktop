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

include!("status_notifier/activation_and_menu.rs");

#[cfg(test)]
#[path = "status_notifier_tests.rs"]
mod tests;
