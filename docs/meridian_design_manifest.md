# Meridian Desktop – Design Manifest

> **STATUS: VERBINDLICH.** Dies ist die maßgebliche Design-Spezifikation für
> Meridian. Jede UI-/Render-Änderung muss ihr entsprechen; bei Konflikt schlägt
> dieses Manifest jede andere Quelle (Audits, Altcode). Verankert als Regel in
> `AGENTS.md` und `CLAUDE.md`. Zentralität wird durch den Guard-Test
> `cargo test -p meridian-tokens --test design_guard` erzwungen.

> **Nativer Produktpfad:** Meridian-eigene UI wird in Rust gerendert. Der
> archivierte WebKit-Prototyp ist nur eine visuelle Referenz; er ist weder
> normativ noch Teil des Builds und erlaubt kein zweites Designsystem.

### Verbindliche Token-Pipeline

`meridian-tokens` (`Palette`, `Interaction`, `Elevation`, `Radius`) plus
`meridian-config` (`Decorations`) bleiben die einzige editierbare Designquelle.
Native Renderer konsumieren diese Werte direkt; Web-UI konsumiert daraus
generierte CSS Custom Properties nur innerhalb der archivierten Referenz.
Produktive UI konsumiert die Rust-Tokens direkt. Marken-Assets und Tests bleiben
nur mit expliziter Begründung ausgenommen.

## 1. Grundidee

**Meridian Desktop** steht für:

- Ruhe
- Präzision
- Klarheit
- Materialität
- technische Eleganz

Meridian soll **nicht laut** sein. Es soll **souverän** wirken.

Die Oberfläche ist nicht verspielt, nicht überdekoriert und nicht demonstrativ futuristisch. Sie wirkt modern durch:

- gute Proportionen
- weiche Materialien
- subtile Tiefe
- klare Hierarchie
- kontrollierte Akzente

---

## 2. Kerngedanke

### Leitsatz

**Meridian muss man nicht ständig sehen – man muss es spüren.**

Das bedeutet:

- Die tägliche UI bleibt neutral.
- Sichtbares Branding bleibt Boot und Login vorbehalten.
- Der Desktop selbst ordnet sich dem Nutzer und seinem Wallpaper unter.

---

## 3. Designprinzipien

### 3.1 Neutral vor dekorativ

Die Shell darf nie mit dem Wallpaper konkurrieren.

**Regel:**

- Oberfläche zurückhaltend
- Wallpaper darf wirken
- UI bleibt lesbar und stabil

### 3.2 Weich statt hart

Meridian nutzt keine aggressive Tech-Ästhetik.

**Bevorzugt:**

- weiche Kontraste
- sanfte Übergänge
- leicht getönte Glasflächen
- subtile Schatten

**Vermeiden:**

- Neon-Glow
- extreme Kanten
- übertriebene Leuchteffekte
- Hacker-/Sci-Fi-Optik

### 3.3 Präzision ohne Kälte

Die Oberfläche soll präzise und technisch sein, aber nicht steril.

Deshalb:

- klares Raster
- gute Ausrichtung
- kontrollierte Abstände
- ruhige Geometrie
- aber mit etwas Wärme in Helligkeit und Material

### 3.4 Funktion vor Symbolik

Meridian ist kein Kompass-Theater.

Navigation, Meridian, Orientierung und Kartografie sind **Inspirationen**, keine Dauerillustrationen.

Das heißt:

- keine große Kompassrose in jedem Fenster
- keine Weltkarte in jeder App
- keine Branding-Grafik als Dauerdeko

Stattdessen:

- kreisförmige Motive
- klare Achsen
- Balance
- richtungsbezogene Geometrie
- ruhige Zentrierung

### 3.5 Material statt Effekt

Blur, Schatten und Glas sind kein Selbstzweck.

Sie sollen:

- Tiefe schaffen
- Lesbarkeit verbessern
- Fläche von Hintergrund trennen
- Wertigkeit vermitteln

Nicht:

- Aufmerksamkeit schreien
- als Tech-Demo wirken

---

## 4. Farbwelt

### Ziel

Die Farbwelt soll:

- dunkel
- weich
- entsättigt
- hochwertig
- wallpaper-kompatibel

sein.

### 4.1 Primärpalette

#### Hintergrund / Basis

| Rolle | Hex | Beschreibung |
|---|---:|---|
| Nachtblau | `#0B1220` | Tiefes, ruhiges Basisdunkel |
| Blau-Grau dunkel | `#111A28` | Allgemeiner dunkler Hintergrund |
| Grundfläche | `#162131` | Weiche UI-Basis |

