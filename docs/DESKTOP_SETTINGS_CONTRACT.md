# Desktop Settings Contract v0

## Ziel

Meridian soll globale Desktop-Einstellungen standardnah und toolkit-neutral bereitstellen.

- Keine App-Hacks.
- Keine Policy im Compositor-Hotpath.
- Apps sollen Desktop-Identity, Theme, Cursor, Font und Color-Scheme konsistent erkennen koennen.

## Nicht-Ziele

- Kein globales Erzwingen von CSD/SSD.
- Kein Firefox-spezifischer Fix.
- Kein GTK-only Design.
- Kein direkter Eingriff in App-Profile.
- Kein pro-App Environment-Hack als langfristige Loesung.

## V0 Settings (perspektivisch)

Meridian sollte als ersten, kleinen Contract perspektivisch diese Settings global bereitstellen:

- Color-Scheme / Dark-Light Preference
- Cursor Theme
- Cursor Size
- Icon Theme
- Base Font / UI Font

Optional spaeter:

- Accent Color
- Decoration/Button-Layout nur falls als Desktop-Policy bewusst entschieden

## Transport und Architektur

- Bevorzugt ueber eine separate Settings-/Portal-Schicht.
- Nicht direkt im Compositor-Hotpath.
- `meridian-config` bleibt die Quelle fuer interne Defaults.
- `meridian-portal` oder ein separater Settings-Daemon kann eine standardisierte Read-only-Schnittstelle fuer Apps bereitstellen.
- XSettings, GSettings und Portal-Settings sind moegliche Integrationsrichtungen, aber in v0 noch nicht implementiert.

## Verantwortungsgrenzen

- Compositor: Protokolle, Fensterverwaltung, Input, Session-Basis.
- Shell/Settings: User-facing Einstellungen.
- Config: persistente Meridian-Einstellungen.
- Portal/Settings-Service: app-seitige Desktop-Settings-Signale.
- Apps/Toolkits: finale CSD/Headerbar/Window-Control-Darstellung.

## Bezug zur Firefox/Headerbar-Beobachtung

- Firefox ist ein Testfall, kein Sonderziel.
- Das Ziel ist globale Desktop-Integration, damit Toolkits korrekte CSD/Headerbar/Window-Control-Entscheidungen treffen koennen.
- Keine Firefox-spezifische Decoration-Policy.

## Patch-Disziplin

Zuerst Contract und Dokumentation, danach kleine, getrennte Patches:

- A. Session-Datei/Packaging pruefen
- B. Portal-Settings-Skeleton
- C. Read-only Config-Export
- D. einzelne Settings-Key-Gruppen

Keine Mixed-Patches.
