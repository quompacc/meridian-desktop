# XDG Portals

> **Priority note (2026-08-19):** existing portal behavior remains supported.
> Broad new portal/UI work is deferred behind the WebKit vertical slice unless
> required for its security boundary or basic daily-driver validation.

Stand: 2026-06-05, auditiert gegen `crates/meridian-portal`.

## Ziel

Portal-Support bleibt ein separater Prozess, damit D-Bus, App-Policy und
Prompts nicht in den Compositor-Render-/Input-Hotpath wandern.

## Aktueller Stand

- Binary: `meridian-portal`.
- D-Bus Name: `org.freedesktop.impl.portal.desktop.meridian`.
- Object Path: `/org/freedesktop/portal/desktop`.
- Implementiert sind die Impl-Portale `FileChooser`, `Screenshot` und `Access`.
- `PickColor` antwortet kontrolliert mit Response-Code `2`.
- `ScreenCast`, Settings/Appearance und OpenURI sind offen.
- Installationsmetadaten liegen unter `packaging/`:
  - D-Bus service file
  - systemd user unit
  - `.portal` descriptor
  - `meridian-portals.conf`

## FileChooser

Implementierte Methoden:

- `OpenFile`
- `SaveFile`
- `SaveFiles`
- Property `version = 3`

Der Backend-Prozess delegiert an einen externen Picker:

- `MERIDIAN_FILE_PICKER`, falls gesetzt
- sonst `/usr/local/bin/meridian-file-picker`

Weitergereichte Umgebung:

- `WAYLAND_DISPLAY`
- `DISPLAY`
- `XDG_RUNTIME_DIR`
- `GDK_BACKEND=wayland`

Rueckgaben:

- Erfolgreiches `OpenFile`: `uris` als `file://...`.
- Erfolgreiches `SaveFile`: `uri`.
- Erfolgreiches `SaveFiles`: `destination`.
- Cancel: Response-Code `1`.
- Picker-Fehler: Response-Code `2`.

## Screenshot

Implementierte Methoden:

- `Screenshot`
- `PickColor` als sauberer Fehlerpfad
- Property `version = 2`

Datenpfad:

1. `meridian-portal` nimmt den D-Bus-Request entgegen.
2. Der Portal-Prozess sendet `ScreenshotBridgeRequest` ueber den Meridian-IPC-
   Socket an den Compositor.
3. Die Compositor-Policy entscheidet:
   - `PortalDbus` + `interactive=false`: Shell-Consent-Modal.
   - `PortalDbus` + `interactive=true`: Shell-Region-Picker.
   - unbekannte oder untrusted Origins: deny-by-default.
   - `Internal` ist nur mit `MERIDIAN_SCREENSHOT_DEV=1` erlaubt.
4. Nach Consent/Region-Pick rendert der DRM-Pfad den Output, schreibt eine PNG
   in `XDG_RUNTIME_DIR` und antwortet mit einem File-URI.

Offene Validierung:

- echter `xdg-desktop-portal`-Client in einer installierten Meridian-Session
- Cancel-/Deny-/Allow-Pfade mit sichtbarem Shell-Modal
- Region-Picker auf echter DRM-Hardware und Multi-Output
- Winit-/Nicht-DRM-Verhalten fuer Portal-Screenshot, falls benoetigt

## Access

`org.freedesktop.impl.portal.Access` ist vorhanden und antwortet auto-allow,
weil Meridian die eigentliche Screenshot-Entscheidung im eigenen Shell-/Compositor-
Consent-Pfad trifft. Das verhindert, dass xdg-desktop-portal die Meridian-
Screenshot-Implementierung schon beim Backend-Scan verwirft.

## Architekturgrenzen

- Portal-Prozess ist die D-Bus- und App-Policy-Grenze.
- Compositor bleibt Frame-/State-Quelle, nicht D-Bus-Frontend.
- Screenshot-Capture braucht explizite Meridian-Policy; es gibt keinen globalen
  Allow-Default.
- FileChooser darf extern delegieren; ScreenCast braucht eine eigene PipeWire-
  Session- und Permission-Architektur.

## Offene Slices

1. FileChooser haerten:
   - echte `xdg-desktop-portal`-Clients testen
   - Filter/Current-folder/Modal-Optionen auswerten
   - Cancel-/Fehlerpfade dokumentieren
2. Screenshot produktionshaerten:
   - installierten Portal-Pfad gegen xdg-desktop-portal testen
   - Consent-/Region-Picker auf echter Hardware validieren
   - Multi-Output-Auswahl und Output-Aufloesung spezifizieren
3. Settings/Appearance read-only:
   - Theme/Accent/Color-Scheme aus Meridian-Config ableiten
4. ScreenCast:
   - PipeWire
   - Session-Lifecycle, Revoke/Stop
   - Multi-Output-Auswahl

## Risiken

- D-Bus Activation und Name-Konflikte.
- Abweichungen zwischen `org.freedesktop.impl.portal.*` und den Erwartungen von
  `xdg-desktop-portal`.
- Sichere App-Identitaet fuer Permission-Entscheidungen.
- PipeWire/ScreenCast-Lifecycle.
