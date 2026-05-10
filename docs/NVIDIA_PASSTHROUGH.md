# NVIDIA Passthrough Test Plan

## Ziel
Meridian auf einer per VFIO durchgereichten NVIDIA RTX 4070 Super in der Entwicklungs-VM mit DRM/GBM validieren.

## Scope
- Nur manueller Hardware-Testplan.
- Keine automatische Host-/VM-Konfigänderung.
- Keine Implementierung.

## 1) Host-Voraussetzungen
- Host-Desktop läuft auf Intel iGPU.
- NVIDIA RTX 4070 Super ist an `vfio-pci` gebunden.
- IOMMU ist aktiv.
- Geräte-IDs:
  - GPU: `10de:2783`
  - Audio: `10de:22bc`
- GPU + Audio liegen in eigener IOMMU-Gruppe.

Empfohlene Host-Checks:
- `lspci -nnk | rg -n "10de:2783|10de:22bc|Kernel driver in use"`
- `find /sys/kernel/iommu_groups -type l | rg -n "10de:2783|10de:22bc"`
- `dmesg | rg -n "DMAR|IOMMU|vfio"`

## 2) VM-Passthrough-Checkliste
1. GPU-PCI-Device und Audio-Function an die VM anhängen.
2. OVMF/UEFI aktiv prüfen.
3. Machine Type prüfen (Q35 empfohlen).
4. PCIe Root Port konfigurieren, falls Topologie es verlangt.
5. Falls nötig:
   - `multifunction=on`
   - ROM-Bar/ROM-Datei nur bei konkretem Bedarf.
6. Virtio-GPU:
   - optional als Fallback belassen oder deaktivieren (bewusst entscheiden).
7. SSH-Zugriff sicherstellen (unabhängig von lokaler Anzeige).
8. Snapshot der VM vor Test erstellen.

## 3) Guest-Checks
1. PCI sichtbar:
   - `lspci -nn | rg -n "10de:2783|10de:22bc|NVIDIA"`
2. DRM-Nodes vorhanden:
   - `ls -l /dev/dri`
   - Erwartung: mindestens `card*` und `renderD*`.
3. Treiberpfad dokumentieren:
   - Pfad A: NVIDIA-Treiber (proprietary) für Zielpfad.
   - Pfad B: Nouveau als Vergleichspfad (nur falls bewusst gewünscht).
4. GBM-Verfügbarkeit prüfen:
   - Treiber/Userspace-Kombination muss GBM-fähig sein.
5. Sitzungszugriff prüfen:
   - `seatd` oder `logind` Zugriff.
6. Nutzerrechte:
   - `eduard` in passenden Gruppen (`video`, ggf. `render`, distroabhängig).

## 4) Meridian-Testablauf
1. Build:
   - `cargo build --release`
2. Start:
   - `RUST_LOG=debug cargo run --release`
3. Prüfen:
   - DRM-Backend gewählt
   - GBM-Device geöffnet
   - EGL/GBM Extensions erkannt
   - `Frame rendered`-Logs
   - Layer-Map funktioniert
   - Panel sichtbar
   - Cursor sichtbar
   - Workspace-Switching (`Super+1..9`)
   - Launcher (`Super+Space`)
   - Hotplug (falls im Setup auslösbar)

### Reproduzierbarer Smoke-Test (DRM/NVIDIA)
Aus Repo-Root:

```bash
MERIDIAN_SMOKE_TIMEOUT=20 \
MERIDIAN_SMOKE_LOG=/tmp/meridian-smoke-drm.log \
scripts/smoke-drm.sh
```

Manueller Lauf für UX-/Launcher-Tests (ohne Timeout):

```bash
scripts/smoke-drm.sh run
# oder:
MERIDIAN_SMOKE_MODE=run scripts/smoke-drm.sh
```

Der Smoke-Test setzt:
- `MERIDIAN_DRM_TIMING=1`
- `MERIDIAN_DIRTY_STATS=1`
- `MERIDIAN_SHELL_RENDER_STATS=1`
- `RUST_LOG=info`

und wertet danach u. a. folgende Muster aus:
- `GL Vendor`
- `GL Renderer`
- `drm api selected`
- `drm mode selected`
- `drm timing summary`
- `dirty reasons`
- `shell render summary`
- `too slow` / `lagging` / `error` / `warn`

