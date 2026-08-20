# Project Status

Stand: 2026-08-19. Dokumentationsabgleich auf Branch `freebsd-port` bei
`664b5ba`; nach `git fetch`/`pull --ff-only` ist der Branch `3 ahead / 0 behind`
gegenüber `origin/freebsd-port`. Nicht eingecheckte Nutzerdateien sind darin
nicht enthalten.

Dieses Dokument beschreibt den **implementierten Ist-Stand**. Die aktive
Zielrichtung steht in `../MERIDIAN_OS_PLAN.md`, `../ROADMAP.md` und
`UI_PLATFORM.md`. Zielarchitektur ist nicht automatisch implementierter Stand.

## Strategische Einordnung (neu, noch nicht implementiert)

- Meridian bleibt ein eigener Rust-Wayland-Compositor.
- OpenBSD wird als nächste reale BSD-Referenz auf dem Acer/Intel-HD-620 geprüft;
  FreeBSD bleibt die ernsthafte Alternative und der vorhandene Supportpfad.
- Meridian-eigene Alltags-UI soll schrittweise auf eine gemeinsame
  WebKit/HTML/CSS/Web-Components-Plattform wechseln.
- Der erste Slice ist Runtime/Bridge → Panel → Launcher → Quick Settings.
- Der unten dokumentierte native `meridian-shell` bleibt bis zum bewiesenen
  Slice Referenz und Fallback.
- Externe GTK-/Qt-/Browser-/wxWidgets-Apps bleiben Wayland/XWayland-Clients.

### OpenBSD-Acer-Baseline (2026-08-19)

- Acer Aspire F5-573G mit OpenBSD 7.9 wurde read-only über SSH inventarisiert.
- Intel HD 620 läuft als `inteldrm0` mit 1920×1080 und verfügbaren DRM-/Render-
  Nodes; Ethernet, Audio, Touchpad und Webcam werden erkannt.
- Interne QCA9377-WLAN- und Bluetooth-Funktionen haben keinen angebundenen
  Treiber. Ethernet ist aktuell der einzige Netzwerkpfad.
- Systempatches `001` bis `009` sind eingespielt. Rust/Cargo 1.94.1,
  Wayland/libinput/xkbcommon/seatd, XWayland und WebKitGTK 4.1 sind installiert;
  `seatd` und D-Bus laufen und starten beim Boot.
- WebKitGTK 4.1 kompiliert und linkt nativ (2.52.5); ein gerenderter
  Wayland/WebKit-Frame ist noch nicht bewiesen.
- `meridian-tokens`, `meridian-ipc`, `meridian-config`, `meridian-portal`,
  `meridian-boot-common`, `meridian-compass-render`, `meridian-freetype` und
  `meridian-ui` bauen auf OpenBSD. Der Design-Guard ist gruen.
- Die erste harte Portierungsgrenze ist geloest: Der gepinnte Smithay-Port
  kompiliert `linux-drm-syncobj-v1` auf OpenBSD nicht, behaelt aber DRM/KMS,
  GBM und EGL. DRM-Nodes werden nativ unter `/dev/dri` gefunden; Eingaben kommen
  direkt aus `wskbd`/`wsmouse`, ohne Smithays udev/libinput-Backends. `meridian-wm`
  und `meridian-compositor` bauen nativ; alle 381 Compositor-Library-Tests sind
  auf OpenBSD gruen. Ein kontrollierter Hardwarelauf hat EGL/GBM, den nativen
  wscons-Open und den ersten atomaren 1920x1080-KMS-Commit bewiesen. OpenBSD
  libdrms `priv_open_device` wird dabei eng ueber die bestehende seatd-Session
  vermittelt; der normale Benutzer `eduard` erhaelt dadurch Intel-HD-620-
  Beschleunigung mit 99 DMA-BUF-Formaten, ohne Root-Compositor oder gelockerte
  Geraeterechte. XWayland wurde ebenfalls bereit.
