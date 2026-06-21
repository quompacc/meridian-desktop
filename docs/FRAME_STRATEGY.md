# Meridian — Window-Frame-Strategie (der Schlachtplan)

**Status:** Entscheidung getroffen 2026-06-21. Umsetzung in Phasen, später.
**Kurzfassung:** Eine uniforme, vom Compositor gezeichnete Meridian-Titelleiste
über **alle** Apps ist auf Wayland **nicht erreichbar**. Wir fahren deshalb
**zweigleisig**: volle Kontrolle für die Kern-Apps (SSD / Eigenbau), und für den
Rest CSD akzeptieren und nur farblich integrieren — wie elementary OS / macOS.

---

## 1. Was wir live bewiesen haben (2026-06-21, Arch-Box)

Empirisch, nicht neu zu verhandeln:

1. **libadwaita/GTK4 (Ptyxis) und GTK3 (Nemo) binden gar kein `xdg-decoration`.**
   Compositor-Log nach dem Öffnen beider Apps: zweimal `new xdg toplevel`, aber
   **kein einziges** `request_mode` / `new_decoration`. Folge: der Compositor
   bekommt nie die Chance, ServerSide zu erzwingen — `force-SSD` greift schlicht
   nie. Dazu `decoration render: skip … has_ssd=false`.
2. **`GTK_CSD=0` wird ignoriert**, sobald eine App eine explizite `GtkHeaderBar`
   setzt (alle modernen GTK/libadwaita-Apps tun das). Es deaktiviert nur die
   *automatische* CSD, nicht eine bewusst gesetzte HeaderBar.
3. **Folge:** Step 2 des [`SSD_FRAME_PLAN.md`](SSD_FRAME_PLAN.md) (force-SSD +
   `GTK_CSD=0`) ist **verworfen**. [`APP_STACK.md`](APP_STACK.md) hatte recht:
   bei diesem Stack zeichnet jede App ihren eigenen Rahmen.

## 2. Warum das so ist (Toolkit-Realität, kein Bug)

- **GNOME/GTK = CSD-Philosophie.** Die Titelleiste ist Teil der *App*
  (HeaderBar verschmilzt Titel + Menü + Suche + Buttons). Auf Wayland gibt es
  keinen erzwingenden Fenstermanager wie unter X11 — GTK nutzt das bewusst.
- **libadwaita-Theming ist bewusst zugesperrt:** nur benannte Farben
  (`@define-color`), kein freies Widget-CSS mehr. Fremde Desktops *sollen* die
  Optik nicht übernehmen können.
- **`xdg-decoration` ServerSide wird von GTK absichtlich nicht angeboten.**
- **Gegenbeispiele, die SSD respektieren:** Qt (über `xdg-decoration`),
  Wayland-Terminals (`foot`, `alacritty`, `wezterm`), SDL, viele Spiele.
- **GTK-Lebenszyklus:** GTK3 ist im Wartungsmodus, GTK4→GTK5 bringt denselben
  Bruch, libadwaita wird eher strenger. **Meridians Kern-Identität darf nicht
  vom Schicksal eines fremden Toolkits abhängen.**

## 3. Die zwei Gleise

### Gleis A — Volle Kontrolle (echte Meridian-SSD-Leiste, wie im Mockup)
Apps, bei denen der **Compositor** die token-getriebene Leiste zeichnet:
- **Terminal:** `foot` oder `alacritty` (beide SSD, heute bei Alacritty
  verifiziert — trägt die Meridian-Leiste samt der neuen grauen Buttons).
- **Qt-Apps:** respektieren ServerSide; Theming aber ohne Plasma fummelig
  (Grund, warum KDE gedroppt wurde) — nur dosiert einsetzen.
