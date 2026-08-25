# Meridian Web-UI prototype (design reference only)

This directory archives the final GTK4/WebKit6 prototype from August 2026.
It is not part of the Cargo workspace, is not installed and must not be loaded
by the Meridian shell. Meridian's production UI is native Rust.

The HTML, CSS, JavaScript and captured OpenBSD screenshots remain useful as a
visual contract for the native panel, launcher and Quick Settings work. Product
values still come from `meridian-tokens` and `meridian-config`; the generated
token sheet here is a frozen reference, not a second source of truth.

Do not add runtime features to this prototype. Transfer accepted visual ideas
into the native renderer and cover them with the normal design guard.
