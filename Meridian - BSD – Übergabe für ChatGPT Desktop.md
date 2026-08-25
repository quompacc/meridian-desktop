# Meridian / BSD – aktueller Stand und nächste Schritte

Ich entwickle **Meridian Desktop**, einen eigenen Desktop/Wayland-Compositor in Rust. Das Projekt läuft grundsätzlich unter Linux und soll explizit auch BSD unterstützen.

Repository:
`https://github.com/quompacc/meridian-desktop`

## Zielbild

Ich möchte Meridian langfristig als modernen, sehr polished Desktop aufbauen – mit klarer Unix/BSD-Philosophie darunter, aber mit einer UI, die optisch und ergonomisch auf dem Niveau moderner macOS-/Windows-Oberflächen liegt.

Das bisherige Problem: Die komplett in Rust selbst gerenderte UI ist für einen Einzelentwickler zu aufwendig. Selbst scheinbar einfache Dinge wie gute Schatten, Blur/Glass, Layout und hochwertige Animationen kosten extrem viel Entwicklungszeit.

Deshalb wollen wir die UI-Strategie grundlegend ändern.

## Neue Meridian-UI-Architektur

Rust bleibt für:

- Wayland-Compositor
- DRM/KMS
- Input
- Window Management
- XWayland
- IPC
- Systemdienste
- Plattformintegration
- privilegierte Helper
- Sicherheitsgrenzen

Die komplette Meridian-eigene UI soll dagegen auf eine gemeinsame Web-basierte UI-Plattform umgestellt werden:

- WebKit-basierte Runtime
- HTML
- CSS
- möglichst wenig TypeScript/JavaScript
- gemeinsame Meridian Web Components
- gemeinsames Designsystem
- gemeinsame Icons
- gemeinsame Design Tokens
- Rust ↔ UI Bridge

Das betrifft ausdrücklich **nicht nur Apps**, sondern auch:

- Taskleiste / Panel
- App Launcher
- Quick Settings
- Notifications
- Overview
- Settings
- Paketmanager
- Storage-/ZFS-Tools
- System Monitor
- weitere Meridian-eigene Systemtools

Die Idee ist, dass alle Meridian-Komponenten ein gemeinsames UI-Framework verwenden, sodass Buttons, Slider, Menüs, Popovers, Dialoge, Listen usw. nur einmal entwickelt und gestaltet werden müssen.

Konzeptionell etwa:

```text
Meridian Compositor
├── Wayland / XWayland
├── DRM/KMS
├── Input
├── Window Management
├── Window-Level Effects
└── IPC
        │
        ▼
Meridian UI Runtime
├── WebKit
├── Rust ↔ JS/TS Bridge
├── Surface/Window Integration
└── Lifecycle / Permissions
        │
        ▼
Meridian UI Framework
├── CSS Design Tokens
├── Web Components
├── Icons
├── Typography
├── Animation primitives
├── Buttons / Inputs
├── Lists / Grids
├── Menus / Popovers
└── Dialogs
        │
        ├── Panel
        ├── Launcher
        ├── Quick Settings
        ├── Settings
        ├── Packages
        └── weitere Meridian Apps
```

Wichtig: Wir wollen **nicht Tauri komplett nachbauen**. Wenn Tauri auf BSD sauber funktioniert, kann man Teile davon nutzen. Ansonsten soll eine möglichst kleine Meridian-eigene Runtime entstehen, die nur die APIs bietet, die wirklich gebraucht werden.

HTMX wurde diskutiert, ist aber wahrscheinlich nicht die beste Grundlage, weil Meridian keinen natürlichen HTTP-Server besitzt. Wahrscheinlicher ist:

**HTML + CSS + Web Components + kleine TypeScript-Schicht + Rust Bridge.**

## Fremde Anwendungen

GTK-, Qt5/Qt6-, Firefox-, Chromium-/Electron- und wxWidgets-Anwendungen sollen weiterhin normale externe Wayland- bzw. XWayland-Clients bleiben.

Meridian soll diese nicht selbst rendern oder ersetzen.

Bestehende Strategie:

```text
GTK / Qt / Firefox / Chromium
          │
        Wayland
          │
       Meridian

Legacy / problematische Apps
          │
         X11
          │
       XWayland
          │
       Meridian
```

Diese Architektur existiert im Projekt bereits weitgehend.

Probleme gibt es aktuell teilweise mit echter Hardware bzw. bestimmten Anwendungen:

- Firefox macht mehr Probleme als Chromium.
- GTK/Qt funktionieren grundsätzlich gut, haben aber einzelne Edge Cases.
- Bambu Studio ist durch wxWidgets/GTK und eigene Fenster-/Decoration-Logik problematisch und hat bereits viel Debugging-Zeit gekostet.

Strategie dafür:

- Wayland-Protokolle möglichst korrekt implementieren.
- XWayland als legitimen Fallback verwenden.
- Keine unnötigen Toolkit-spezifischen Hacks.
- Referenz-/Compatibility-Testmatrix für Firefox, Chromium, GTK3/4, Qt5/6, wxWidgets usw. aufbauen.
- App-spezifische Quirks nur als letzter Ausweg.

## BSD-Entscheidung