Erwartete Erfolgsindikatoren (nach Setup):
- DRM-API: `atomic`
- Timing stabil bei etwa `16-17ms` vblank wait (60Hz-Pfad)
- Shell-Idle sauber:
  - `frames=0` in steady-state
  - `outputs_skipped_clean` hoch
  - `commit_ms=0`
  - `dirty reasons=<none>`
- Clock-Update erzeugt nur einen einmaligen Frame und kehrt dann in clean idle zurück.
- Für manuelle UX-Läufe bleibt der Compositor im `run`-Modus aktiv, bis er per `Ctrl+C` oder `pkill` beendet wird.

## 5) Fehlerdiagnose (Kurzpfad)
- Kein `/dev/dri`:
  - PCI passthrough/guest-driver/sitzungsrechte prüfen.
- `Permission denied`:
  - Gruppen/seatd/logind prüfen.
- `atomic commit failed`:
  - Mode/Connector-Status, Treiberzustand, Kernel-Logs prüfen.
- GBM/EGL init failed:
  - Treiberpfad und GBM/EGL-Support prüfen.
- Black screen:
  - SSH nutzen, Logs prüfen, auf Fallback-Display wechseln.
- No input:
  - Eingabegeräte/seat-Konfiguration prüfen.
- NVIDIA modifier/explicit-sync Probleme:
  - Treiber-/Mesa-/Kernel-Kombination dokumentieren, ggf. Vergleich mit alternativer Stack-Version.

## 6) Recovery / Sicherheit
- SSH muss vor Test funktionieren.
- VM-Snapshot vor jedem tiefen Test.
- Host bleibt auf iGPU (Host darf NVIDIA nicht übernehmen).
- NVIDIA muss an `vfio-pci` gebunden bleiben.

Rollback-Schritte:
1. VM stoppen.
2. Letzten VM-Snapshot wiederherstellen.
3. VM ohne NVIDIA-Passthrough starten.
4. Falls nötig Host-Bindings prüfen, dann neu testen.

## 7) Ergebnisprotokoll (ausfüllen)
| Check | Status (pass/fail/pending) | Notizen |
|---|---|---|
| PCI passthrough (GPU+Audio) |  |  |
| DRM device (`/dev/dri`) |  |  |
| GBM/EGL init |  |  |
| Compositor start |  |  |
| Panel sichtbar |  |  |
| Input stabil |  |  |
| Workspace switching |  |  |
| Launcher |  |  |
| Hotplug (reconfigure/remove/add) |  |  |

Ergänzende Metadaten:
- Guest Kernel:
- NVIDIA-/Mesa-Version:
- VM-Config (Q35/OVMF, relevante PCI-Optionen):
- Beobachtete Warnungen/Fehler:

## Aktuelles Ergebnis (reale VM)
Status:
- NVIDIA passthrough: **pass**
- NVIDIA DRM/GBM/EGL render: **pass**
- Runtime hotplug add/remove/reconfigure: **pending**

Bestätigter Lauf (sauberer Treiberpfad):
- `GL Vendor: NVIDIA Corporation`
- `GL Renderer: NVIDIA GeForce RTX 4070 SUPER/PCIe/SSE2`
- `drm api selected: path=atomic`
- Timing (steady-state, Richtwerte):
  - `render_ms ~0.6–1.2 ms`
  - `commit_ms ~0.2–0.4 ms`
  - `vblank_wait_ms ~16–17 ms`
- Visuell: Maus flüssig, Bild korrekt, Panel sichtbar.

Beobachtungen:
1. PCI sichtbar:
   - `07:00.0 NVIDIA RTX 4070 SUPER [10de:2783]`
   - `08:00.0 NVIDIA Audio [10de:22bc]`
2. DRM-Zuordnung:
   - `card0 = NVIDIA` (vendor `0x10de`, device `0x2783`)
   - `card1 = virtio-gpu`
3. Connector:
   - `card0-HDMI-A-1 connected`
4. Meridian:
   - `frame rendered` sichtbar (Debug-Log) auf `3440x1440@60Hz`
   - Panel layer surface `3440x36` sichtbar/gemappt
   - Layer map `surfaces=2`
   - Pointer input auf `drm-0` funktioniert

Hinweis:
- Warnung beobachtet: `Smithay atomic restore previous state failed` mit `EINVAL`
- Kein Crash beobachtet.