#### Oberflächen

| Rolle | Hex | Beschreibung |
|---|---:|---|
| Glasbasis | `#1B2737` | Dunkle Glasfläche |
| Sekundäre Oberfläche | `#223145` | Panels, Menüs, erhöhte Flächen |
| Helle Surface-Variante | `#2A3A50` | Hover, leichte Betonung |

#### Linien / Borders

| Rolle | Hex | Beschreibung |
|---|---:|---|
| Standard-Border | `#31445C` | Normale Kontur |
| Aktive Border | `#40556E` | Aktiver Zustand |
| Highlight-Border | `#536A86` | Fokus / leichte Hervorhebung |

#### Text

| Rolle | Hex | Beschreibung |
|---|---:|---|
| Primärtext | `#EAF0F7` | Haupttext |
| Sekundärtext | `#C1CBD8` | Labels, Untertitel |
| Tertiärtext | `#93A1B4` | Deaktiviert, Hinweise |

#### Akzent

| Rolle | Hex | Beschreibung |
|---|---:|---|
| Primärer Akzent | `#6EA6FF` | Auswahl, Fokus, Hauptaktion |
| Hover/Fokus | `#88B8FF` | Hover und Fokushelligkeit |
| Gedrückt/Aktiv | `#4E87D8` | Pressed State |
| Sehr sparsames Highlight | `#97C5FF` | Nur für kleine Details |

### 4.2 Akzent-Regel

Akzentfarbe wird **gezielt** eingesetzt.

Nur für:

- Fokus
- aktive Auswahl
- aktive Taskbar-Items
- Buttons mit Hauptaktion
- Slider / Toggles / Auswahlzustände
- Links / wichtige Interaktion

Nicht für:

- komplette Flächen
- große Hintergründe
- dekorative Dauerbeleuchtung

### 4.3 Verbotene Farbrichtung

Vermeiden:

- knalliges Cyan
- Neonblau
- stark gesättigte Verläufe
- bunte UI-Teile ohne funktionalen Grund

Meridian ist **gedämpft**, nicht grell.

---

## 5. Typografie

### Typografischer Charakter

Die Schrift muss:

- ruhig
- klar
- modern
- sachlich

wirken.

### Regeln

- Primär: neutrale UI-Sans
- keine verspielten oder dekorativen Schriften
- keine Script-/Handschrift-Schrift im Standard-UI
- Brandtitel dürfen leichtes Letterspacing haben

### Einsatz

#### Brand / Login / Boot

- `MERIDIAN` in Großbuchstaben
- großzügiges Letterspacing
- sachlich, elegant

#### UI

- normale Lesbarkeit
- kompakt, aber nicht gequetscht
- Labels eher unauffällig
- Dialoge klar hierarchisch

---

## 6. Formensprache

### 6.1 Grundformen

Meridian nutzt:

- Rechtecke mit Radius
- Linien
- Kreise
- Bögen
- ruhige, ausgewogene Achsen

### 6.2 Radien

| Element | Radius |
|---|---:|
| Kleine Controls | 10–12 px |
| Eingabefelder | 12–14 px |
| Fenster / Panels | 14–18 px |
| Große Dialoge / Launcher | 16–20 px |

**Nicht zu rund, nicht zu kantig.**

### 6.3 Linien

- dünn
- präzise
- zurückhaltend
- eher 1 px bzw. visuell leicht

Kein schweres Einrahmen.

---

## 7. Materialsystem

### 7.1 Glasflächen

Meridian verwendet Glassmorphism kontrolliert.

#### Ziel

- Hintergrund leicht durchscheinen lassen
- Fenster klar vom Wallpaper trennen
- Materialtiefe erzeugen

#### Eigenschaften

- dunkle Tönung
- geringer bis mittlerer Blur
- leichte Innenaufhellung möglich
- dezente Border

#### Faustregel

Glas darf niemals so transparent sein, dass Lesbarkeit leidet.

### 7.2 Transparenz

Empfohlene Opazität:

| Element | Opazität |
|---|---:|
| Normale Fenster | 82–92 % |
| Taskbar | 78–88 % |
| Launcher / Menüs | 84–92 % |
| Login-Panel | 88–94 % |

Je unruhiger das Wallpaper, desto opaker die Fläche.

### 7.3 Schatten

Schatten sind zentral für Meridian.

#### Schattenwirkung

- weich
- realistisch
- tief, aber unaufdringlich
- nicht klebend

#### Empfehlung

- 1 Primärschatten
- 1 leichter Kontaktschatten oder Ambient-Layer
- keine übertriebenen Drop-Shadow-Wolken

