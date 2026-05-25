# XDG Portals

Stand: 2026-05-25, auditiert gegen `crates/meridian-portal`.

## Ziel
Portal-Support bleibt ein separater Prozess, damit D-Bus, App-Policy und
Prompts nicht in den Compositor-Render-/Input-Hotpath wandern.

## Aktueller Stand
- Binary: `meridian-portal`.
- D-Bus Name: `org.freedesktop.impl.portal.desktop.meridian`.
- Object Path: `/org/freedesktop/portal/desktop`.
- Implementiert ist derzeit das Impl-Portal `FileChooser`.
- Screenshot-Bridge-Typen existieren in `meridian-ipc`; der Compositor
  beantwortet Bridge-Requests deny-only. `meridian-portal` bietet aktuell
  keine echte Screenshot-Portal-Implementierung an.
- ScreenCast, Settings/Appearance und OpenURI sind offen.

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

## Screenshot-Bridge
Die gemeinsamen Typen liegen in `meridian-ipc`:
- `ScreenshotBridgeRequest`
- `ScreenshotBridgeResponse`
- `ScreenshotBridgeError`
- `ScreenshotBridgeResult`
- `ScreenshotBridgeMessage`

Compositor-Verhalten:
- Request-Validierung ist aktiv.
- Region-Capture bleibt `Unsupported`.
- Valide Requests werden per Policy aktuell `PermissionDenied`.
- Ungueltige Requests werden `InvalidRequest`.

Das ist bewusst noch kein produktiver Screenshot-Portal-Pfad.

## Architekturgrenzen
- Portal-Prozess ist die Policy-Grenze.
- Compositor bleibt Frame-/State-Quelle, nicht D-Bus-Policy-Ort.
- Ohne explizite Permission darf kein Screenshot oder Screencast
  freigegeben werden.
- FileChooser darf extern delegieren; Screenshot/ScreenCast brauchen vor
  Capture eine eigene Permission-/Prompt-Entscheidung.

## Offene Slices
1. FileChooser haerten:
   - Desktop-Dateien/Systemd-Activation.
   - Cancel-/Fehlerpfade gegen echte `xdg-desktop-portal`-Clients testen.
   - Filter/Current-folder/Modal-Optionen auswerten.
2. Screenshot spezifizieren:
   - Permission-Prompt.
   - Ziel-Output/Region.
   - Dateipfad oder FD-Transport.
   - Kein globaler Allow-Default.
3. Settings/Appearance read-only:
   - Theme/Accent/Color-Scheme aus Meridian-Config ableiten.
4. ScreenCast:
   - PipeWire.
   - Session-Lifecycle, Revoke/Stop.
   - Multi-Output-Auswahl.

## Risiken
- D-Bus Activation und Name-Konflikte.
- Abweichungen zwischen `org.freedesktop.impl.portal.*` und den Erwartungen
  von `xdg-desktop-portal`.
- Sichere App-Identitaet fuer Permission-Entscheidungen.
- PipeWire/ScreenCast-Lifecycle.
