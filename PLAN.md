# Meridian – Masterplan

> Hinweis (Stand 2026-05): Dieses Dokument ist ein historischer Masterplan und in Teilen veraltet. Für den aktuellen Ist-Stand bitte primär den aktuellen Commit-Verlauf sowie `README.md` (Build-/Workflow-Checks) heranziehen; für aktive Patch-Regeln gilt `AGENTS.md`.

## Backlog-Status (konsolidiert, Stand 2026-05)

- `P2.3` SplitDir-Duplikation: **resolved** (Config->WM-Mapping zentralisiert; keine geplante Type-Unification).
- `P2.4` Workspace-Index `u8` vs `usize`: **resolved** (IPC-Konvertierung zentralisiert; IPC-Schema bewusst unverändert).
- `P2.21` `clamped_w`/`clamped_h` Dead Code: **false positive** (Variablen werden für Tracing-Diagnostik genutzt).
- `P2.22` `unsafe` ohne `SAFETY`-Kommentare: **resolved** (durch Commit `53512f7`).
- `P2.5` DPI-Support Shell: **deferred** (größerer Designblock; Compositor aktuell effektiv scale-1-only).
- `P1.7` `drm_backend.take()/restore` RAII-Guard: **optional follow-up** (auditiert; kein akuter Non-Panic-Restore-Bug gefunden).

## Projektübersicht

Meridian ist eine vollständige Wayland-Desktop-Umgebung, geschrieben in Rust.
Benannt nach dem Kompass-Meridian. Ziel ist eine native, moderne Desktop-Umgebung
mit erstklassiger NVIDIA-Unterstützung.

**Stack:**
- Sprache: Rust (Stable)
- Compositor-Basis: Smithay (Git-Version)
- Protokoll: Wayland-nativ
- GPU-Target: NVIDIA RTX 4070 Super, Treiber >525, GBM
- Entwicklungsumgebung: VM mit Winit-Backend, später DRM/KMS

---

## Cargo Workspace Struktur

```
meridian/
├── Cargo.toml                  # Workspace Root
├── crates/
│   ├── meridian-compositor/    # Wayland Compositor Kern
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── state.rs        # MeridianState + alle Trait-Impls
│   │       ├── backend/
│   │       │   ├── mod.rs
│   │       │   ├── winit.rs    # VM/Entwicklung Backend
│   │       │   └── drm.rs      # Echte Hardware Backend (NVIDIA)
│   │       ├── input/
│   │       │   ├── mod.rs
│   │       │   ├── keyboard.rs
│   │       │   └── pointer.rs
│   │       └── protocols/
│   │           ├── mod.rs
│   │           ├── xdg_shell.rs
│   │           └── xwayland.rs
│   ├── meridian-wm/            # Fenstermanager
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── window.rs       # Fenster-Abstraktion
│   │       ├── floating.rs     # Floating Mode
│   │       ├── tiling.rs       # Tiling Mode (BSP)
│   │       └── workspace.rs    # Workspace-Verwaltung
│   ├── meridian-shell/         # Panel, Launcher, Widgets
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs         # Eigener Prozess (Wayland Client)
│   │       ├── panel.rs        # Taskbar, Uhr, Tray
│   │       └── launcher.rs     # App-Starter
│   ├── meridian-ipc/           # IPC Protokoll
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs          # Unix Socket IPC
│   └── meridian-config/        # Konfigurationssystem
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs          # TOML Config
└── src/
    └── main.rs                 # Einstiegspunkt
```

---

## Implementierungs-Reihenfolge

### Schritt 1 – Cargo Workspace aufsetzen
- Workspace `Cargo.toml` mit allen Crates
- Alle `Cargo.toml` der einzelnen Crates
- Smithay als Git-Dependency (aktuelle Version):
```toml
smithay = { 
    git = "https://github.com/Smithay/smithay.git", 
    features = [
        "backend_drm",
        "backend_gbm", 
        "backend_egl",
        "backend_winit",
        "renderer_gl",
        "wayland_frontend",
        "xwayland"
    ] 
}
```

### Schritt 2 – MeridianState (meridian-compositor/src/state.rs)
- `MeridianState` struct mit allen Smithay States
- `ClientState` struct mit `CompositorClientState`
- Alle Pflicht-Trait-Implementierungen:
  - `CompositorHandler`
  - `XdgShellHandler`
  - `ShmHandler`
  - `BufferHandler`
  - `SeatHandler`
  - `OutputHandler`
- Alle `delegate_*!()` Makros

### Schritt 3 – Winit Backend (meridian-compositor/src/backend/winit.rs)
- Compositor startet als Fenster im bestehenden Desktop
- Ideal für VM-Entwicklung ohne echte GPU
- Output initialisieren
- Event-Loop aufsetzen
- Ersten Frame rendern (schwarzer Screen = Erfolg)

### Schritt 4 – Ersten Output rendern
- Leerer schwarzer Screen ohne Abstürze
- Frame-Loop stabil
- `cargo run` in der VM muss ein Fenster öffnen

### Schritt 5 – XDG Shell (meridian-compositor/src/protocols/xdg_shell.rs)
- `new_toplevel` – neues Fenster registrieren
- `new_popup` – Popup-Fenster
- Fenster auf dem Screen darstellen
- Test: `weston-terminal` oder `foot` starten

