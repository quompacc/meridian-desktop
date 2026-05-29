# Meridian Login

Stand: 2026-05-25, auditiert gegen `crates/meridian-login`.

`meridian-login` ist kein Planungs-Scaffold mehr. Es ist der root-seitige
DRM-Login-Prozess zwischen `bootsplash` und dem User-Compositor.

## Scope
- DRM-Login-UI mit gemeinsamem Compass-Renderer.
- Synchronous Bootsplash-Handover.
- Evdev-Keyboard und Pointer fuer die Login-Maske.
- YubiKey/PIN-Authentifizierung mit Passwort-Fallback ueber PAM.
- Logind-Session via `pam_systemd`.
- Start des Compositors als authentifizierter User.
- IPC-Handover vom Login-Prozess zum Compositor.
- Poweroff/Reboot aus der Login-UI.

Nicht Scope:
- Remote-Login oder TTY-Autologin.
- Lock-Screen-Frontend.
- Vollstaendige Display-Manager-Funktionalitaet.

## Boot-Kette

```text
bootsplash -> meridian-login -> meridian -> meridian-shell
```

1. `bootsplash` haelt DRM-Master und zeigt den Compass-Frame.
2. `meridian-login` sendet `handover` an den Bootsplash-Socket.
3. Bootsplash bestaetigt erst, nachdem DRM-Master freigegeben wurde.
4. `meridian-login` oeffnet die DRM-Card, rendert die Login-UI und
   verarbeitet Eingaben.
5. Nach erfolgreicher Authentifizierung startet `meridian-login` den
   Compositor als User.
6. Der Compositor verbindet sich mit `/run/meridian-login.sock`, sendet
   `handover`, uebernimmt DRM und meldet optional `first-frame`.
7. `meridian-login` wartet auf das Ende des Compositors und schliesst danach
   die PAM/logind-Session.

## Relevante Pfade
- Binary: `crates/meridian-login/src/main.rs`
- PAM/Auth: `crates/meridian-login/src/auth.rs`
- User-Session/Compositor-Spawn: `crates/meridian-login/src/session.rs`
- Evdev/xkb Input: `crates/meridian-login/src/input.rs`
- PAM-Konfiguration: `/etc/pam.d/meridian-login` und
  `/etc/pam.d/meridian-login-password`
- Repo-Vorlage fuer Passwort-Fallback: `packaging/pam/meridian-login-password`
- Systemd-Unit: `/etc/systemd/system/meridian-login.service`
- Gemeinsame Boot-Helfer: `crates/meridian-boot-common`
- Gemeinsamer Renderer: `crates/meridian-compass-render`

## Konfiguration und Umgebungsvariablen
- `BOOTSPLASH_SOCKET`: ueberschreibt `/run/bootsplash.sock`.
- `MERIDIAN_LOGIN_DRM_CARD`: ueberschreibt `/dev/dri/card0`.
- `MERIDIAN_LOGIN_COMPOSITOR`: ueberschreibt `/usr/local/bin/meridian`.
- `RUST_LOG`: wird an den Compositor weitergereicht.
- Alle `MERIDIAN_*` Variablen werden an den Compositor weitergereicht,
  damit Debug-Knobs wie `MERIDIAN_DRM_TIMING` erhalten bleiben.
- YubiKey/U2F-Mapping: `/etc/Yubico/u2f_keys`.

## Input-Modell
- `open_keyboards()` scannt `/dev/input/event*` nach Keyboard-Keysets.
- Tastaturen werden nonblocking geoeffnet und fuer die Eingabe gegriffen.
- xkbcommon erzeugt UTF-8-Eingabe; Layout-Fallback ist `de`.
- Caps-Lock und Layout werden in der UI angezeigt.
- Keypad-Digits und Enter werden explizit behandelt.
- Pointer werden ebenfalls direkt ueber evdev verarbeitet; absolute und
  relative Bewegung werden auf die Login-UI projiziert.

