//! Lazy icon delivery for the isolated launcher document.
//!
//! WebKit sees only opaque `meridian-icon:` identifiers. Rust resolves icon
//! theme files and caches encoded PNG bytes by name, theme and output scale.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use gtk::prelude::*;
use meridian_app_catalog::DesktopApp;
use meridian_tokens::Launcher;
use webkit2gtk::{URISchemeRequestExt, WebContext, WebContextExt};

const SCHEME: &str = "meridian-icon";
const EMPTY_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    name: String,
    theme: String,
    logical_size: i32,
    scale: i32,
}

struct IconService {
    names: Vec<Option<String>>,
    theme: gtk::IconTheme,
    theme_identity: String,
    scale: i32,
    cache: RefCell<HashMap<CacheKey, Option<Vec<u8>>>>,
}

pub(crate) fn install(context: &WebContext, apps: &[DesktopApp], scale: i32) {
    let Some(theme) = gtk::IconTheme::default() else {
        eprintln!("meridian-ui-runtime: no GTK icon theme available");
        return;
    };
    let theme_identity = gtk::Settings::default()
        .map(|settings| settings.property::<String>("gtk-icon-theme-name"))
        .unwrap_or_else(|| "unknown".to_string());
    let service = Rc::new(IconService {
        names: apps.iter().map(|app| app.icon_name.clone()).collect(),
        theme,
        theme_identity,
        scale: scale.max(1),
        cache: RefCell::new(HashMap::new()),
    });
    context.register_uri_scheme(SCHEME, move |request| service.respond(request));
}

pub(crate) fn uri(index: usize) -> String {
    format!("{SCHEME}://app/{index}")
}

impl IconService {
    fn respond(&self, request: &webkit2gtk::URISchemeRequest) {
        let icon = request
            .uri()
            .and_then(|uri| uri.rsplit('/').next()?.parse::<usize>().ok())
            .and_then(|index| self.names.get(index))
            .and_then(Option::as_deref)
            .and_then(|name| self.load_cached(name));

        let (bytes, content_type) = match icon {
            Some(bytes) => (gtk::glib::Bytes::from_owned(bytes), "image/png"),
            None => (gtk::glib::Bytes::from_static(EMPTY_SVG), "image/svg+xml"),
        };
        let stream = gtk::gio::MemoryInputStream::from_bytes(&bytes);
        request.finish(&stream, bytes.len() as i64, Some(content_type));
    }

    fn load_cached(&self, name: &str) -> Option<Vec<u8>> {
        let key = CacheKey {
            name: name.to_string(),
            theme: self.theme_identity.clone(),
            logical_size: Launcher::DEFAULT.app_icon_size,
            scale: self.scale,
        };
        if let Some(cached) = self.cache.borrow().get(&key) {
            return cached.clone();
        }
        let loaded = self.load(name).and_then(|pixbuf| {
            pixbuf
                .save_to_bufferv("png", &[])
                .map_err(|error| {
                    eprintln!("meridian-ui-runtime: failed to encode icon {name:?}: {error}")
                })
                .ok()
        });
        self.cache.borrow_mut().insert(key, loaded.clone());
        loaded
    }

    fn load(&self, name: &str) -> Option<gtk::gdk_pixbuf::Pixbuf> {
        let size = Launcher::DEFAULT.app_icon_size;
        if name.starts_with('/') {
            return gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(
                name,
                size * self.scale,
                size * self.scale,
                true,
            )
            .ok();
        }
        self.theme
            .load_icon_for_scale(name, size, self.scale, gtk::IconLookupFlags::FORCE_SIZE)
            .ok()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_uri_exposes_only_an_opaque_index() {
        assert_eq!(uri(7), "meridian-icon://app/7");
        assert!(!uri(7).contains("/usr/"));
    }
}
