# XDG Portals Plan

## Ziel
Kleine, umsetzbare Basis für Portal-Support ohne sofortige Vollimplementierung.

## Scope (späterer Bedarf)
- FileChooser
- Screenshot
- ScreenCast
- Settings/Appearance
- OpenURI
- Backend Discovery / Desktop-Integration

## Empfohlene Architektur
- Eigenes Backend als separater Prozess (später z. B. `meridian-portal`), nicht im Render-/Input-Hotpath des Compositors.
- Kommunikation:
  - D-Bus: App <-> `xdg-desktop-portal` <-> `meridian-portal`
  - Interne Steuerung: `meridian-portal` <-> Meridian (kleiner, stabiler IPC-Pfad; kein direkter DBus-Code im Compositor-Core nötig).
- Datenfluss:
  - Screenshot/ScreenCast: Output-Geometrie, Auswahl/Target-Output, Pixelquelle/Framequelle.
  - Settings: read-only Theme/Appearance-Werte.
  - OpenURI/FileChooser: Delegation an Shell/Launcher oder externen Handler.

## Sicherheitsgrenzen
- Portal-Prozess ist Policy-Grenze:
  - Session-/App-Kontext und User-Prompts.
  - Whitelist der erlaubten Requests.
  - Keine unkontrollierte Weitergabe von Surface-/Output-Daten.
- Compositor liefert nur minimal notwendige Daten/Frames.
- Kein globaler „always allow“-Pfad als Default.

## Implementierungsreihenfolge
1. Screenshot Portal (kleinster vertikaler Slice):
   - Request entgegennehmen
   - Prompt/Policy
   - Einzelbild aus Compositor liefern
2. FileChooser Integration/Delegation:
   - zunächst delegiert (externer chooser oder Shell-Dialog-Stub)
   - saubere Antwort-/Cancel-Pfade
3. ScreenCast:
   - PipeWire-Anbindung
   - Session-Lifecycle, Revoke/Stop, Multi-Output

## Minimal Scaffold (bewusst nicht implementiert)
- `meridian-portal` ist als eigenes Workspace-Binary angelegt.
- Start:
  - `cargo run -p meridian-portal`
- Aktuelles Verhalten:
  - Health-Logs:
    - `meridian-portal starting`
    - `portal backend scaffold ready`
  - Bei fehlendem Session-Bus: sauberer Warn-Log, kontrolliertes Beenden.
  - D-Bus-Skeleton:
    - Service: `org.meridian.Portal1`
    - Object path: `/org/meridian/portal`
    - Interface: `org.meridian.portal.Screenshot1`
  - Screenshot-Stummel vorhanden (`handle_screenshot_request`) und liefert bewusst `Unsupported`.
  - Screenshot-Bridge-Contract (v0, deny-only) definiert:
    - `meridian_ipc::ScreenshotBridgeRequest`
      - `request_id` (Korrelations-ID)
      - `kind = full-output`
      - `output: Option<String>`
      - `include_cursor: bool`
      - `region: Option<ScreenshotRegion>` (aktuell validiert, aber bewusst `Unsupported`)
      - `metadata`:
        - `requester: Option<String>`
        - `origin: portal-dbus|internal|unknown`
        - `request_marker: Option<u64>`
        - `identity_trusted: bool`
    - `meridian_ipc::ScreenshotBridgeResponse`
    - `meridian_ipc::ScreenshotBridgeError`:
      - `Unsupported`
      - `PermissionDenied`
      - `CompositorUnavailable`
      - `InvalidRequest`
      - `Internal`
  - D-Bus-Error-Mapping im Portal:
    - `Unsupported` -> `NotSupported`
    - `PermissionDenied` -> `AccessDenied`
    - `InvalidRequest` -> `InvalidArgs`
    - `CompositorUnavailable`/`Internal` -> `Failed`
  - Deny-only Bridge-Transport aktiv:
    - `meridian-portal` sendet `ScreenshotBridgeMessage::ScreenshotRequest { request }` über den bestehenden Meridian-IPC-Socket.
    - Compositor beantwortet mit `ScreenshotBridgeMessage::ScreenshotResponse { request_id, result }`.
    - Valid requests werden aktuell durch Policy mit `PermissionDenied` beantwortet (kein Capture-Pfad).
    - Invalid requests werden mit `InvalidRequest` bzw. `Unsupported` beantwortet.
  - Policy-Layer aktiv:
    - Compositor wertet Requests über `ScreenshotPolicy::evaluate(...)` aus.
    - Standardentscheidung bleibt `Deny` für alle validen Requests.
    - `region` bleibt `Unsupported`.
    - ungültige Requests bleiben `Invalid`.
- Nicht implementiert:
  - keine echte Screenshot-Pipeline
  - keine FileChooser-/ScreenCast-Logik
  - keine Permission-Dialoge
  - keine echte `org.freedesktop.portal.*`-Kompatibilität (nur Meridian-internes Interface-Skeleton)

## Hauptrisiken
- D-Bus Lifecycle/Activation (Systemd user services, Names, Timeouts)
- PipeWire/xdg-desktop-portal Interop
- Permission Prompts und Persistenz
- Screenshot-/Capture-API im Compositor sauber kapseln
- Sandboxed Apps (Flatpak/Snap) und erwartete Portal-Semantik

## Bridge-Richtung und Policy
- Geplante Richtung bleibt: `D-Bus request -> portal policy -> compositor bridge -> response handle`.
- Quelle der Wahrheit für Policy ist `meridian-portal`, nicht der Compositor.
- Ohne explizite Permission werden keine Screenshots freigegeben.
- Es gibt keinen globalen Allow-Default-Pfad.
- Aktuelle Grenze:
  - `requester`-Identität wird noch nicht als vertrauenswürdig behandelt (`identity_trusted=false`).
  - Es wird keine sichere App-Identität vorgetäuscht.

## Kurz-Backlog für den ersten Code-Task
1. Permission-Pfad spezifizieren (keine globale Allow-Policy).
2. Meridian-Skeleton schrittweise auf `org.freedesktop.impl.portal.desktop.*` + `org.freedesktop.portal.*` ausrichten.
3. Erst danach Capture-API-Grenze (Screenshot) konkretisieren, weiterhin ohne PipeWire-/ScreenCast-Scope.