## Authentifizierung
- Der Login-Prozess pollt YubiKey-Praesenz ueber USB Vendor ID `1050`.
- Wenn ein registrierter YubiKey bereit ist, nutzt die UI den Smartcard-Modus:
  User wird aus `/etc/Yubico/u2f_keys` abgeleitet, Eingabe ist die PIN,
  PAM-Service ist `meridian-login` mit `pam_u2f`.
- Wenn kein registrierter YubiKey bereit ist, schaltet die UI auf
  Benutzername+Passwort: Tab wechselt zwischen den Feldern, PAM-Service ist
  `meridian-login-password` mit `pam_unix`.
- Beim Wechsel aus dem Smartcard-Modus in den Passwort-Fallback wird der
  PIN/Passwort-Puffer geleert, damit eine PIN nicht als Passwort
  wiederverwendet wird.
- PAM laeuft in einem Worker-Thread, damit der Renderloop reaktionsfaehig
  bleibt.
- Passwoerter/PINs werden in `Zeroizing<String>` gehalten; die eigene
  PAM-Conversation-Kopie wird beim Drop geloescht.

## Session-Lifecycle
- `auth.rs` nutzt `pam-sys`, nicht die hoeherstufige `pam`-Crate, weil
  `pam_set_item(PAM_TTY, "tty1")` fuer `pam_systemd`/logind wichtig ist.
- `pam_open_session` erzeugt die logind-Session; `pam_getenvlist` wird
  ausgelesen und an den Compositor weitergereicht.
- Der PAM-Handle bleibt im Auth-Worker bis zum Ende des Compositor-Prozesses
  alive.
- Drop/Close des `AuthDriver` fuehrt `pam_close_session` und `pam_end` aus.

## Compositor-Start
- `session::launch_compositor_for` sucht den Unix-User, bereitet
  `/run/user/<uid>` vor und startet den Compositor mit sauberem
  Wayland-Desktop-Environment.
- Vor `exec` laufen `initgroups`, `setgid`, `setuid` in dieser Reihenfolge.
- Damit erbt der Compositor Gruppen wie `video`, `render` und `input`.
- Explizit gesetzte Env-Werte: `HOME`, `USER`, `LOGNAME`, `PATH`, `SHELL`,
  `XDG_RUNTIME_DIR`, `XDG_SESSION_TYPE=wayland`,
  `XDG_CURRENT_DESKTOP=Meridian`.

## Login-Handover-IPC
- Socket: `/run/meridian-login.sock`.
- Socket-Berechtigungen werden ueber `meridian-boot-common` abgesichert.
- Der Compositor darf den Login-Prozess mit folgenden Nachrichten steuern:
  - `handover`: Login gibt DRM-Master frei, haelt aber den FD noch offen.
  - `first-frame`: Compositor hat den ersten sichtbaren Frame geschafft.
- Ohne `handover` gibt es einen Timeout (`HANDOVER_DEADLINE`, 5s), damit
  ein fehlerhafter Compositor den Login-Frame nicht endlos einfriert.

## Power-Controls
- Login-UI enthaelt `Neustart` und `Ausschalten`.
- Beide Aktionen brauchen innerhalb eines kurzen Fensters einen zweiten Klick.
- Nach Bestaetigung wird die passende Systemaktion ausgefuehrt.

## Tests
- Unit-Tests liegen direkt in `src/main.rs`, `auth.rs`, `session.rs` und
  `input.rs`.
- Clippy ist Pflicht:
  `cargo clippy --workspace --all-targets -- -D warnings`
- Realtest:
  `sudo scripts/test-login-uinput.py --prepare-user --run --lock-user`
- Logout via IPC:
  `sudo scripts/test-login-uinput.py --prepare-user --run --logout-ipc --lock-user`
- Logout via UI:
  `sudo scripts/test-login-uinput.py --prepare-user --run --logout-ui --lock-user`

## Offene Produktentscheidungen
- Soll `/etc/Yubico/u2f_keys` konfigurierbar werden?
- Soll Login-Poweroff/Reboot ueber systemd/logind statt direkter Befehle
  normalisiert werden?
- Lock-Screen-UI ist separat zu entwerfen; der aktuelle Login-Prozess ist
  nicht automatisch der Lock-Screen.
