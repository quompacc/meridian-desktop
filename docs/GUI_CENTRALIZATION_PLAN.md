# GUI-Zentralisierung & Redesign — Plan und Audit

> **Zweck dieses Dokuments:** Es ist der Einstiegspunkt für die Umsetzung in einem
> **frischen Chat ohne Vorkontext**. Es enthält das Ziel, einen belegten Audit
> (mit Datei:Zeile), die Zielwerte aus dem Mockup, den phasierten Plan und den
> aktuellen Laufzeit-Zustand der Test-Box. Wer hier anfängt, braucht nichts
> aus früheren Sessions zu kennen.

Stand: 2026-06-19 · Branch `freebsd-port` · letzter Commit-Stand siehe `git log`.

---

## 1. Ziel (Vorgabe des Nutzers)

- **Eine** zentrale Anlaufstelle für Design. Jedes UI-Element zieht Farbe,
  Schriftfarbe, Geometrie, Radien und Effekte **ausschließlich** daraus — keine
  hartverdrahteten Werte mehr.
- Genau **2 feste Designs: Hell und Dunkel**, die sich **nur** in Farb- und
  Schriftfarbe unterscheiden. Layout, Geometrie, Radien, Glas/Blur/Schatten sind
  **identisch**. Theme-Wechsel = nur die Farbtabelle tauschen.
- **Transparenz + Blur fast raus** — nur eine *Andeutung*, exakt wie im Mockup.
- **Harte Referenz:** `assets/bsd_desktop_mockup.png` (Dunkel) und
  `assets/bsd_desktop_mockup_hell.png` (Hell). Beide 1672×941, gleiches Layout —
  Hell-Farben daraus samplen wie bei Dunkel.
- **Reichweite:** alles — Shell, Greeter (`meridian-login`), Lock
  (`meridian-lock`), Compositor-Titlebars.
- **Architektur-Entscheidung des Nutzers:** Den Theme-**Loader behalten**, nur
  noch **2 Themes ausliefern** (kein komplettes Entfernen des Theme-Systems).

## 2. Harte Referenzen & Zugang

- Mockup Dunkel: `assets/bsd_desktop_mockup.png` · Wallpaper: `assets/bsd_wallpaper.png`.
- Mockup Hell: `assets/bsd_desktop_mockup_hell.png` (vom Nutzer geliefert).
- Test-Hardware: FreeBSD 15.1, `ssh meridian-bsd` (passwortlos root, Checkout
  `/root/meridian-desktop`). Build nur dort (Windows baut `wayland-sys` nicht).
  Build-Befehl: `LIBRARY_PATH=/usr/local/lib cargo build --release …`.
  **Release** deployen (Debug-CPU-Rendering ist unbrauchbar langsam).

## 3. Architektur heute (Single-Source existiert, wird aber umgangen)

Zentral vorhanden und teils korrekt genutzt:
- `meridian-config` → `ThemeConfig { colors, decorations, fonts, icons, cursor, wallpaper }`
  ist die strukturelle Single-Source. Geladen aus `themes/<name>/theme.toml`;
  Default-Name `meridian`. Fehlt `[decorations]` in der Datei, greifen serde-Defaults.
- `meridian-tokens` → `Color`/`Palette` (Single-Source der Farben),
  `Interaction` (Hover/Pressed/Accent), `Elevation` (Schatten), `Radius`, `Font`.
- Korrekt zentralisiert: `panel_view.rs`, `context_menu.rs`, `popup_card.rs`.

**Das Problem ist nicht fehlende Struktur, sondern dass viele Stellen sie umgehen.**

## 4. Audit — die belegten Lücken (Datei:Zeile)

### Shell (`crates/meridian-shell`, `crates/meridian-ui`)
- `settings_view.rs`: importiert `Interaction` **nicht**; **~44** hand-gerollte
  `base.lerp(weiß/schwarz, 0.xx)`-Stellen mit verstreuten Faktoren
  (0.06/0.08/0.10/0.12/0.14/0.15/0.16/0.18). Größte Insel. → über `Interaction`.
- `app_view.rs`: `LAUNCHER_BAND_ALPHA=0`, `LAUNCHER_CELL_ALPHA=0`,
  `LAUNCHER_HOVER_ALPHA=42`, `LAUNCHER_SELECTED_ALPHA=56`; Inline-Alphas für
  Scrollbar/Divider (25/44/105/180). (`LAUNCHER_GLASS_ALPHA` wurde bereits an
  `glass_alpha` gekoppelt — siehe §7.)
- `panel_view.rs`: Kompass-Icon ~9 hartverdrahtete `paint_rgba(...)` (Schatten
  92/66/42, Rim 18,22,34, Specular 255-Weiß) → über `Elevation`/Theme.
- `meridian-ui/widget/button.rs` (Z. 140/147/148) und `tile.rs` (Z. 150/151):
  0.15/0.18/0.40 statt `Interaction::DEFAULT.hover()/.pressed()`.