### Schritt 6 – Input (meridian-compositor/src/input/)
- Libinput über Smithay
- Keyboard-Events an fokussiertes Fenster
- Pointer-Events (Maus bewegen, klicken)
- Seat-Verwaltung

### Schritt 7 – Floating Fenstermanager (meridian-wm/src/floating.rs)
- Fenster mit Maus verschieben (move grab)
- Fenster mit Maus skalieren (resize grab)
- Z-Order – Fenster nach vorne/hinten
- Maximieren / Fullscreen

### Schritt 8 – Workspaces (meridian-wm/src/workspace.rs)
- Mehrere Workspaces pro Monitor
- Wechseln per Tastenkürzel (Super+1 bis Super+9)
- Fenster verschieben (Super+Shift+1-9)
- Nur aktiver Workspace sichtbar

### Schritt 9 – DRM/KMS Backend (meridian-compositor/src/backend/drm.rs)
- Echte Hardware, kein Fenster mehr
- GBM Buffer Management
- EGL Context
- NVIDIA-kompatibel: Atomic KMS, GBM (kein EGLStreams)
- `linux-drm-syncobj-v1` für Explicit Sync (NVIDIA Pflicht)

### Schritt 10 – Multi-Monitor (meridian-compositor output/state paths)
- Mehrere DRM-Outputs gleichzeitig
- `xdg-output` Protokoll
- Hot-Plug Support
- Workspaces pro Monitor

### Schritt 11 – Tiling Mode (meridian-wm/src/tiling.rs)
- Binary Space Partitioning (BSP) Layout
- Automatische Aufteilung bei neuem Fenster
- Resize zwischen Tiles
- Umschalten Floating ↔ Tiling: `Super+T`

### Schritt 12 – Xwayland (meridian-compositor/src/protocols/xwayland.rs)
- X11 Kompatibilitätslayer
- Xwayland Prozess starten und verwalten
- X11-Fenster wie normale Wayland-Fenster behandeln

### Schritt 13 – Shell / Panel (meridian-shell/)
- Separater Prozess (eigener Wayland-Client)
- `wlr-layer-shell` Protokoll – bleibt immer oben
- Taskbar: offene Fenster anzeigen, klicken zum Fokussieren
- Workspace-Switcher
- Uhrzeit / Datum
- System-Tray (StatusNotifierItem)
- Launcher: `.desktop` Dateien, Suche, `Super+Space`

### Schritt 14 – XDG Portals
- `xdg-desktop-portal` Backend implementieren
- Datei-Dialog
- Screenshot
- Screen-Share (Pipewire)

### Schritt 15 – Konfiguration (meridian-config/)
- TOML-basierte Config (`~/.config/meridian/config.toml`)
- Tastenkürzel anpassbar
- Farben / Abstände
- Monitor-Layout
- Config-Reload ohne Neustart via IPC

### Schritt 16 – NVIDIA Passthrough Test
- Host: VFIO Passthrough aktiv (bereits vorbereitet)
- VM: NVIDIA Treiber installieren
- DRM/KMS Backend mit echter 4070 Super testen
- Explicit Sync validieren

---

## NVIDIA Kompatibilität – Pflichtregeln

Diese Punkte müssen von **Schritt 1 an** im Code berücksichtigt werden:

1. **Kein hardcodiertes X11** – alles nativ Wayland
2. **GBM als Buffer-API** – niemals EGLStreams
3. **Atomic KMS** – modernes DRM API (NVIDIA ab Treiber 525)
4. **Explicit Sync** – `linux-drm-syncobj-v1` Protokoll implementieren
5. **`nvidia_drm.modeset=1`** – Kernel-Parameter bereits gesetzt
6. **Kein `wl_drm`** – stattdessen `linux-dmabuf-v1`
7. **GBM Modifier Support** – für optimale Buffer-Allokation

---

## Technische Details

**Smithay Features die wir nutzen:**
- `backend_drm` – DRM/KMS für echte Hardware
- `backend_gbm` – GBM Buffer Management
- `backend_egl` – EGL Context (NVIDIA kompatibel)
- `backend_winit` – VM/Entwicklung
- `renderer_gl` – OpenGL Renderer
- `wayland_frontend` – Wayland Server
- `xwayland` – X11 Kompatibilität

**Wayland Protokolle:**
- `wl_compositor` – Basis
- `wl_shm` – Shared Memory
- `xdg_shell` – Fenster
- `wl_seat` – Input
- `wl_output` / `xdg_output` – Monitore
- `wlr_layer_shell` – Panel/Shell
- `linux_dmabuf` – GPU Buffer
- `linux_drm_syncobj` – Explicit Sync (NVIDIA)
- `xdg_decoration` – Fenster-Dekorationen

**IPC:**
- Unix Socket: `/run/user/{uid}/meridian.sock`
- JSON-basiertes Protokoll
- Kommandos: focus, move, resize, workspace, reload-config

---

## Erster Auftrag an Claude Code

```
Setze die Cargo Workspace Struktur für das Projekt "Meridian" auf.
Befolge dabei exakt die Verzeichnisstruktur aus dem Masterplan.
Erstelle alle Cargo.toml Dateien und leere Rust-Quelldateien (mod.rs, lib.rs etc.).
Smithay wird als Git-Dependency eingebunden.
Das Projekt soll danach mit `cargo build` ohne Fehler kompilieren (Warnungen sind ok).
```
