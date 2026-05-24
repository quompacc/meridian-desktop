# Meridian Login Plan

## Ziel
Eigener Login Manager als Teil der Meridian-Suite. Pixel-kontinuierlicher Übergang vom Boot-Splash zum Compositor — ein durchgehendes visuelles Erlebnis vom POST bis zur Session, ohne VT-Switch, Schwarzphase oder externen DM (GDM/SDDM/greetd).

## Scope
- User-Authentifizierung (PAM)
- Session-Start für `meridian-compositor` als User (libseat / logind)
- Visueller Übergang Boot-Splash → Login → Compositor
- Eigene Lifecycle-Domain (eigener systemd-Service, ersetzt getty@tty1)

Nicht im ersten Scope:
- Lock-Screen (separate spätere Phase, kann denselben Renderer wiederverwenden)
- Session-Auswahl (mehrere DEs) — Meridian ist die einzige Session
- Multi-User Switch
- Remote-Login / autologin auf TTYs

## Visuelles Konzept

Der Boot-Splash ([QuompaCC](https://gitea.hl.home.arpa/eduard/bootsplash)) zeigt eine Kompassrose mit Nadel, die sich auf Norden einpendelt und dort dezent atmet ("breathing"). Der Übergang in den Login-Bildschirm darf keine Schwarzphase haben und keinen Bruch in der Bildsprache.

### Sequenz (konzeptuell)

1. **Boot-Splash steady state**
   Kompass ist eingependelt, Nadel zeigt Norden, der cyan Glow am Nordende ("Nordboble") atmet mit leichter Auf-/Ab-Bewegung.
2. **Handover-Trigger**
   `meridian-login.service` startet, sendet `handover` an `/run/bootsplash.sock`. Boot-Splash hört auf zu animieren, hält den letzten Frame, gibt DRM-Master ab und bleibt am Leben (Wait-State, hält FB).
3. **Master-Übernahme durch meridian-login**
   meridian-login wird DRM-Master, allokiert eigenen FB, rendert ein Frame **das visuell identisch zum letzten Boot-Splash-Frame ist** (gleicher Hintergrund-Gradient, gleiche Kompass-Geometrie, gleiche Nordboble-Position). Schickt `exit` an Boot-Splash → Boot-Splash terminiert, Kernel reaped seinen FB. Aus User-Sicht: kein sichtbarer Übergang.
4. **Animation "Stern fällt"**
   Die Nordboble löst sich von der Nadelspitze und fällt in einer parabolischen Bahn nach unten zur Bildschirmmitte. Während des Falls wächst sie. Easing: fall-physik (gravity-ähnlich, leichtes overshoot/bounce am Zielpunkt).
5. **Morph zur Login-Card**
   Am Zielpunkt morphed die gewachsene Boble in eine Login-Card: ovale/rechteckige Form, abgerundet, dunkelblau mit cyan Akzentkante. Innenausstattung erscheint: User-Avatar (Platzhalter oder Profilbild), Eingabefeld für Username, Feld für Passwort, dezenter "Anmelden"-Button.
6. **Hintergrund verblasst**
   Die restliche Kompassrose dimmt zu einer dezenten Wasserzeichen-Variante (alpha ~0.15-0.25). Bleibt sichtbar — visuelle Anker-Identität für die Marke. Die Nadel ohne Spitze bleibt erhalten und zeigt weiter nach Norden (jetzt offene Spitze, als hätte sie ihre Boble "abgegeben").

### Designprinzipien

- **Dezent**: keine Particle-Effekte, kein Glitter, kein Blur-Overload. Eine einzelne Bewegung mit klarer Bedeutung.
- **Kalligrafisch**: Beschriftungen in Italianno (wie Boot-Splash-Wortmarke); funktionale Labels in Sans (DejaVu Bold konsistent mit Kompass-Kardinalpunkten).
- **Dunkelblau-Grundton**: identisch zum Boot-Splash (Hintergrund-Radial-Gradient bleibt unverändert).
- **Physikalisches Gefühl**: parabolische Bahn, leichtes Overshoot beim Eintreffen, nicht mechanisch linear.

### Pixel-Kontinuität: technische Konsequenz

Für den nahtlosen Übergang in Schritt 3 müssen Boot-Splash und meridian-login dieselben Render-Konstanten und denselben Renderer benutzen. Konkret:
- Hintergrund-Radial-Gradient (Stops und Center) identisch
- Kompass-Geometrie identisch (Radius-Faktor `0.32`, Skalenring, 8-zackige Rose, Nadel-Proportionen)
- Cyan-Glow identisch (3 Layer mit `(0.18, 24), (0.12, 50), (0.08, 110)` Alpha)
- Boot-Splash-Freeze-Frame ist deterministisch (Nadel exakt auf 360° = Norden, ohne breathing-Offset oder mit definiertem Offset)

→ **Konsequenz für Architektur**: Gemeinsame Crate `meridian-compass-render` (oder Modul in `meridian-ui`) als Single Source of Truth für die Kompass-Geometrie. Boot-Splash und meridian-login linken die gleiche Crate. Alternativ: gemeinsames Asset-File (SVG/serialisierte Renderer-Description), gerendert in beiden Prozessen identisch.

## Empfohlene Architektur

### Workspace-Integration
- Neue Crate `crates/meridian-login` im bestehenden Workspace
- Eigenes Binary `meridian-login`
- Separate systemd-Unit `meridian-login.service`
- Läuft als root (PAM, DRM-Master, libseat)

### Renderer-Entscheidung
Zwei Optionen mit klaren Trade-offs:

**Option R1: Direkter `drm-rs` + `tiny-skia` (wie Boot-Splash)**
- Pro: Minimaler Footprint (~13 MB RAM wie Boot-Splash). Renderer ist 1:1 ableitbar/portierbar. Schnell startbar. Keine Smithay-Komplexität für eine reine Ausgabeschleife.
- Contra: Kein Wayland-Stack — wenn später Lock-Screen / IM-Kontext / Accessibility nötig wird, müsste man das nachziehen.

**Option R2: Eigener Smithay-Compositor (privat, minimal)**
- Pro: Konsistent mit `meridian-compositor`. Direkter Pfad zu IME (input method), Tastatur-Layout, A11y. Lock-Screen wäre später nur ein zusätzlicher Modus.
- Contra: Mehr Code, mehr Memory, langsamer Start. Smithay ist Overkill für eine Login-Maske.

**Empfehlung für Phase 1**: R1 (direkter `drm-rs` + `tiny-skia`). Render-Pfad ist als Modul im Boot-Splash-Code bereits erprobt. Wenn Lock-Screen / IME später dazukommt, kann auf R2 migriert werden (saubere Abstraktion vorausgesetzt).

### Komponenten
- DRM/KMS-Backend (Renderer-Decision oben)
- Auth-Provider: `pam`-Crate (oder direkte libpam-FFI)
- Session-Provider: `libseat` (analog zu meridian-compositor) — Session-Acquire, Seat-Switch, libpam handles credential-validation
- IPC-Client: bootsplash-Socket-Talker, später eigener Socket-Server für Übergabe zum Compositor

### IPC-Schnittstellen

**Eingehend (`/run/meridian-login.sock`)**
Selbe Befehle wie Boot-Splash:
- `handover` — Login fadet ggf. UI aus, gibt Master ab, hält FB im Wait-State
- `exit` — Wait-State beenden
- `status` — Lebensdiagnostik

**Ausgehend**
- `/run/bootsplash.sock`: `handover` beim eigenen Start, `exit` nach erstem committed Frame
  - Pfad per `BOOTSPLASH_SOCKET` überschreibbar
- ggf. an `meridian-compositor`: über dessen Lifecycle-Hook (separat zu spezifizieren)

### Lifecycle

```
[boot]
  └─ bootsplash.service (root)
       └─ rendert Kompass, hält DRM-Master, /run/bootsplash.sock listening

[handover-target erreicht]
  └─ meridian-login.service (root)
       ├─ open /run/bootsplash.sock, send "handover", wait ack
       ├─ acquire DRM master
       ├─ render first frame (visuell identisch zum bootsplash-Endframe)
       ├─ send "exit" to /run/bootsplash.sock → bootsplash terminiert
       ├─ play compass→input-card animation
       ├─ show input fields, accept keyboard
       └─ /run/meridian-login.sock listening (für späteren compositor-handover)

[user gibt creds ein]
  └─ PAM authentifiziert
       └─ libseat: session für target user
            └─ fork+exec meridian-compositor als user
                 ├─ open /run/meridian-login.sock, send "handover"
                 ├─ acquire DRM master
                 ├─ render first compositor frame
                 ├─ send "exit" to /run/meridian-login.sock
                 └─ meridian-login terminiert
```

### systemd-Integration
- `meridian-login.service`:
  - `After=systemd-udev-trigger.service local-fs.target`
  - `Before=getty.target multi-user.target`
  - `Conflicts=getty@tty1.service` (Login darf nicht mit getty kollidieren)
  - `Restart=on-failure` (mit Restart-Limit — wiederholtes Crashen darf nicht endlos rebooten)
- Boot-Splash muss `Before=meridian-login.service`
- `meridian-compositor` wird nicht systemd-managed; läuft als Child von `meridian-login` (oder als User-Service nach libseat-Session-Aktivierung)

## Sicherheitsgrenzen

- Läuft als root → Codebase muss klein und auditierbar sein
- PAM ist Source-of-Truth für Auth — keine eigene Passwort-Logik
- Passwörter:
  - Nie loggen (auch nicht auf trace-Level)
  - Im Speicher in `Vec<u8>` mit `zeroize`-On-Drop
  - Nach PAM-Übergabe sofort wegen Drop überschreiben
- Privilege-Drop:
  - `meridian-compositor` startet **nicht** als root, sondern als der authentifizierte User (libseat regelt Seat-Ownership, restliche FDs werden geschlossen)
  - meridian-login selbst gibt nach erfolgreicher Session-Übergabe Master ab und terminiert — kein Daemon-Lifetime
- IPC-Socket `/run/meridian-login.sock`:
  - Permissions 0660 root:root
  - Befehle nur lebensdiagnostisch oder handover-relevant — kein User-Daten-Pfad

## Implementierungsreihenfolge

### Phase 1: Crate-Skeleton + IPC
- `crates/meridian-login/` mit Cargo.toml, `src/main.rs`
- Hello-World-Binary, schreibt Log und exit(0)
- systemd-Unit als reine Doku in `docs/MERIDIAN_LOGIN.md` (kein Install)
- `cargo check --workspace` grün

### Phase 2: DRM-Master-Handover Boot-Splash → Login
- meridian-login öffnet die DRM-Card (`MERIDIAN_LOGIN_DRM_CARD`, Default `/dev/dri/card0`)
- compositor binary: `MERIDIAN_LOGIN_COMPOSITOR`, Default `/usr/local/bin/meridian`
- Sendet `handover` an Boot-Splash-Socket
- Wird DRM-Master
- Rendert deterministischen End-Frame des Boot-Splashs (statisch, gleiche Kompass-Geometrie)
- Sendet `exit`
- Hält Frame für 3s, exit
- Reboot-Test: Boot → Splash → ohne Riss in Login-Frame → exit → Schwarz

### Phase 3: Render-Sharing zwischen bootsplash und meridian-login
- Gemeinsame Render-Crate (`meridian-compass-render`) oder Modul-Extraktion
- Beide Binaries linken den gleichen Code
- Visuelle Parität verifiziert (Pixel-Diff zwischen Boot-Splash-Endframe und Login-Startframe = identisch)

### Phase 4: Stern-Fall-Animation
- "Nordboble löst sich, fällt parabolisch, wächst"
- Easing: `gravity * t² / 2` für Position, smooth-step für Größe
- Hintergrund-Kompass dimmt parallel
- End-Frame: leere Login-Card (kein Inhalt) im Zentrum

### Phase 5: Login-Card Inhalt
- Eingabefelder (Username, Passwort) erscheinen via Fade-in
- Keyboard-Input via libinput (oder via xkb wenn smithay)
- Visual feedback bei Tippen
- "Anmelden"-Button mit Enter-Hotkey
- Cursor / Caret in Italianno-Akzentfarbe

### Phase 6: PAM-Auth
- `pam`-Crate integration
- Konversation: Username → PAM → Password → PAM
- Bei Erfolg: PAM-Handle hält Session offen
- Bei Fehler: Card "schüttelt" kurz (klassisches Login-Feedback), Felder werden cleared

### Phase 7: Session-Start
- libseat: Seat-Acquire
- Setze Environment für User (`XDG_RUNTIME_DIR`, `XDG_SESSION_TYPE=wayland`, etc.)
- fork+exec `meridian-compositor` als User
- meridian-login öffnet `/run/meridian-login.sock` als Listener
- Wartet auf `handover` vom Compositor

### Phase 8: Handover-Acknowledge zu meridian-compositor
- meridian-compositor muss beim DRM-Init `handover` an meridian-login senden (separate Aufgabe in meridian-desktop)
- meridian-login gibt Master ab, hält FB, wartet auf `exit`
- meridian-compositor commited erstes Frame, sendet `exit`
- meridian-login terminiert

### Phase 9: Polish
- Fehlerzustände sauber visualisieren
- Locale / Tastaturlayout aus systemd-localed
- A11y-Grundlagen (Screen-Reader-Hook später)

### Phase 10 (separat): Lock-Screen-Modus
- Re-use des meridian-login-Renderers
- Wird vom Compositor getriggert (z.B. nach Idle-Timeout oder Hotkey)
- Eigenes Service-Pattern (z.B. `meridian-locker` als anderer Binary, gleiche Crate)

## Hauptrisiken

1. **Pixel-Kontinuität:** Wenn Render-Konstanten zwischen Boot-Splash und meridian-login auch nur minimal divergieren, ist ein Sprung im Frame-Übergang sichtbar. **Mitigation:** Shared-Render-Crate als harte Dependency-Beziehung. Diff-Test in CI (renderframe to PNG, hash check).
2. **DRM-Master-Race:** Wenn meridian-login Master nehmen will bevor Boot-Splash ihn dropt, scheitert acquire. **Mitigation:** IPC-Ack pattern statt Race — meridian-login wartet auf Boot-Splash-Ack des `handover`-Befehls, erst dann acquire.
3. **systemd-Ordering und getty:** Wenn getty@tty1 parallel startet, übernimmt fbcon den Bildschirm. **Mitigation:** `Conflicts=getty@tty1.service` in der Unit + Plymouth-style Boot-Pattern als Referenz.
4. **PAM ist alt und voller Fußangeln:** Fehlende Modul-Konfiguration, falsch behandelte Konversations-Items, Race mit logind. **Mitigation:** PAM-Config sauber dokumentieren (`/etc/pam.d/meridian-login`), nur Standard-Module verwenden (`pam_unix`, `pam_systemd`).
5. **libseat / Session-Übergabe:** Falsche FD-Übergabe lässt den Compositor ohne Tastatur/Maus stehen. **Mitigation:** systemd-logind ist Quasi-Standard und wird auch von smithay benutzt — meridian-login integriert sich dort statt eigener Session-Logik.
6. **Renderer-Choice falsch:** Wenn R1 gewählt wird und später IME/A11y nötig wird, ist Refactor groß. **Mitigation:** Renderer-Frontend von Auth-/Session-Backend sauber trennen, sodass R2-Migration nur Renderer-Modul betrifft.
7. **Passwort-Sicherheit im Speicher:** Standard-`String` lässt Passwort-Bytes im Heap bis GC/Free. **Mitigation:** `zeroize`-Crate; sicherer Eingabepuffer; nie als `String` typen.
8. **Animationen verstecken Auth-Latenz nicht:** PAM kann mehrere Sekunden brauchen (Kerberos, LDAP). **Mitigation:** Status-Spinner während Auth-Call, klare Fehlermeldungen bei Timeout.

## Kurz-Backlog für den ersten Code-Task (Phase 1)

1. `crates/meridian-login/Cargo.toml` mit minimalen deps (libc, tracing, anyhow). Workspace-Eintrag in Root-`Cargo.toml`.
2. `crates/meridian-login/src/main.rs` als Hello-World-Binary:
   - Tracing-Subscriber init
   - Log "meridian-login starting"
   - Log "meridian-login exiting"
   - Exit 0
3. `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings` grün
4. Bericht im Format aus `AGENTS.md`

**Bewusst nicht in Phase 1:**
- Keine DRM-Logik
- Keine PAM-Logik
- Keine systemd-Unit (Beschreibung in Doku reicht)
- Kein Renderer
- Keine IPC-Anbindung

## Verwandte Dokumente
- [Design Manifesto](design-manifesto.md) — Coherence-Prinzipien gelten auch hier
- [Technical Design Guidelines](technical-design-guidelines.md)
- Boot-Splash-Projekt (separates Repo): IPC-Protokoll, DRM-Master-Handover-Stub, systemd-Unit-Vorlage