- `thumbnail_popup.rs:58`: `Color::rgb(40,40,55)` Platzhalter → `surface_alt`.
- Sauber: `context_menu.rs`, `popup_card.rs` (nutzen `Elevation::POPUP`, Tokens).

### Greeter (`crates/meridian-login`)
- Lädt das Theme bereits (`LOGIN_THEME` OnceLock, `metro_*`-Helfer lesen
  `login_theme().colors.*`). Aber **~25 Stellen hartverdrahtet**:
  Karte (`main.rs:1491` 20,25,31), Input-Box (`1537/1538`), Submit-Button-Gradient
  (`1760/1764/1770`), Titel/Untertitel (`1548/1556`), Brand-/Kompass (`1805/1844/1857`),
  Icons (`1864/1879`), Cursor (`2080/2085`); Backdrop-Tint-Gradient
  (`visual.rs:102-109`) und Kompass-Guides (`visual.rs:123/138/149/163`).
  Radien `LOGIN_CARD_RADIUS=18`, `LOGIN_CONTROL_RADIUS=8` hartverdrahtet →
  `decorations.corner_radius`/`surface_radius`.
- Schatten nutzen bereits `Elevation::LAUNCHER` (gut).

### Lock (`crates/meridian-lock`)
- **0 % Theme.** Komplett hartverdrahtetes Tokyo-Night (Konstanten `BG/CARD/
  FIELD_BG/FIELD_BORDER/ACCENT/TEXT/DIM/DOT/ERR` in `main.rs:22-31`), lädt nie ein
  Theme, importiert `meridian-config` nicht. Maße/Radien (`CARD_W/H`, 16/9/8) fix.
- **Hürde:** Lock läuft ohne Session-Kontext. **Vorschlag:** Greeter/Session
  schreibt das aktive Theme nach `/run/meridian/theme.toml` (root, world-readable),
  Lock liest es beim Start, Fallback = Default.

### Compositor (`crates/meridian-compositor`) — der systemische Kern
- **`themed_layer_glass_info` (`backend/drm/render.rs:58-76`) reicht
  `treatment.fill_alpha`/`frame_alpha` NIE an `GlassTitlebarInfo`/den Glas-Shader.**
  Folge: Deckkraft von **Panel/Launcher/Popup** ist **nicht** theme-gesteuert —
  nur Blur+Tint kommen vom Compositor, die Deckkraft malt jeweils die Shell selbst
  (genau dein Launcher-Bug, nur systemisch). Layer-Surface-Opacity ist in
  `backend/drm/render/layers.rs:184` auf `1.0` fix.
- Fix nötig: `fill_alpha`/`frame_alpha` in `GlassTitlebarInfo` + Glas-Shader-Uniform
  führen, dann ist `glass_alpha` **der eine** Deckkraft-Regler für alle Flächen.
- Weitere Hardcodes: Button-Zonen-Alpha-Clamps (`decoration/render/elements.rs:441-450`),
  Divider `0.30` (`:516`), `INACTIVE_SHADOW_ALPHA=0.3` (`decoration/render/buffers.rs:5`),
  prozedurales Default-Wallpaper (`build.rs`, nur Fallback).
- Titlebar-Glas selbst liest `glass_alpha`/`glass_blur_radius`/`glass_tint` korrekt.

### Theme-Infra (`crates/meridian-config`, `crates/meridian-tokens`, `themes/`)
- 6 ausgelieferte Themes: `meridian` (dunkel), `meridian-light`, `catppuccin-latte`,
  `catppuccin-mocha`, `earth-cream`, `solarized-dark`. **Alle haben nahezu identische
  `[decorations]`** — nur Farben/Icon-Theme unterscheiden sich. → auf **2** reduzieren.
- `appearance_is_light()` (Luminanz des Backgrounds) existiert, wird aber nirgends
  aktiv genutzt — kann den Hell/Dunkel-Schalter tragen.
- `color.rs` ist die Single-Source der Farben; `Palette::default()` = Tokyo-Night,
  durch Tests abgesichert (Default-Änderung bricht Tests → bewusst anpassen).

## 5. Zielwerte „Andeutung" (aus dem Mockup gemessen)

Flache Flächen im Mockup haben Kanal-Streuung ~5 → **praktisch deckend**. Zentrale
Dekorations-Defaults daher:
- `glass_alpha ≈ 0.92` (**hoch = deckend**; NICHT 0.08 — das wäre fast unsichtbar)
- `glass_blur_radius ≈ 3` (von 10)
- `glass_tint ≈ 0.15` (von 0.5)
- Schatten dezent belassen, Radius wie gehabt (10).
Werte am Schluss am Bildschirm gegen das Mockup feinjustieren.

## 6. Plan (phasiert)

**Leitidee:** Nicht-Farb-Defaults (Dekorationen/Glas/Radien/Fonts/Geometrie) leben
an **einer** Stelle (zentrale Rust-Defaults). Die 2 Theme-Dateien enthalten **nur
`[colors]`** (+ Icon/Wallpaper). Theme-Wechsel = Farbtabelle tauschen.

