use gtk::prelude::*;

pub(super) fn open(parent: &gtk::ApplicationWindow) {
    let dialog = gtk::FileChooserNative::new(
        Some("Hintergrund auswählen"),
        Some(parent),
        gtk::FileChooserAction::Open,
        Some("Auswählen"),
        Some("Abbrechen"),
    );
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Bilder"));
    for mime_type in ["image/jpeg", "image/png", "image/webp"] {
        filter.add_mime_type(mime_type);
    }
    dialog.set_filter(&filter);
    dialog.connect_response(|dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(path) = dialog
                .filename()
                .and_then(|path| path.to_str().map(str::to_string))
            {
                if let Err(error) = crate::ipc::set_appearance_wallpaper(path) {
                    eprintln!("meridian-ui-runtime: failed to set selected wallpaper: {error}");
                }
            }
        }
        dialog.destroy();
    });
    dialog.show();
}