Bestätigter Root Cause (vor Fixlauf):
- VM lief vorher zeitweise auf falschem GL-/Treiberpfad (Mesa llvmpipe) bzw. inkonsistentem nouveau/nvidia-Zustand.
- `nvidia`/`nvidia_drm` waren initial nicht konsistent verfügbar/geladen.
- Nach Reboot + konsistentem NVIDIA-Kernelmodulzustand war der atomare DRM-Pfad stabil und performant.

## NVIDIA Input Smoke-Test (vor Runtime-Hotplug)

### Ziel
Input-Probleme zwischen Meridian/libinput und USB-/KVM-/Hub-Pfad sauber trennen.

### 1) Geräteerkennung nach Monitor-Port-Umschaltung
Direkt nach Umschaltung auf die Meridian-VM:
- `lsusb`
- `sudo libinput list-devices`
- `journalctl -k -f`

Erwartung:
- Maus und Tastatur sind im Kernel sichtbar.
- Maus und Tastatur sind in libinput sichtbar.

### 2) Live-Event-Test
- `sudo libinput debug-events`
- Maus bewegen/klicken.
- Tastaturtasten drücken (inkl. Modifier).

Interpretation:
- Keine Tastatur-Events in libinput:
  - Kein Meridian-Keybinding-Problem.
  - USB-/KVM-/Hub-Pfad priorisieren.
- Tastatur-Events in libinput sichtbar, Meridian reagiert nicht:
  - Meridian-Inputpfad separat prüfen.

### 3) Vergleichstest ohne Monitor-Hub/KVM
- Tastatur direkt per USB an VM/Host-Passthrough testen.
- Maus direkt per USB testen.
- Ergebnis mit Monitor-Hub/KVM-Betrieb vergleichen.

### 4) Statusfelder (pro Lauf)
- Mouse input: `pass|partial|fail`
- Keyboard input: `pass|partial|fail`
- libinput keyboard events visible: `yes|no`
- monitor hub suspected: `yes|no`
- direct USB test: `pass|fail|pending`

### 5) Aktueller Zwischenstand
- Maus: `partial pass` (funktioniert, aber hackelig).
- Tastatur: `pending/fail` im Meridian-Test.
- Monitor-Hub/KVM: `yes` als wahrscheinlicher Risikofaktor.
- Empfehlung: keine Codeänderung bis libinput-/USB-Vergleichsdaten vorliegen.

### Befund Relative vs. Absolute Pointer
- QEMU Tablet (absolute motion / `POINTER_MOTION_ABSOLUTE`): funktioniert.
- USB-Maus (relative motion / `POINTER_MOTION`): war zuvor nicht im Meridian-Dispatch angebunden.
- Status nach Fix: relativer Motion-Pfad ist jetzt angebunden; manueller Re-Test auf NVIDIA-VM weiterhin erforderlich.

## DRM Stutter-Messung (NVIDIA VM)

### Ziel
Render-/Event-Loop-Langsamkeit gegen reinen Input-Dispatch trennen.

### Läufe
1. Baseline:
   - `RUST_LOG=warn cargo run`
2. Timing-Aggregation:
   - `RUST_LOG=info MERIDIAN_DRM_TIMING=1 cargo run`
3. Kurzdiagnose:
   - `RUST_LOG=debug MERIDIAN_DRM_TIMING=1 cargo run`
4. Forcierte Cadence (nur Scheduler-Override, keine Renderlogik):
   - `RUST_LOG=info MERIDIAN_DRM_TIMING=1 MERIDIAN_DRM_FORCE_REFRESH_HZ=60 cargo run`
   - oder: `RUST_LOG=info MERIDIAN_DRM_TIMING=1 MERIDIAN_DRM_FRAME_INTERVAL_MS=16 cargo run`
   - Hinweis: `MERIDIAN_DRM_FORCE_REFRESH_HZ` und `MERIDIAN_DRM_FRAME_INTERVAL_MS` beeinflussen nur den Repaint-Scheduler (Timer), nicht die KMS/Display-Mode-Auswahl.
5. Optionaler Mode-Override (nur Diagnose):
   - `RUST_LOG=info MERIDIAN_DRM_TIMING=1 MERIDIAN_DRM_MODE=1920x1080 cargo run`
   - `RUST_LOG=info MERIDIAN_DRM_TIMING=1 MERIDIAN_DRM_MODE=2560x1440 cargo run`
   - oder: `RUST_LOG=info MERIDIAN_DRM_TIMING=1 MERIDIAN_DRM_MODE_INDEX=0 cargo run`
   - Legacy-Alias bleibt: `MERIDIAN_DRM_FORCE_MODE=...`
   - Die KMS/Display-Mode-Auswahl wird über `MERIDIAN_DRM_MODE`, `MERIDIAN_DRM_FORCE_MODE` oder `MERIDIAN_DRM_MODE_INDEX` beeinflusst.
