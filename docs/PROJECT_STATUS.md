# Project Status

Stand: 2026-05-25, auditiert gegen `master` bei `2e7a2ed`.

Dieses Dokument ist der kompakte Ist-Stand. Aeltere Phasenlisten in anderen
Dokumenten koennen historischen Kontext enthalten; bei Widerspruch gilt hier
der Code-Stand plus `AGENTS.md`.

## Validierter Basisstand
- Git-Stand: `master...origin/master`, Arbeitsbaum vor dem Audit sauber.
- Dokumentationsaudit gegen `2e7a2ed`; anschliessend Shell-Idle-Slice im
  Arbeitsbaum umgesetzt.
- `cargo check --workspace --manifest-path /home/eduard/meridian-desktop/Cargo.toml`: gruen.
- `cargo test --workspace --manifest-path /home/eduard/meridian-desktop/Cargo.toml`: gruen.
- `cargo clippy --workspace --manifest-path /home/eduard/meridian-desktop/Cargo.toml -- -D warnings`: gruen.
- Live-DRM-Session: Release-`meridian-shell` installiert, Shell PID blieb
  stabil nach Popup-/Notification-Smokes; Idle danach 20-31 voluntary context
  switches pro 10s und 0 CPU-Ticks.
- Window-/Thumbnail-/Screenshot-Smoke fand einen Compositor-Zombie im
  Launch-Pfad; Fix im Arbeitsbaum reapt gestartete App-Prozesse per
  Reaper-Thread. Live-Bestaetigung nach Compositor-Neustart ist gruen:
  30 kurzlebige `/bin/true`-Launches hinterliessen keine Zombie-Children.

## Aktueller Ist-Stand

### Compositor
- DRM/KMS- und Winit-Backends sind aktiv; DRM bleibt der echte Session-Pfad,
  Winit der Entwicklungs-/Fallback-Pfad unter einem Parent-Display.
- XDG Shell, Layer Shell, XDG Decoration, SHM, Data Device/DnD, XWayland,
  dmabuf, syncobj, screencopy, session-lock, idle-inhibit und output-power
  sind im Compositor verdrahtet.
- Rendering bleibt in der verbindlichen Reihenfolge:
  wallpaper/background -> bottom layer -> normale Fenster -> top/panel ->
  overlay/launcher/popups -> cursor.
- NVIDIA/DRM-Diagnostik ist umfangreich vorhanden: Mode-Override,
  Timing-Aggregation, Commit-/VBlank-Metriken, Startup-Gates und reduzierte
  Hotpath-Logs.
- Shell-Launches werden im aktuellen Arbeitsbaum nach `spawn()` durch einen
  kleinen Reaper-Thread gewartet, damit gestartete Apps nach Exit nicht als
  Zombies am Compositor haengen bleiben.

### Shell
- `meridian-shell` ist ein eigener Layer-Shell-Client mit Panel, Launcher,
  App-Grid, Kategorieansicht, Kontextmenues, Kalender-, Workspace-, Netzwerk-,
  Thumbnail- und Notification-Popups.
- Der Launcher scannt `.desktop`-Dateien, filtert nach freedesktop-Regeln,
  startet Programme argv-basiert und unterstuetzt Kategorien, Suche, Hover,
  Favoriten/Pinned Apps und versteckte Apps.
- Settings-UI v1 hat jetzt ein Root-Korsett: `Desktop` enthaelt Theme,
  Cursor, Display, Wallpaper und Pinned Apps; `System` enthaelt ein
  bewusstes Untermenue-Skeleton fuer Overview, Network, Bluetooth, Sound,
  Printers, Power, Users und Updates. Aktiv sind Theme-Auswahl,
  Display-Status mit Primary-Output-Umschaltung, Wallpaper-Auswahl inkl.
  Thumbnails/Picker/Modus, Pinned-App-Verwaltung, Printers read-only v1
  und Sound read-only v1.
  Printers pollt CUPS ueber `lpstat`, zeigt Service-Status, Default-Drucker,
  konfigurierte Drucker und Queue-Zaehler. Sound pollt `wpctl status`, zeigt
  PipeWire/WirePlumber-Verfuegbarkeit, Default-Output/Input, Devices,
  Volume und Mute-Status. Das Panel hat einen Audio-Tray-Chip mit eigener
  Sound-Karte; der Panel-Klick oeffnet nicht mehr direkt Settings, die Karte
  enthaelt nur einen optionalen Link nach `System -> Sound`. Die restlichen
  System-Unterseiten sind Platzhalter und werden schrittweise mit Leben
  gefuellt.
- Power-Footer ist aktiv: Poweroff/Reboot/Suspend/Lock via Systemtools,
  Logout via `ShellCommand::Quit` an den Compositor. Power-Aktionen sind
  bewusst arming-basiert.
- Notifications: `org.freedesktop.Notifications` v1 laeuft im Shell-Prozess
  auf einem D-Bus-Thread; Notify/CloseNotification/GetCapabilities/
  GetServerInformation sind implementiert, die UI zeigt derzeit eine
  kompakte top-right Notification.
