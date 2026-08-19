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