- Der Shell-Screencopy-Pfad baut ebenfalls nativ: OpenBSD verwendet
  `shm_mkstemp(3)` plus `FD_CLOEXEC`, waehrend bestehende Targets bei
  `memfd_create` bleiben. Alle 310 Shell-Tests sowie der Centralization-Guard
  sind auf OpenBSD gruen. Ein voller Smoke startete die Shell, authentifizierte
  IPC und konfigurierte Panel sowie Launcher; ohne gestarteten D-Bus-Session-Bus
  deaktivieren sich Notifications und Status-Notifier derzeit kontrolliert.
- Login und Lock nutzen auf OpenBSD nativ `auth_userokay(3)`; PAM bleibt fuer
  andere Targets unveraendert erhalten. Polkit nutzt weiterhin seinen nativen
  setuid-Helper und hat keine unbenutzte direkte PAM-Abhaengigkeit mehr. Der
  gesamte Workspace besteht `cargo check`; 52 Login-, 9 Lock- und 11 Polkit-
  Tests sind gruen. Ein echter erfolgreicher Passwort-Login/Unlock bleibt als
  interaktiver Test offen. Der OpenBSD-Sessionstart nutzt ein besitzgeprueftes
  `/tmp/meridian-runtime-<uid>` und findet XWayland ueber `/usr/X11R6/bin`.
- Vollständige Evidenz und offene Tests: `OPENBSD.md`.

## Validierter Basisstand
- Remote-Stand: `origin/freebsd-port` bei `24177fe`; lokal zusätzlich
  `e0116ea`, `751cab8` und `664b5ba`.
- Audit-Reports: `docs/AUDIT_2026-08-19.md` (Bug-Audit gegen HEAD `874381e`
  inkl. unverpflichtetem Resize-/Glass-WIP; priorisierte Bug-Liste P1-P3),
  `docs/AUDIT_2026-06-20.md` (GLM, gegen Code geprueft: bis auf
  eine falsche Pfadangabe in §3.6 korrekt), `docs/AUDIT_2026-05-25.md` (aelter).
- `cargo test --workspace`: gruen (zuletzt auf der Arch-Box).
- `cargo clippy --workspace -- -D warnings`: gruen.
- `cargo test -p meridian-tokens --test design_guard`: gruen (0 Findings,
  erzwingt zentrale Design-Quelle).
- Die bisherige vollstaendige Live-Testbox/Workflow-Dokumentation bezieht sich
  auf Arch Linux. Der OpenBSD-Entwicklungs- und Teil-Buildpfad ist eingerichtet,
  aber eine Meridian-Grafiksession ist noch nicht validiert. FreeBSD bleibt
  separat dokumentiert.
- Theme-Assets liegen im Repo unter `themes/` und werden via
  `scripts/install-local.sh` nach `/usr/local/share/meridian/themes` installiert.

## Design-Zentralisierung (verbindlich)
- `docs/meridian_design_manifest.md` ist die massgebliche, **verbindliche**
  Design-Spezifikation (in `AGENTS.md` + `CLAUDE.md` als feste Regel verankert).
  Bei Konflikt schlaegt das Manifest jede andere Quelle.
- Eine zentrale Design-Quelle: `meridian-tokens` + `meridian-config`. Kein
  hartverdrahteter Farb-/Alpha-/Radius-/Mix-Wert im Render-Code ausserhalb davon
  (Ausnahmen nur via `// guard:allow: <grund>` bzw. `guard:allow-file`).
- Erzwungen durch `design_guard` (meridian-tokens): scannt `crates/*/src` auf
  hartverdrahtete Farb-/Alpha-/lerp-/ALPHA-Const-Werte; 112 -> 0 Findings.
  Definition of Done: `docs/GUI_CENTRALIZATION_PLAN.md` §9.