- StatusNotifierItem/System-Tray v1: `org.kde.StatusNotifierWatcher` laeuft
  im Shell-Prozess auf einem D-Bus-Thread, akzeptiert
  `RegisterStatusNotifierItem`/`RegisterStatusNotifierHost`, stellt die
  Watcher-Properties bereit, liest pro Item `Title` und `IconName` von
  `/StatusNotifierItem` und rendert registrierte Items als `panel-sni-N`
  Slots mit Icon- oder Label-Fallback im Panel. Panel-Linksklick,
  Mittelklick und Rechtsklick forwarden `Activate(x, y)`,
  `SecondaryActivate(x, y)` bzw. `ContextMenu(x, y)` mit globalen
  Panel-Koordinaten an das Item. Wenn die `Menu`-Property gesetzt ist, wird
  beim Rechtsklick zusaetzlich `com.canonical.dbusmenu.GetLayout` gegen den
  Menu-ObjectPath geprobt und Revision/Root/Child-Anzahl geloggt.
  DBusMenu-Rendering ist Folgearbeit.
- Screenshots: Panel-Screenshot nutzt clientseitig `ext-image-copy-capture`
  und schreibt PNGs in `~/Pictures/Screenshots`.
- Window-Thumbnails: Shell fordert Thumbnails ueber IPC an, Compositor rendert
  sie in den naechsten Frame-Pass und schickt sie als `WindowThumbnail`-Event.
- Shell-Idle-/Input-Redraw-Pfad wurde reduziert: der separate
  Commit-Stats-Timer ist entfernt, Tick-/Notification-Timer laufen im Idle
  langsamer, Network-Polling unterscheidet Popup-offen vs. idle,
  Panel/Launcher-Leave redrawen nur noch bei sichtbarer Zustandsaenderung,
  Workspace-Popup-Motion redrawt nur bei Hover-Zellenwechsel, und
  Network-Popup-Leave redrawt nicht mehr ohne sichtbaren Zustand.

### Boot und Login
- Boot-Kette ist aktiv: `bootsplash` -> `meridian-login` -> `meridian` ->
  `meridian-shell`, mit gemeinsamem Compass-Renderer.
- `meridian-login` uebernimmt DRM nach synchronem Bootsplash-Handover,
  rendert die Login-Karte, liest evdev-Keyboards/Pointers und greift
  Tastaturen waehrend der Eingabe.
- Login unterstuetzt zwei Pfade: registrierte YubiKeys schalten in den
  Smartcard/PIN-Modus (`/etc/Yubico/u2f_keys`, PAM-Service
  `meridian-login`), ohne bereiten Key faellt die UI auf
  Benutzername/Passwort zurueck (`meridian-login-password` mit `pam_unix`).
- YubiKey-Hotplug wird ueber USB Vendor `1050`, HID Vendor `00001050` und
  Yubico/YubiKey-Namensfallback erkannt. Die Live-Logs melden
  `security key state changed`.
- PAM laeuft in einem Worker, oeffnet via `pam_systemd` eine logind-Session
  und haelt den PAM-Handle bis zum Compositor-Ende offen.
- Der Compositor wird als authentifizierter User mit kompletter
  Supplementary-Group-Liste gestartet; `MERIDIAN_*`-Debug-Env wird
  weitergereicht.
- Login-Handover an den Compositor laeuft ueber `/run/meridian-login.sock`;
  `handover` gibt DRM-Master frei, `first-frame` markiert den sichtbaren
  Uebergang, danach wird auf den Compositor-Prozess gewartet.
- Login-Poweroff/Reboot sind direkt in der Login-UI mit zweitem Klick zur
  Bestaetigung vorhanden.

### Config
- `~/.config/meridian/config.toml` unterstuetzt Keybinds, Theme, Cursor,
  Wallpaper, Output-Layout und Panel-Pinned-Apps.
- Runtime-Reload ist aktiv: Theme, Cursor, Wallpaper, Keybinds und Output-
  Layout werden ueber `ReloadConfig` neu angewendet; Shell erhaelt
  `ConfigReloaded`.
- Shell-seitige Settings schreiben Pinned Apps in die Meridian-Config und
  versteckte Apps separat nach `~/.config/meridian/hidden_apps.txt`.

### Workspaces, Outputs und Hotplug
- OutputRegistry ist die zentrale Metadaten-Quelle fuer OutputId, Name,
  Geometry, Scale, Transform, Refresh und Primary.
- Focused-output Workspace-Modell ist aktiv: `focused_output` plus
  `active_workspace_by_output`, mit globalem `WorkspaceManager.active` als
  Kompatibilitaets-Shadow.
- `Super+1..9` wirkt auf den fokussierten Output; `Super+Shift+1..9`
  verschiebt das fokussierte Fenster ohne Auto-Switch.