#### Ziel

Die Fläche soll schweben, nicht schreien.

---

## 8. Desktop-Grundregeln

### 8.1 Desktop selbst

Der Desktop ist **neutraler Träger**, nicht Hauptmotiv.

Er soll:

- auf dunklen wie helleren Wallpapern funktionieren
- sich visuell zurücknehmen
- die Fenster wirken lassen

### 8.2 Desktop-Icons

- einfach
- sauber
- leicht modernisiert
- keine verspielte Comic-Optik

Icons sollten:

- auch auf Foto-Wallpapern gut lesbar sein
- genug Kontrast haben
- nicht überglänzen

---

## 9. Taskbar / Panel

### Grundrichtung

**Windows-artig, unten, durchgehend, funktional.**

Das bleibt.

### Eigenschaften

- horizontale Leiste unten
- leicht transparent
- dunkel getönt
- sanfter Blur
- schmale Trennungen
- klare Gruppierung

### Linke Seite

- Startbutton / Launcher-Button
- angeheftete Apps
- laufende Apps

#### Startbutton

Kein buntes Logo und kein Meridian-/Kompass-Motiv. Der Button verwendet ein
neutrales, vertrautes Launcher-Symbol aus dem zentralen Icon-System. Meridians
Identitaet entsteht im Alltag aus Proportion, Material und Verhalten, nicht aus
einem wiederholten Markenzeichen.

### Aktive Apps

Aktive Apps werden markiert durch:

- feine helle Unterstreichung
- leichten Glow
- minimale Aufhellung

Nicht durch:

- große harte Kacheln
- grelle Hintergründe

### Rechte Seite / Tray

- kompakt
- aufgeräumt
- wenig visuelles Rauschen
- Uhr, Netzwerk, Audio, Akku, Sitzung

### 9.1 Launcher

Der Launcher ist ein **vollstaendiger, vertrauter Anwendungskatalog**. Seine
Informationsdichte darf hoeher sein als die der Settings, solange Hierarchie,
Tastaturbedienung und Positionen stabil bleiben.

Verbindliche Struktur:

- dauerhaft sichtbares Suchfeld; Tippen startet unmittelbar die Suche
- Favoriten als vorhersehbare Startansicht
- vollstaendige alphabetische App-Liste
- Kategorien als Filter, nicht als verschachtelte Menuebaeume
- kompakte Listen oder Raster ohne grosse dekorative Karten
- feste Fusszeile fuer Benutzer, Settings, Sperren und Sitzung
- vollstaendige Bedienung mit Pfeiltasten, Tab, Enter und Escape

Nicht erlaubt:

- Vollbild-Dashboard als Standard
- News, Werbung oder Remote-Inhalte
- allgemeine Shell-/Befehlsausfuehrung ueber die Suche
- Branding-Flaechen, Kompassgrafik oder Logo-Kacheln
- wechselnde Positionen fuer Sitzungs- und Sicherheitsaktionen

Der Launcher darf funktional an bewaehrte KDE-Menues erinnern, kopiert aber
weder deren konkrete Optik noch deren Konfigurationsbreite. Meridian liefert
eine einzige sorgfaeltig abgestimmte Variante.

---

## 10. Fensterdesign

### Eigenschaften

- dunkle Glasflächen
- weiche Ecken
- reale Schatten
- klare Titelleisten
- dünne Border
- Fokuszustände mit Akzent

### Titelleiste

- ruhig
- wenig Chrom
- keine unnötigen Effekte
- Buttons dezent, aber gut klickbar

### Aktives Fenster

Merkmale:

- minimale Aufhellung
- klarere Border
- leicht sichtbarer Akzent
- optional subtile aktive Linie

### Inaktive Fenster

- etwas matter
- geringerer Kontrast
- kein starker Schattenunterschied

### 10.1 Settings und Quick Settings

Settings folgen dem Prinzip **weniger ist mehr**: gute Defaults, wenige
Hauptbereiche und progressive Offenlegung statt einer flachen Liste aller
technisch moeglichen Optionen. Bestehende Faehigkeiten werden sinnvoll
gruppiert, nicht entfernt.

Verbindliche Informationsarchitektur:

- Erscheinungsbild
- Desktop & Apps
- Anzeigen
- Netzwerk & Bluetooth
- Geraete
- Audio
- Energie
- Benutzer & Sicherheit
- System

Regeln:

- haeufige Sofortaktionen gehoeren in Quick Settings
- dauerhafte Systementscheidungen gehoeren in Settings
- aufgabenspezifische Optionen bleiben bei der betroffenen Aufgabe
- Aenderungen wirken unmittelbar; riskante Aenderungen erhalten Rueckfall
- seltene Optionen liegen unter einer klar bezeichneten Erweiterung
- Suche findet Unteroptionen und gebraeuchliche Synonyme
- keine globale Speichern-Schaltflaeche und keine leeren Zukunftskategorien

Settings duerfen sich an der Ruhe und progressiven Offenlegung von GNOME/macOS
orientieren, verwenden aber ausschliesslich Meridians Komponenten und Tokens.

---

## 11. Login-Design

Hier darf Meridian **sichtbarer** sein.

### Login soll transportieren

- Identität
- Ruhe
- Präzision
- Willkommen, aber sachlich

### Regeln

- zentriertes Panel
- weiches Glas
- starke Typografie
- ruhiger Hintergrund
- Meridian-Anklänge erlaubt

### Erlaubt

- dezente Kompass-/Meridian-Geometrie
- zentrale Achse
- kreisförmige Linien
- Berg-/Landschafts-Wallpaper oder ruhiges abstraktes Motiv
- kleine Brand-Grafik

### Nicht erlaubt

- zu technische HUD-Optik
- Neon-Radar
- zu viele Linien
- überdekorierte Seefahrtsromantik

---

## 12. Bootsplash

Der Bootsplash ist der stärkste Branding-Moment.

Hier darf Meridian am klarsten sichtbar sein.

### Ziel

- atmosphärisch
- elegant
- ruhig
- hochwertig
- sofort wiedererkennbar

### Geeignet

- weiches Berg-/Himmels-/Horizontmotiv
- sehr subtile Kompass-/Meridianstruktur
- klare vertikale Achse
- Ladeanimation schlicht und präzise

---

## 13. Wallpaper-Regel

### Wichtigstes Prinzip

**Die Shell muss mit jedem Wallpaper gut aussehen.**

Deshalb:

- keine fest eingebauten Motive in der Standard-UI
- keine farbliche Abhängigkeit vom Default-Wallpaper
- Glasflächen neutral halten
- Borders und Text immer robust genug

### Default-Wallpaper

Das Default-Wallpaper bleibt atmosphaerisch, aber markenneutral:

- Horizont
- Berge
- Wasser
- ruhige Landschaft
- weiche Dämmerungsfarben

Keine Kompass-, Meridian- oder Logo-Grafik. Sichtbares Branding bleibt Login
und Bootsplash vorbehalten.

---

## 14. Do / Don’t

### Do

- dunkle, weiche Blau-Grau-Töne
- zurückhaltende Glasflächen
- echte Tiefe
- ruhige Kontraste
- präzise Proportionen
- kleine Meridian-Hinweise
- klare Windows-artige Taskbar unten
- subtile Akzentnutzung

### Don’t

- Kompass überall
- Weltkarten in jeder Oberfläche
- Neon-Cyan
- zu harte Schwarz-Blau-Kontraste
- macOS-Dock-Kopie
- zu viel Branding in Alltagsfenstern
- dekorative Script-Schriften
- Sci-Fi-Kontrollzentrum-Overkill

---

## 15. Meridian in einem Satz

**Meridian Desktop ist eine ruhige, präzise, glasartige Desktop-Oberfläche mit subtiler Navigations-DNA, die den Nutzer und sein Wallpaper respektiert.**

---

## 16. Praktische Umsetzungsreihenfolge

### Phase 1

- Farbpalette finalisieren
- Theme-Tokens anpassen
- Border-/Text-/Surface-System vereinheitlichen

### Phase 2

- Taskbar neutralisieren
- aktive Zustände überarbeiten
- Fensterflächen und Titelleisten angleichen

### Phase 3

- Bootsplash neu
- Login neu

### Phase 4

- Launcher
- Settings
- File Manager / System Apps optisch angleichen

---

## 17. Kurzfassung als interne Regeln

1. **UI nie lauter als das Wallpaper.**
2. **Weiche Blau-Grau-Palette statt harter Tech-Farben.**
3. **Akzentfarbe nur funktional einsetzen.**
4. **Glas und Schatten für Materialität, nicht für Show.**
5. **Meridian im Alltag subtil, im Boot/Login sichtbar.**
6. **Taskbar bleibt unten und Windows-artig.**
7. **Klare Typografie, keine verspielten Fonts.**
8. **Präzision + Ruhe schlagen Deko + Effekte.**
9. **Launcher ist vollstaendig und vertraut; Settings sind kuratiert und ruhig.**
10. **Meridian wird an Verhalten und Proportion erkannt, nicht an Alltags-Branding.**