- Genau 2 Themes (hell/dunkel), identisch bis auf Farben. Branding (Kompass) nur
  subtil in Login/Bootsplash, nie in Alltags-UI; der Panel-Startbutton ist ein
  reduziertes, getheemtes Meridian-Symbol, nicht das alte Kompass-Badge.

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
- Tastatur-Layout wird aus der System-Konfiguration gelesen
  (`/etc/vconsole.conf` `XKBLAYOUT`) statt hart auf `us` zu defaulten; greift
  beim `seat.add_keyboard`.

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
  bewusst arming-basiert. Fehlende Power-Icons in Papirus/Papirus-Dark fallen
  ueber den IconLoader auf Breeze zurueck.
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
  Menu-ObjectPath gelesen, in ein lokales Menumodell normalisiert, an den
  Shell-Eventloop zurueckgegeben und als erstes Popup auf der geteilten
  Network/Audio-Layer-Surface gerendert. Linksklick auf einen aktivierten
  Menueintrag sendet `com.canonical.dbusmenu.Event(id, "clicked", ...)`.
  Submenu-/Scroll-Politur bleibt Folgearbeit.
- Themes: Builtin-Theme-Dateien liegen im Repo unter `themes/`. `ThemeManager`
  scannt User-Themes, `MERIDIAN_THEME_DIR(S)`, XDG-Datenpfade,
  `/usr/local/share/meridian/themes`, `/usr/share/meridian/themes` und den
  Dev-Repo-Pfad. Runtime-Themewechsel baut den Shell-IconCache neu auf,
  aktualisiert `available_themes` und markiert Panel/Launcher/Popups dirty.
- Icons: symbolische Icons werden ueber einen Alpha-Mask-Recolour
  (`icons/svg.rs`) auf die Theme-Textfarbe gefaerbt - icon-set-unabhaengig
  (der alte per-Farbstring-Hack matchte nur Breeze, Papirus-Icons trafen
  vorher nur zufaellig). Beide Themes nutzen Papirus (liefert `-symbolic`).
  Der Loader nutzt das konfigurierte Icon-Theme, faellt danach auf Breeze und
  `hicolor` zurueck und hat Aliase fuer XTerm/UXTerm auf `utilities-terminal`.
- Glass: Panel und Launcher-Hauptseite uebermalen die Compositor-Glasflaeche
  nicht mehr mit einem opaken Body; die Transluzenz ist jetzt konsistent mit
  den Popups (`Decorations.glass_divider_alpha` zentral im Theme).
- Power-Management: Tray zeigt einen Akku-Chip (Icon + %) aus
  `/sys/class/power_supply` (`battery.rs`). Klick schaltet das Power-Profil
  zyklisch Eco/Standard/Volle Leistung via `powerprofilesctl` (`power_profile.rs`)
  mit OSD-Einblendung; das Akku-Icon wird pro Profil eingefaerbt (Eco gruen,
  Standard neutral, Performance amber) als sichtbares Feedback.
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
- Wallpaper ist eine reine User-Einstellung und wurde aus `theme.toml`
  entfernt; ein Theme-Wechsel aendert das Wallpaper daher nicht mehr.
- Runtime-Reload ist aktiv: Theme, Cursor, Wallpaper, Keybinds und Output-
  Layout werden ueber `ReloadConfig` neu angewendet; Shell erhaelt
  `ConfigReloaded`. Theme-Reload umfasst inzwischen auch IconCache-Rebuild und
  Theme-Liste.
- Shell-seitige Settings schreiben Pinned Apps in die Meridian-Config und
  versteckte Apps separat nach `~/.config/meridian/hidden_apps.txt`.
- Wallpaper-Browse nutzt zuerst `/usr/local/bin/meridian-file-picker` und erst
  als Fallback `zenity`, weil `zenity` in der aktuellen Meridian-Session sofort
  mit Status 1 beendet.

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
- Screenshot ist im Portal-Prozess exponiert und laeuft ueber Meridian-IPC,
  Compositor-Policy, Shell-Consent bzw. Region-Picker und DRM-PNG-Capture.
  Der installierte xdg-desktop-portal-E2E-Pfad muss noch real validiert werden;
  ScreenCast bleibt offen.
