# Meridian — External Application Compatibility

> Updated 2026-08-19. This replaces the former “GTK-only application stack” as
> product policy. The previous GTK findings remain relevant to client-side
> decorations, but Meridian no longer chooses one toolkit as its architecture.

## Boundary

Third-party applications are ordinary clients:

```text
GTK / Qt / Firefox / Chromium / Electron / wxWidgets
                         │
                      Wayland
                         │
                      Meridian

legacy or incompatible clients
                         │
                       X11
                         │
                     XWayland
                         │
                      Meridian
```

Meridian-owned surfaces and tools use the native Rust shell. Meridian does not
embed, replace or restyle arbitrary external applications.

## Compatibility policy

1. Prefer protocol-correct Wayland behavior.
2. Keep XWayland as a supported, legitimate fallback.
3. Respect client-side versus server-side decoration ownership.
4. Do not force global SSD or `GTK_CSD=0` to chase uniform visuals.
5. Do not introduce toolkit-specific compositor policy.
6. Add an application quirk only after a minimal reproducer, upstream/protocol
   analysis and a tightly scoped test.

GTK and libadwaita commonly draw their own header bars and decorations. Qt and
other clients may negotiate server-side decorations. Meridian must honor each
protocol path rather than double-frame clients.

## Reference matrix

Record version, backend (Wayland/XWayland), result and known issue on each
reference OS and real-hardware run.

| Family | Reference | OpenBSD | FreeBSD | Linux | Notes |
|---|---|---|---|---|---|
| Browser | Firefox | pending | usable, gaps tracked | reference | historically harder than Chromium in some Meridian paths |
| Browser | Chromium | pending | known platform gap | reference | test native Wayland and fallback separately |
| GTK3 | Thunar | XWayland quirk verified 2026-08-22 | pending | reference | launcher scopes `GDK_BACKEND=x11` + `GTK_CSD=0` to Thunar so Meridian owns SSD |
| GTK4/libadwaita | representative modern app | pending | pending | reference | client owns header bar |
| Qt5 | representative app | pending | pending | reference | verify decoration negotiation |
| Qt6 | representative app | pending | pending | reference | verify decoration negotiation |
| wxWidgets | Bambu Studio or smaller reproducer | pending | pending | problematic | avoid app-specific compositor hacks |
| Electron | representative app | pending | pending | pending | record Ozone/Wayland flags if required |
| XWayland | representative legacy app | pending | pending | reference | isolate X11-specific policy |

“Pending” is not support. Replace it only with a dated test result and evidence.

## Default applications

Default-app selection is a packaging/product decision per operating system, not
a compositor constraint. Choose maintained applications available from trusted
platform repositories and set defaults using freedesktop MIME associations when
the platform supports them.

Do not require Plasma/GNOME session infrastructure merely to make a default app
work. Prefer applications that behave correctly as standalone Wayland clients.

## Relationship to theming

Meridian may export compatible color/theme hints through standard desktop
mechanisms. Those exports are best-effort integration, not a promise that all
toolkits share identical geometry. The binding visual system applies to
Meridian-owned UI; external clients retain their toolkit conventions.

See `FRAME_STRATEGY.md` for the historical decoration findings and
`NATIVE_UI_PLAN.md` for Meridian-owned UI.
