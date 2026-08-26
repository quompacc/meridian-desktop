# OpenBSD Suspend/Resume Input Investigation — 2026-08-26

## Ergebnis

Die Untersuchung ist abgeschlossen. Auf dem OpenBSD-Referenzgerät fällt die
Zeigerbewegung des internen Elantech-Touchpads nach Suspend/Resume reproduzierbar
aus. Tastatureingaben und Mausklicks kommen weiterhin an. Ein Neustart stellt die
Zeigerbewegung wieder her.

Der Fehler ist als OpenBSD-`pms(4)`-/Gerätefirmware-Blocker klassifiziert, nicht
als Meridian-Fehler. Die entscheidende Gegenprobe reproduzierte denselben
Kernel-Fehler nach einem frischen Neustart mit null laufenden Meridian-Prozessen
und einem direkten `doas /usr/sbin/zzz`. Meridian erhebt auf dieser Hardware
daher vorerst keinen Anspruch auf funktionierendes Suspend/Resume.

## Testumgebung

- Acer Aspire F5-573G
- OpenBSD 7.9/amd64, `GENERIC.MP#4`
- Syspatches `001` bis `009`
- Elantech Clickpad v4 an `pms0`/`wsmouse0`, Firmware `0x4d5f02`
- Intel HD Graphics 620 über `inteldrm0`
- Meridian als nativer Rust-Wayland-Compositor mit separater nativer Shell

## Reproduktion mit Meridian

1. Das Gerät frisch starten und genau eine Meridian-Sitzung öffnen.
2. Prüfen, dass Zeigerbewegung, Klicks und Tastatur funktionieren.
3. Suspend über Meridians Systemaktion auslösen und das Gerät wieder wecken.
4. Bis zur sichtbaren Meridian-Sitzung vergehen auf diesem Gerät ungefähr zwei
   Minuten.
5. Danach funktionieren Tastatur und Klicks, aber keine Zeigerbewegung.
6. Ein vollständiger Neustart stellt die Zeigerbewegung wieder her.

## Unabhängige Gegenprobe

Nach einem frischen Neustart wurde vor dem Test geprüft, dass weder Compositor,
Shell noch ein anderer Meridian-Prozess lief. Anschließend wurde Suspend direkt
mit `doas /usr/sbin/zzz` ausgelöst. Beim Aufwachen erschien dieselbe
`pms0`-Fehlerfolge wie im Meridian-Lauf. Meridian wurde erst danach gestartet;
die Zeigerbewegung funktionierte in dieser neuen Sitzung zunächst wieder.

Damit liegt die auslösende Störung unterhalb von Meridians wscons-Lese- und
Ereignisverarbeitungspfad.

## Kernel-Evidenz

In den Hardwareläufen wiederholte sich beim Aufwachen diese Folge:

```text
pms0: disable error
pms0: disable error
pms0: disable error
pms0: enable error
pms0: not in sync yet...
pms0: device reset (state = 2)
```

Der aktuelle OpenBSD-Quellpfad versetzt `pms0` bei `DVACT_QUIESCE` in den
Suspend-Zustand und initialisiert das Protokoll bei `DVACT_WAKEUP` erneut. Die
Meldungen entstehen in diesem Kernel-/Treiberpfad. Das Elantech-v4-Protokoll
liefert intern absolute und Multitouch-Daten; die frühere Arbeitshypothese eines
von Meridian nicht behandelten absoluten wscons-Ereignisses wurde durch
Quellprüfung und Gegenprobe verworfen.

Maßgebliche OpenBSD-Quellen:

- [`sys/dev/pckbc/pms.c`](https://raw.githubusercontent.com/openbsd/src/master/sys/dev/pckbc/pms.c)
- [`sys/dev/wscons/wsmouse.c`](https://raw.githubusercontent.com/openbsd/src/master/sys/dev/wscons/wsmouse.c)
- [`sys/dev/wscons/wsconsio.h`](https://raw.githubusercontent.com/openbsd/src/master/sys/dev/wscons/wsconsio.h)
- [Änderungshistorie von `pms.c`](https://github.com/openbsd/src/commits/master/sys/dev/pckbc/pms.c)

In der zum Untersuchungszeitpunkt aktuellen offiziellen Quelle wurde kein
neuerer Resume-Fix gefunden, der diesen Pfad auf dem Referenzgerät erkennbar
ändert.

## Durchgeführte Meridian-Experimente

Alle folgenden Varianten wurden einzeln auf der realen OpenBSD-Maschine gebaut
und nach Suspend/Resume geprüft:

| Experiment | Ergebnis |
|---|---|
| wscons vor der Autorisierung schließen | Falsche Reihenfolge: Authentifizierungsoberfläche verlor Eingabe; verworfen |
| ConsoleKit-Delay-Inhibitor und `PrepareForSleep`, wscons nach erfolgreicher Autorisierung schließen | Authentifizierung korrekt, Zeigerbewegung weiterhin ausgefallen |
| wscons über Suspend behalten, nach 500 ms aktualisieren | Keine Wiederherstellung |
| Neuen wscons-FD vor Schließen des alten öffnen | Keine Wiederherstellung |
| wscons nach Authentifizierung schließen und nach 2 s neu öffnen | Keine Wiederherstellung |
| Meridian nach dem Fehler vollständig beenden, wscons 10 s geschlossen lassen und eine neue Sitzung starten | Keine Wiederherstellung |
| Frischer Neustart, null Meridian-Prozesse, direktes `zzz` | Derselbe Kernel-/Treiberfehler; Meridian als Ursache ausgeschlossen |

Die erfolglosen FD-, Timing- und Reopen-Workarounds werden nicht im Produktcode
behalten.

## Beibehaltener Meridian-Pfad

Beibehalten wird ausschließlich der korrekte, typisierte Schlafzyklus:

- Die Shell autorisiert Suspend zuerst über die vorhandene ConsoleKit-/Polkit-
  Grenze.
- Auf OpenBSD hält sie einen ConsoleKit-Delay-Inhibitor und verarbeitet
  `PrepareForSleep`.
- Ein typisiertes IPC-Acknowledge synchronisiert Shell und Compositor, bevor
  der Inhibitor freigegeben wird.
- Nach dem Aufwachen setzt der Compositor seinen DRM-Zustand zurück, hebt
  veraltete `frame_in_flight`-Zustände auf und fordert genau den nötigen neuen
  Frame an.
- FreeBSD behält seinen getrennten Plattformpfad; aus dem OpenBSD-Befund wird
  kein gemeinsames BSD-Verhalten abgeleitet.

Der Pfad ist ereignisgesteuert. Es gibt keinen neuen Idle-Timer, kein
kontinuierliches Repaint und keine Änderung am Asset- oder Icon-Cache.

## Abschluss und mögliche spätere Fortsetzung

Die Untersuchung wird nicht weiter im Meridian-Produktcode verfolgt. Falls das
Hardwareproblem später erneut aufgenommen wird, sind sinnvolle nächste Schritte:

1. einen vollständigen OpenBSD-Fehlerbericht mit `dmesg`, genauer Firmware und
   der direkten `zzz`-Reproduktion erstellen;
2. eine eng begrenzte `pms(4)`-Quirk-Hypothese ausschließlich in einem separaten
   Testkernel wie `/bsd.meridian-test` prüfen;
3. den normalen `/bsd`-Kernel unverändert lassen und einen sicheren
   Rückfall-Bootpfad erhalten.

Auf dem Referenzgerät ist derzeit kein `/usr/src/sys`-Quellbaum installiert.
Ein Kernelversuch wäre deshalb eine neue, ausdrücklich zu beauftragende
Untersuchung und kein Teil dieser Meridian-Qualitätsrunde.