- IPC sendet legacy Workspace-Events weiter und parallel output-aware
  Workspace-Snapshots/Changes; die Shell nutzt diese fuer die aktive
  Workspace-Markierung im Panel. `OutputWorkspaceSnapshot` enthaelt zusätzlich
  Output-Geometrie, Scale, Transform und Refresh, damit der Display-Settings-
  Reiter denselben OutputRegistry-Stand anzeigen kann. Die Shell kann den
  Primary-Output textuell in `config.toml` setzen und danach `ReloadConfig`
  ausloesen; Position/Scale/Mode bleiben unveraendert.
- Occupied-Status bleibt bewusst global aus `WindowSnapshot`.
- Hotplug-Pipeline H1-H5c ist implementiert: Registry-Update,
  Workspace-State-Sync/Fallback, Layer-Shell-Recovery, Snapshot-Broadcast,
  Winit-Reconfigure sowie DRM-Reconfigure/Remove/Add fuer Connectoren.
- H5d ist ein manueller E2E-Runbook-Status, kein weiterer Code-Slice.
  Runtime-Reconfigure/Remove/Add bleiben ohne neue dokumentierte
  Vollvalidierung pending.

### Portals
- `meridian-portal` ist ein eigenes D-Bus-Backend unter
  `org.freedesktop.impl.portal.desktop.meridian` am Pfad
  `/org/freedesktop/portal/desktop`.
- FileChooser ist implementiert: `OpenFile`, `SaveFile`, `SaveFiles`
  delegieren an `MERIDIAN_FILE_PICKER` bzw.
  `/usr/local/bin/meridian-file-picker`.
- Screenshot-Bridge-Typen existieren in `meridian-ipc`; der Compositor
  behandelt Bridge-Requests deny-only. Ein echtes Portal-Screenshot-Capture
  und ScreenCast sind noch offen.

## Offene Risiken
- Shell-Idle-Last ist verbessert, aber noch nicht abgeschlossen; naechster
  sinnvoller Fokus sind laengere Burn-in-Messungen und die Frage, ob weitere
  Popup-/Notification-Pfade Signaturen statt Voll-Redraws brauchen.
- Runtime-Hotplug braucht weiterhin einen dokumentierten realen E2E-Lauf.
- Login ist live mit Passwort-Fallback und YubiKey-Hotplug validiert; die
  Host-/VM-USB-Durchreichung bleibt eine externe Fehlerquelle.
- Portal-Screenshot/ScreenCast und Permission-Prompts sind noch nicht
  produktionsfaehig.
- Per-output Occupancy, pro-Output-Panel-Rollout und vollstaendige
  Multi-Monitor-Politur bleiben offen.
- Lock-Screen-Frontend ist noch offen, obwohl session-lock serverseitig
  vorhanden ist.

## Naechste sinnvolle Arbeiten
1. Runtime-Hotplug H5d auf echter DRM-Hardware erneut ausfuehren und
   Ergebnisse in `docs/MULTI_MONITOR.md`/`docs/NVIDIA_PASSTHROUGH.md`
   eintragen.
2. StatusNotifierItem-Tray weiter ausbauen: DBusMenu-Layout in ein lokales
   Menu-Modell parsen und als ContextMenu-Popup rendern.
3. Portal-Scope entscheiden: FileChooser haerten oder Screenshot-Permission-
   Pfad spezifizieren, nicht beides in einem Slice.
4. Login-Installationspfad dokumentieren: PAM-Dateien und Host-/VM-USB-
   Durchreichung fuer YubiKey stabil beschreiben.

## Manuelle Testhinweise

### Login
1. `sudo scripts/test-login-uinput.py --prepare-user --run --lock-user`
2. Logout-Smoke via IPC:
   `sudo scripts/test-login-uinput.py --prepare-user --run --logout-ipc --lock-user`
3. Logout-Smoke via UI:
   `sudo scripts/test-login-uinput.py --prepare-user --run --logout-ui --lock-user`

### Workspace-Switching
1. Meridian starten.
2. `Super+1` bis `Super+9` druecken.
3. Logs auf focused-output Switch pruefen.
4. Multi-Output-Ablauf siehe `docs/DEBUGGING.md`.

### Move-to-workspace
1. Fenster oeffnen.
2. `Super+Shift+2` druecken.
3. Pruefen, dass kein automatischer Wechsel stattfindet.
4. `Super+2` druecken und Fenster dort erwarten.

### Panel Active/Occupied
1. Workspaces wechseln und aktive Markierung pruefen.
2. Fenster mit `Super+Shift+N` verschieben.
3. Active-Markierung muss output-aware bleiben; Occupied bleibt global.

### Hotplug
1. Runbook `docs/DEBUGGING.md`, Abschnitt
   `Manueller Test: H5d DRM Hotplug E2E (Reconfigure/Remove/Add)`.
2. Ergebnisse danach in `docs/MULTI_MONITOR.md` und
   `docs/NVIDIA_PASSTHROUGH.md` nachtragen.