- Settings (`org.freedesktop.impl.portal.Settings`) ist implementiert und
  exponiert `org.freedesktop.appearance` -> `color-scheme` aus dem aktiven
  Meridian-Theme (dunkel=1, hell=2). Damit folgen GTK4/libadwaita, Firefox und
  (via `QT_QPA_PLATFORMTHEME=xdgdesktopportal`) Qt/KDE-Apps dem Theme.
  Ein Watcher-Task pollt das color-scheme und feuert `SettingChanged`, sodass
  bereits laufende Apps live dunkel<->hell umschalten (kein Neustart noetig).
  Voraussetzung im Betrieb (auf der Box verdrahtet): die Session laeuft auf dem
  systemd `--user`-Bus statt einer privaten `dbus-run-session` (FreeBSD-
  Fallback bleibt), und `graphical-session.target` wird beim Login ueber
  `meridian-session.target` hochgezogen, damit die Portal-Services starten.
  Stolperfalle: eine veraltete `.portal`-Kopie unter `/usr/local/share` kann die
  aktualisierte beschatten (xdg-desktop-portal nimmt die erste pro Quelle).

## Offene Risiken
- Die bekannten OpenBSD-Compile-Grenzen fuer Shared Memory und Authentifizierung
  sind geloest. Offen sind interaktive Login-/Unlock-, Input-, D-Bus-Session-,
  Screenshot- und laengere Runtime-/Performance-Smokes. Eine beschleunigte
  Meridian-Session mit Shell ist startbar, aber noch kein validierter
  Daily-Driver.
- Breiter Bug-Audit ist erledigt: `docs/AUDIT_2026-08-19.md` katalogisiert die
  Befunde priorisiert (P1: Multi-Monitor-Screenshot erfasst falschen Output,
  ReloadConfig/Theme-Wechsel friert die Shell-Eventloop ein (LAUNCH-2-
  Regression), OpenBSD-Audio-Backend fehlt, Lock-Spawn ohne Reaping/Exit-Log;
  P2: blockierende nmcli/wpctl/mixer-Aufrufe ohne Timeout, drei Panic-Stellen
  in Hotplug/XDG-Popup/Resize-Grab; P3: Kosmetik/Hygiene). Erledigt davon
  (2026-08-20, auf OpenBSD verifiziert): P3-5 (Arbeitsbaum committet),
  P1-2 (ReloadConfig nutzt wieder den Off-Thread-Rescan), P1-1 (lokaler
  Screenshot erfasst den Primary-Output des Region-Pickers), P2-1
  (`process::output_with_timeout` fuer nmcli/wpctl/mixer, 2-s-Frist mit Kill),
  P1-4 (Lock-Spawn-Reaping + Exit-Log) und die drei `expect`→Fallback-Stellen
  P2-2/P2-3/P2-4 (Hotplug-Resolver, XDG-Popup-Initialconfigure, Resize-Grab).
  Offen: P1-3 (OpenBSD-Audio), P2-5, Rest-P3; Lauf-Repros (Theme-Wechsel,
  Multi-Monitor) auf der Arch-Box stehen weiter aus.
- Live-Theme-Switch funktioniert via Portal-`SettingChanged`; der Watcher pollt
  (ca. 2s Latenz). Auf inotify wurde bewusst verzichtet. Polling-Intervall und
  ob es auf einen ereignisbasierten Pfad umgestellt werden soll, sind offen.
- Shell-Idle-Last ist verbessert, aber noch nicht abgeschlossen; naechster
  sinnvoller Fokus sind laengere Burn-in-Messungen und die Frage, ob weitere
  Popup-/Notification-Pfade Signaturen statt Voll-Redraws brauchen.