Linux hat mich als Desktopplattform nie wirklich überzeugt. Gründe sind u. a. Fragmentierung, Distributionen als zusammengesetzte Systeme und die allgemeine Komplexität.

BSD gefällt mir konzeptionell wesentlich besser, weil es stärker als vollständiges Betriebssystem gedacht ist.

Ursprünglich war **FreeBSD** die favorisierte Plattform.

Inzwischen interessiert mich **OpenBSD** sehr stark, insbesondere wegen:

- defensiver Systemarchitektur
- sichere Defaults
- `pledge`
- `unveil`
- Privilege Separation
- kleiner, verständlicher Komponenten
- sehr konsequenter Sicherheitsphilosophie

Die Meridian-Architektur könnte davon profitieren.

Beispielidee:

```text
Meridian UI
   ↓ IPC
unprivilegierter Rust-Service
   ↓
kleiner privilegierter Helper
```

Auf OpenBSD könnten Meridian-Prozesse jeweils mit möglichst kleinen `pledge`-/`unveil`-Rechten laufen.

Allerdings darf Sicherheit nicht dazu führen, dass der Daily Driver praktisch unbrauchbar wird.

Deshalb bleibt **FreeBSD weiterhin eine ernsthafte Alternative**, wenn OpenBSD bei Hardware oder Desktop-Kompatibilität zu viele Einschränkungen hat.

FreeBSD soll nicht künstlich in einen OpenBSD-Klon verwandelt werden. Wenn FreeBSD verwendet wird, sollen dessen eigene Mechanismen sauber genutzt werden, z. B.:

- Capsicum
- Jails
- MAC
- securelevel
- ZFS
- Privilege Separation

## GPU / Hardware

Mein Haupt-PC ist der Daily Driver und soll zunächst unangetastet bleiben.

Hauptrechner ungefähr:

- Intel Core i9 14xxxK
- NVIDIA RTX 4070 Super
- eventuell später AMD RX 9070 XT

RDNA4 war zunächst ein Grund zur Sorge. OpenBSD hat jedoch bereits Navi-48/RX-9070-Unterstützung im DRM-Stack, was überraschend positiv ist. Trotzdem soll der Haupt-PC nicht zum experimentellen Testsystem werden.

## OpenBSD-Testhardware

Ich habe einen älteren Acer-Laptop, der als echte Testmaschine dienen soll:

- Intel Core i7-7500U
- Intel HD Graphics 620
- NVIDIA GeForce 940MX
- 2 GB VRAM auf der NVIDIA

Plan:

- Für OpenBSD zunächst ausschließlich die Intel HD 620 verwenden.
- NVIDIA 940MX ignorieren.
- OpenBSD nativ auf dem Acer testen.
- Keine VM als primäre Aussage über Hardware-Kompatibilität verwenden.

Der Acer ist absichtlich eine eher schwache/ältere Plattform. Wenn Meridian dort sauber und flüssig läuft, ist das ein guter Performance-Benchmark.

## Warum echte Hardware?

Bei früheren Meridian-Tests gab es massive Unterschiede zwischen VM und echter Hardware.

In der VM lief vieles problemlos, auf echter Hardware traten dagegen Probleme auf.

Deshalb:

**VM**
- schnelle Regressionstests
- Architekturtests
- reproduzierbare Entwicklungsumgebung

**Acer / echte Hardware**
- DRM/KMS
- GPU
- Pageflips
- Cursor
- Input
- Touchpad
- externe Displays
- Suspend/Resume
- Hotplug
- Audio
- WLAN
- Firefox/Chromium
- GTK/Qt
- WebKit
- echte Performance
- Meridian selbst

Die echte Hardware ist die maßgebliche Referenz.

## Nächster konkreter Schritt

Jetzt soll der Acer als OpenBSD-Testgerät vorbereitet werden.

Als Erstes sollte die genaue Acer-Hardware identifiziert werden, insbesondere:

- exaktes Modell
- WLAN-Chip
- Ethernet
- Audio
- Intel-GPU
- Touchpad
- eventuell Bluetooth

Danach:

1. OpenBSD installieren.
2. Intel HD 620 als einzige relevante GPU verwenden.
3. Basis-Desktop-/Hardwarefunktion prüfen.
4. Rust-Toolchain prüfen.
5. Meridian/Smithay/DRM-Pfad testen.
6. Firefox/Chromium/GTK/Qt testen.
7. Suspend/Resume und Input testen.
8. Danach mit der neuen Meridian-WebKit-UI-Runtime beginnen.

## Priorität für Meridian

Noch keine neuen großen Features.

Zuerst soll die neue UI-Plattform bewiesen werden.

Erster Vertical Slice:

```text
WebKit Runtime
    ↓
Rust ↔ UI Bridge
    ↓
neues Meridian Panel
    ↓
Launcher
    ↓
Quick Settings
```

Wenn diese drei Komponenten sauber funktionieren und überzeugend aussehen, wird anschließend die restliche Meridian-UI darauf migriert.

Danach können Apps wie:

- Settings
- Package Manager
- ZFS/Storage Manager
- System Monitor

auf derselben Plattform aufgebaut werden.

Langfristige Leitidee:

**Native where it matters. Web where it shines.**

Rust besitzt System, Performance, Sicherheit und OS-Integration.

HTML/CSS besitzt das moderne UI-Design.