- **Eigenbau-Apps in Rust** (der wirklich „neutrale" Weg): eigenes Rendering,
  kein fremdes Toolkit, Designtokens direkt aus `meridian-tokens`. Die Shell
  macht das bereits (smithay-Rendering).

### Gleis B — Lange Leine (CSD akzeptiert, nur farblich integriert)
GTK4/libadwaita-Apps zeichnen ihre eigene HeaderBar; wir vereinheitlichen nur
Farbe/Form über Tokens:
- `@define-color`-Palette nach `~/.config/gtk-{3,4}.0/gtk.css` (**erledigt**,
  `theme_export.rs`).
- GTK-CSS-Formanpassung (Eckenradius/Shadow) soweit libadwaita es zulässt
  (`window.csd` honoriert es; Widget-CSS nicht).
- **Bewusst nicht pixel-uniform.** Diese Apps bleiben „Gäste".

## 4. Phasenplan (später umsetzbar)

**Phase 0 — erledigt (2026-06-21):**
- Graue Mockup-Buttons auf der Meridian-SSD-Leiste (`decoration/render/elements.rs`).
- libadwaita-Palette nach `~/.config/gtk-{3,4}.0/gtk.css` (`theme_export.rs`).
- Build/Test/Install nur auf der Arch-Box festgeschrieben (`CLAUDE.md`).
- Step 2 (force-SSD + `GTK_CSD=0`) verworfen und zurückgenommen.

**Phase 1 — SSD-Pfad sauber machen (klein, Gleis A):**
- Default-Terminal auf ein SSD-Terminal festlegen (`foot` bevorzugt: klein,
  schnell, sauber konfigurierbar; sonst `alacritty`). Beide nur aus Arch `extra`.
  - **2026-06-21 entschieden:** `alacritty` (bereits installiert) bleibt Default;
    es trägt die SSD-Leiste live verifiziert (`request_mode … forcing
    ServerSide`). `foot` bleibt offene Option (in `extra`, nicht installiert).
    Die Fallback-Liste in `terminal_program()` / `prepare_launch()` listet
    `foot` zuerst — ein späteres `pacman -S foot` befördert es automatisch zum
    Default, ohne Code-Change. **Kein Code-Change nötig.**
- Step 1 des SSD-Plans abschließen: Titelleisten-Ton/Höhe/Ecken + exakte
  Button-Alphas gegen das Mockup feinjustieren (Screenshot-Iteration auf der Box).
  - **2026-06-21 (teilweise):** Ruhe-Chrome der Fensterknöpfe entfernt — das
    Mockup zeigt nur saubere graue `─□×`-Glyphen ohne Hintergrund. Raus sind
    die drei frosted Ruhe-Zonen, die zwei Trennlinien, der Ruhe-Frost-Schleier
    und die Tint-Fallback-Pille; es bleibt nur die Hover-Wölbung (frosted bei
    `glass_blur`), Close grau. Glass+Blur lebt weiter auf der Titelleisten-Fläche
    selbst. (`decoration/render/elements.rs`.) Gebaut + Gate grün + deployt auf
    der Box. **Noch offen:** On-Box-Screenshot-Verifikation + ggf. Feinschliff
    von Höhe/Eckenradius/Hover-Alpha (Session war beim Deploy nicht eingeloggt).
- Verifizieren: SSD-Apps tragen die Leiste pixelgenau wie im Mockup. **(offen —
  Screenshot-Abgleich steht noch aus.)**

**Phase 2 — Kern-App-Inventar (Entscheidungsphase, kein Code):**
- Pro Default-App-Kategorie aus `APP_STACK.md` einsortieren in: **Gleis A** (SSD
  vorhanden / Eigenbau lohnt) vs **Gleis B** (CSD akzeptieren).
- Liste finalisieren: Dateimanager, Editor, Bildbetrachter, PDF, Terminal,
  Taschenrechner, Archiv, Medien, Browser. Browser/Office bleiben Gleis B.

**Phase 3 — Eigenbau-Machbarkeit (Gleis A, Prototyp):**
- Toolkit-Wahl evaluieren: **Slint** vs **iced** vs **egui** (Kriterien:
  SSD-Support, Token-/Theming-Kontrolle, Wartung, Binärgröße, A11y).
  Empfehlung als Default: Slint (deklarativ, eigenes Rendering, gut themebar) —
  in Phase 3 final entscheiden.
- Ein schmaler Prototyp (z. B. minimaler Dateimanager oder Bildbetrachter) mit
  echter Meridian-SSD-Leiste + Tokens, als Tracer-Bullet.

**Phase 4 — Token-Bridge / Meridian-UI-Kit (Gleis A, Fundament):**
- Ein wiederverwendbares Crate, das `meridian-tokens` für Eigenbau-Apps
  bereitstellt (Farben/Spacing/Radius/Fonts), damit jede neue App ohne Copy-Paste
  Meridian-konform ist. Idealerweise teilen Shell + Apps Widgets.

**Phase 5 — schrittweise Substitution:**
- Default-Apps der Gleis-A-Kategorien nacheinander durch Eigenbau ersetzen,
  Gleis-B-Apps farblich integriert belassen. Kein „Big Bang".

## 5. Leitplanken (gelten für alle Phasen)
- **CLAUDE.md bleibt bindend:** nur offizielle Repos (kein AUR), Tokens als
  einzige Design-Quelle, design_guard grün, bauen/testen/installieren **nur auf
  der Arch-Box**.
- **Keine Wette auf ein fremdes Toolkit** für die Kern-UX.
- **Ehrliche Grenze kommunizieren:** „uniform über alles" ist unmöglich; Ziel ist
  „uniform über das, was wir kontrollieren" (elementary-OS-Modell).

## 6. Offene Fragen
- Slint vs iced vs egui — endgültige Wahl (Phase 3).
- Wartungsbudget für Eigenbau-Apps realistisch?
- `foot` vs `alacritty` als Default-Terminal.
- Wie viel GTK-CSS-Formangleich bei Gleis B lohnt sich, bevor es Augenwischerei wird?

## 7. Referenzen
- [`APP_STACK.md`](APP_STACK.md) — Gleis-B-Begründung (GTK zeichnet selbst).
- [`SSD_FRAME_PLAN.md`](SSD_FRAME_PLAN.md) — Step 1 (SSD-Styling, läuft weiter),
  Step 2 (verworfen, siehe oben).
- [`meridian_design_manifest.md`](meridian_design_manifest.md) — maßgebliche
  Design-Spezifikation (schlägt im Konflikt jede andere Quelle).