6. Optionaler Shell-Isolationstest:
   - `RUST_LOG=info MERIDIAN_DRM_TIMING=1 MERIDIAN_DRM_DISABLE_SHELL=1 cargo run`
   - Alias: `MERIDIAN_NO_SHELL=1`
   - Erwartung im Idle: überwiegend leere Ticks, `commit_ms` nahe 0.

### Erwartete Timing-Zeile
- `drm timing summary: ... interval_ms(avg/min/max)=... render_ms(... ) commit_ms(... ) queue_ms(... ) vblank_wait_ms(... ) ...`
- Erweiterte Diagnosefelder:
  - `timer_fire_ms`, `timer_lag_ms`, `tick_ms`
  - `output_pass_ms`, `vblank_handler_ms`, `frame_submitted_ms`
  - `queued_pending`, `queue_failures`

### Vergleich Vorher/Nachher
- Vorher (bekannt): per-frame `info`-Renderlogs + beobachtetes ~90ms Intervall.
- Nachher: per-frame Renderlog nur noch `debug`; `info` zeigt nur aggregierte 1s-Summary.
- Prüfen:
  - `libinput ... lagging behind` Häufigkeit
  - `interval_ms(avg)` Richtung ~16-17ms
  - sichtbare Maus-Hakler (pass|partial|fail)
  - `timer_fire_ms(avg)` vs. Override-Intervall (z. B. 16ms)
  - ob `commit_ms` weiterhin dominant bleibt

### Aktuelle Diagnose (NVIDIA VM)
- Beobachtet: `timer_fire_ms ≈ 90ms`, `tick_ms ≈ 90ms`, `render_ms ≈ 73ms`, `commit_ms ≈ 62ms`, `vblank_wait_ms ≈ 90ms`, `queue_ms < 1ms`, `frame_submitted_ms ≈ 0ms`.
- Interpretation:
  - Event-Loop wird effektiv auf ~11Hz gestreckt.
  - Engpass ist nicht `queue_frame` und nicht `frame_submitted`-Handler.
  - `drm api selected: path=legacy` und `Unable to become drm master` korrelieren mit blockierendem Commit-Pfad.
  - Nächster Fokus: Commit-/Pageflip-Blockierung im Legacy-Pfad bzw. DRM-Master-/Session-Constraints.

## DRM Master Diagnose/Failsafe (H6a)
- Startup loggt jetzt explizit:
  - `XDG_SESSION_ID`, `XDG_SEAT`, `XDG_VTNR`, `XDG_SESSION_TYPE`
  - `LIBSEAT_BACKEND` (oder `auto`)
  - gewählten DRM-Node inkl. `primary_node=true|false`
  - session-opened FD-Pfad
- `acquire_master_lock` ist Diagnose, nicht alleiniger Gate:
  - Erfolg: `drm master acquired: ...`
  - Fehler: `diagnostic drm master lock check failed ... functional KMS gate decides startup success`
- Funktionaler Gate:
  - KMS-Surface-Erzeugung muss gelingen (`drm kms surface created ...`)
  - erster echter KMS-Commit muss gelingen:
    - bei Erfolg trotz Lock-Fehler: `diagnostic drm master lock check failed earlier, but functional KMS gate succeeded (first commit ok); continuing`
    - bei Fehler: fataler Abbruch mit vollem Kontext
- Winit-Fallback ist für diesen DRM-Startup-Fehlerpfad deaktiviert.
- `acquire_master_lock` bleibt Diagnose; funktionaler KMS-Commit ist der reale Start-Gate.

### EPERM-Hinweis (H6b Untersuchung)
- In der lokalen Rust-DRM-API (`drm` crate) ist `acquire_master_lock()` als privilegierter ioctl dokumentiert
  (CAP_SYS_ADMIN-Kontext).
- Praktisch kann das in nicht-root Sessions trotz aktivem `seat0/tty1` als `EPERM` enden.
- Meridian wertet daher den funktionalen KMS-Gate höher:
  - `EPERM` allein ist kein finaler Abbruchgrund;
  - fehlender erster KMS-Commit bleibt fatal.
