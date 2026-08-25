# Meridian – Regeln für Claude Code

## ⛔ UNIX-CODE NICHT AUF DER WINDOWS-DEV-MASCHINE VALIDIEREN
Die lokale Dev-Maschine ist **Windows**; die Wayland-Crates
(`meridian-compositor`, `meridian-shell`, …) kompilieren dort nicht vollständig
(`std::os::unix`). Builds und Gates laufen auf einem dokumentierten Unix-Ziel.
Bis der OpenBSD-Acer eingerichtet ist, bleibt die **Arch-Box**
(`ssh meridian-arch`) der bekannte Build-/Regression-Pfad:

1. **Sync:** `tar czf - --exclude=./target --exclude=./.git --exclude='*/target' . | ssh meridian-arch 'tar xzf - -C /root/meridian-build'`
   (Build-Dir ist `/root/meridian-build`, NICHT `/root/meridian-desktop`).
2. **Bauen** (detached, WLAN flapt mid-build):
   `cd /root/meridian-build && setsid nohup cargo build --release --workspace > /root/build_release.log 2>&1 &` — dann Log auf `Finished` pollen.
3. **Gate:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
   `cargo test -p meridian-tokens --test design_guard` — alles auf der Box.
4. **Installieren für den Test:** `install -m755 target/release/{meridian,meridian-shell} /usr/local/bin/`, dann `systemctl reboot` (Compositor-Änderung braucht Session-Neustart).
5. **Screenshot:** `grim` als eduard (siehe `docs/SSD_FRAME_PLAN.md` für die genaue `sudo -u eduard …`-Umgebung).

Nur `design_guard` (reiner Text-Scan in `meridian-tokens`) läuft notfalls lokal.
OpenBSD-/FreeBSD-Ergebnisse stets mit OS/Hardware kennzeichnen; ein VM- oder
Linux-Pass ist kein BSD-Hardware-Pass.

## Aktive Produktstrategie (2026-08-25)

- Meridian bleibt eigener Rust-Wayland-Compositor.
- OpenBSD wird auf Acer/Intel HD 620 evaluiert; FreeBSD bleibt Alternative.
- Meridian-eigene Alltags-UI bleibt vollständig nativ in Rust. Der archivierte
  WebKit-Prototyp dient ausschließlich als visuelle Designreferenz.
- Reihenfolge: Hardware-Inventar -> OpenBSD-Baseline -> Meridian-Core ->
  natives Panel -> nativer Launcher -> native Quick Settings.
- Keine großen neuen Features vor dieser nativen Qualitätsrunde.
- Privilegierte Aktionen bleiben in kleinen Rust-Services/Helpern; die Shell
  bleibt unprivilegiert.

## ⛔ ABSOLUTES TABU — KEINE AUR-PAKETE. NIEMALS.
**NUR offizielle Repos** (Arch `core`/`extra`, Distro-Repos). **DAS AUR IST
VERBOTEN** — es ist ungeprüft und derzeit mit über 1600 kompromittierten
Paketen verseucht; jede AUR-Installation ist ein Sicherheitsrisiko. Wenn etwas
nur im AUR existiert: **NICHT installieren** — eine offizielle Alternative
wählen oder nachfragen. Niemals `yay`/`paru`/`makepkg` o. Ä. ausführen.

## Pflichtregeln bei JEDER Änderung:
1. Für jede neue Funktion mindestens einen Unit-Test
2. cargo test --workspace muss grün bleiben
3. Keine Heap-Allokation im Render-Loop
4. Kein Clone() von Theme-Daten im Render-Loop
5. Keine externen Abhängigkeiten für Kernfunktionen
6. Alles was zum Desktop gehört wird eingebettet
7. KEINE AUR-Pakete (siehe Tabu oben) — nur offizielle Repos

## Design – VERBINDLICH (bei jeder UI-/Render-Änderung):
- `docs/meridian_design_manifest.md` ist die maßgebliche Design-Spezifikation.
  Bei Konflikt schlägt das Manifest jede andere Quelle.
- Eine zentrale Design-Quelle: `meridian-tokens` + `meridian-config`. KEIN
  hartverdrahteter Farb-/Alpha-/Radius-/Mix-Wert im Render-Code außerhalb davon
  (Ausnahmen nur via `// guard:allow: <grund>` / `guard:allow-file`).
- Genau 2 Themes (hell/dunkel), identisch bis auf Farben.
- Branding (Kompass) nur subtil in Login/Bootsplash, nie in Alltags-UI/Startbutton
  (Manifest §3.4/§9/§14).
- Guard grün halten: `cargo test -p meridian-tokens --test design_guard`.
- Definition of Done: `docs/GUI_CENTRALIZATION_PLAN.md` §9.
- Archivierte Mockups sind nicht normativ; Produkt-UI konsumiert ausschließlich
  `meridian-tokens` + `meridian-config`.

## Vor jedem Commit:
- cargo test --workspace grün
- cargo clippy -- -D warnings grün
- design_guard grün (`cargo test -p meridian-tokens --test design_guard`)