- Theme-Assets sind im Repo und via `scripts/install-local.sh` nach
  `/usr/local/share/meridian/themes` installierbar; distro-native Pakete fuer
  Arch/Debian/Fedora bleiben offen.
- Runtime-Hotplug braucht weiterhin einen dokumentierten realen E2E-Lauf.
- Login ist live mit Passwort-Fallback und YubiKey-Hotplug validiert; die
  Host-/VM-USB-Durchreichung bleibt eine externe Fehlerquelle.
- Portal-Screenshot hat einen Consent-/Region-Pfad, ist aber noch nicht als
  installierter xdg-desktop-portal-E2E-Lauf validiert; ScreenCast fehlt.
- Per-output Occupancy, pro-Output-Panel-Rollout und vollstaendige
  Multi-Monitor-Politur bleiben offen.
- `meridian-lock` ist als Session-Lock-Frontend vorhanden und wird via
  `scripts/install-local.sh` installiert; installierter E2E-Lauf, Idle-Trigger
  und Multi-Output-Politur bleiben offen.

## Naechste sinnvolle Arbeiten
1. Erfolgreichen Login, Sessionstart, Lock/Unlock und D-Bus-Session interaktiv
   auf dem Acer testen, ohne Passwoerter in Logs oder Automatisierung zu geben.
2. Tastatur, Touchpad, Cursor, Screenshot und XWayland in einer kontrollierten
   OpenBSD-Sitzung interaktiv pruefen.
3. Smithay-OpenBSD-Grenze upstream vorbereiten; `linux-drm-syncobj-v1` darf auf
   OpenBSD nicht beworben werden.
4. Bug-Audit (`docs/AUDIT_2026-08-19.md`) wird abgearbeitet. Erledigt
   (2026-08-20): P3-5 (Arbeitsbaum: Asset-Rename, Audit-Doku und Resize-/
   Glass-WIP committet), P1-2 (ReloadConfig-Eventloop-Freeze: zurueck auf den
   bestehenden Off-Thread-Rescan), P1-1 (Multi-Monitor-Screenshot: Capture
   laeuft auf dem Primary-Output des Region-Pickers), P2-1
   (`process::output_with_timeout`, 2-s-Frist + Kill fuer nmcli/wpctl/mixer).
   Verifiziert mit `cargo check --workspace` und `cargo test --workspace
   --exclude smithay` auf der OpenBSD-Box (alles gruen). Ebenfalls erledigt:
   P1-4 (Lock-Spawn wird gereapt, Exit-Status wird geloggt) und die drei
   `expect`→Fallback-Stellen P2-2 (Hotplug-Resolver), P2-3 (XDG-Popup-
   Initialconfigure), P2-4 (Resize-Grab ohne wl_surface) — erneut auf OpenBSD
   verifiziert (391 Compositor-, 318 Shell-, 52 Login-Tests gruen). Als
   Naechstes: P1-3 (OpenBSD-Audio-Backend), danach P2-5 und Rest-P3.
   Lauf-Repros (Theme-Wechsel, Multi-Monitor-Screenshot) auf der Arch-Box
   stehen weiter aus.
5. Runtime-Hotplug H5d auf echter DRM-Hardware erneut ausfuehren und
   Ergebnisse in `docs/MULTI_MONITOR.md`/`docs/NVIDIA_PASSTHROUGH.md`
   eintragen.
6. Theme-/Asset-Packaging definieren: installierbare Theme-Ziele,
   Dependency-Liste und Cross-Distro-Pfade dokumentieren.
7. StatusNotifierItem-Tray weiter ausbauen: DBusMenu-Submenus, Scrollen,
   Hover-State und sauberere Positionierung pro Tray-Icon polieren.
8. Portal-Scope entscheiden: FileChooser haerten oder Screenshot-Permission-
   Pfad spezifizieren, nicht beides in einem Slice.
9. Login-Installationspfad dokumentieren: PAM-Dateien und Host-/VM-USB-
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