- **Phase 0 — Fundament & 2 Themes.** Zentrale Nicht-Farb-Defaults bündeln;
  `themes/dark/` + `themes/light/` mit nur `[colors]`; die 4 übrigen Themes raus;
  `Palette::DARK`/`LIGHT`. Zentrale Glas-Werte auf §5 setzen. (Hell-Farben aus dem
  Nutzer-Mockup, sobald geliefert.)
- **Phase 1 — Compositor: Deckkraft verdrahten.** `fill_alpha`/`frame_alpha` in
  den Glas-Shader; `glass_alpha` wird der eine Deckkraft-Regler für Panel/Launcher/
  Popup/Titlebar. Hartverdrahtete Button-/Divider-Clamps lösen.
- **Phase 2 — Shell-Zentralisierung.** `settings_view` (44), `app_view`-Rest-Alphas,
  `panel_view`-Kompass, `ui` button/tile → über `Interaction`/`Elevation`/Theme.
- **Phase 3 — Greeter + Lock.** Greeter-Restwerte über die geladene Quelle; Lock
  via `/run/meridian/theme.toml` an dieselbe Quelle anbinden.
- **Phase 4 — Schutz gegen Rückfall.** Test/Lint, der neue hartverdrahtete
  `lerp(0xFF…)`/Roh-Alphas außerhalb Icons/Tests aufschlägt.

**Arbeitsweise pro Phase (verbindlich):** Build + `cargo test --workspace` +
`cargo clippy -- -D warnings` auf der BSD-Box, Release deployen, Nutzer prüft am
Bildschirm gegen das Mockup. Nach jeder Phase konkret berichten, was grün ist und
was offen — **keine unbelegten „fertig"-Aussagen** (ausdrücklicher Wunsch des Nutzers).

## 7. Aktueller Laufzeit-Zustand der Box (wichtig für den neuen Chat)

Bereits umgesetzt/deployt (Zwischenstand, teils vom Plan später ersetzt):
- **Theme**: `themes/meridian/theme.toml` neutralere graublaue Palette +
  `glass_alpha = 0.75`. Deployt nach `/usr/local/share/meridian/themes/meridian/`.
- **Launcher-Fix**: `app_view.rs` — Launcher-Körper koppelt an
  `surface_treatment(Launcher).fill_alpha` (war hart `0`). Release-Shell gebaut,
  nach `/usr/local/bin/meridian-shell` installiert (Backup `…-shell.bak`).
- **Icons**: `papirus-icon-theme` per pkg installiert; aktives Icon-Theme in
  `/root/.config/meridian/config.toml` auf **`Papirus`** (Basis-Theme hat
  `firefox.svg` + `system-file-manager.svg` direkt; `Papirus-Dark` erbt nur
  breeze-dark und hätte sie nicht). Backups: `meridian-shell.bak`, `main.rs.bak`,
  `visual.rs.bak` im Login-src.
- **App-Launch**: Default-Handler für `https` von kaputtem `userapp-Firefox-…`
  auf `firefox.desktop` zurückgesetzt (`xdg-mime`). Files startet pcmanfm
  (dolphin war nicht installiert; altes Binary gab fälschlich dolphin zurück).
- Session wurde zuletzt neu gestartet (Greeter aktiv). Der Nutzer hatte die
  jüngste Shell-/Theme-Version **noch nicht final am Bildschirm gegengeprüft**.

Offene Verifikation: Launcher-Deckkraft/Graublau/Launch nach Re-Login bestätigen.

## 8. Offene Entscheidungen / Risiken

- **Hell-Mockup** liegt vor (`assets/bsd_desktop_mockup_hell.png`). Struktur so
  bauen, dass Hell = eine Farbtabelle ist; Farben daraus samplen.
- **Lock-Theme-Auslieferung**: Vorschlag `/run/meridian/theme.toml` (vs. Wayland-
  Protokoll-Extension). Im Plan: einfache Datei-Variante zuerst.
- `Palette::default()`-Tests brechen bei Default-Farbänderung — bewusst mit anpassen.
- `corner_radius` aus dem Theme wird im Greeter/Lock noch nicht genutzt — bei der
  Umstellung Layout gegen das Mockup prüfen.

## 9. Definition of Done (gesamt)

Kein hartverdrahteter Farb-/Alpha-/Radius-Wert in Render-Code außerhalb von Icons/
Tests; Panel/Launcher/Popup/Titlebar/Greeter/Lock beziehen Deckkraft+Farbe aus
`ThemeConfig`; genau 2 Themes (hell/dunkel), identisch bis auf Farben; Transparenz/
Blur nur als Andeutung wie im Mockup; Guard-Test gegen Rückfall grün; alles auf der
BSD-Box gebaut/getestet/deployt und vom Nutzer gegengeprüft.
