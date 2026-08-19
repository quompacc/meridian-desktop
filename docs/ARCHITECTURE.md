# Meridian Architecture

> Updated 2026-08-19. Sections explicitly distinguish the current native-shell
> implementation from the target WebKit UI platform.

## Stable system boundary

Meridian is a Wayland compositor and desktop, not a themed layer over another
desktop. Rust remains responsible for protocol correctness, hardware access,
policy, IPC and privileged integration on Linux and BSD.

External applications remain Wayland/XWayland clients in both the current and
target architecture.

## Current implementation

The current executable path is:

```text
boot/login → meridian compositor → native meridian-shell
                                  ├─ panel / launcher / popups / settings
                                  └─ IPC to compositor and system services
```

Important workspace responsibilities:

- `src/main.rs`: backend selection, XWayland, IPC timer and shell watchdog
- `meridian-compositor`: Wayland server, DRM/Winit backends, input, rendering,
  output/workspace policy and compositor IPC
- `meridian-shell`: native Rust layer-shell client and current desktop UI
- `meridian-config`: TOML configuration, themes, outputs and keybindings
- `meridian-tokens`: authoritative design values and guard tests
- `meridian-ipc`: shell/compositor contracts
- `meridian-wm`: workspace, tiling and floating logic
- `meridian-login`, `meridian-lock`: authentication/session surfaces
- `meridian-portal`: portal policy and D-Bus integration
- `meridian-polkit`: authorization agent
- `meridian-ui`: reusable native UI primitives

The exact source inventory is generated in `CODE_INDEX.md`.

## Target UI architecture

```text
Meridian compositor
├─ Wayland / XWayland
├─ DRM/KMS, input and output management
├─ window/workspace policy and effects
└─ typed IPC and supervision
          │
          ▼
Meridian UI runtime (unprivileged)
├─ WebKit lifecycle and Wayland surface integration
├─ capability-scoped Rust bridge
├─ packaged/offline assets
└─ crash containment and diagnostics
          │
          ▼
Meridian UI framework
├─ CSS generated from meridian-tokens/config
├─ Web Components and shared icons
└─ minimal TypeScript state adapters
          ├─ panel
          ├─ launcher
          ├─ Quick Settings
          └─ later Meridian system tools
```

The new runtime is an additional unprivileged client, not a compositor plugin
and not a privileged web view. Detailed requirements are in `UI_PLATFORM.md`.

## Migration boundary

The native shell remains the fallback and behavioral reference until the full
panel/launcher/Quick Settings vertical slice is proven. Migration may extend IPC
additively but must not silently break existing native-shell decoding.

Login and bootsplash stay native during the first slice. Authentication UI is a
separate security decision; the WebKit direction does not automatically move it.

## Compositor / shell / IPC contract

The compositor owns surfaces, focus, workspaces, outputs, final composition and
policy. UI clients request actions; they do not bypass compositor decisions.

Existing IPC includes window/workspace snapshots, focus, launch, config reload,
quit, thumbnails and screenshot mediation. New bridge/state schemas must reuse
or version these semantic contracts rather than duplicate policy in JavaScript.

## Render order

Visual stacking is correctness and remains:

1. background/wallpaper
2. bottom layer surfaces
3. normal application windows
4. top layer surfaces/panel
5. overlay surfaces/launcher/popups
6. cursor

Moving a surface from native drawing to WebKit does not authorize reordering.

## Backends and platforms

- DRM/KMS is the authoritative real-session path.
- Winit/nested execution supports development and regression tests.
- Linux is the currently exercised implementation platform.
- FreeBSD has an existing logind-free installer/session path.
- OpenBSD is the next hardware/portability evaluation; support is not yet
  claimed.

OS integrations live behind explicit platform adapters. OpenBSD `pledge` and
`unveil` and FreeBSD Capsicum/jails/MAC are not treated as interchangeable APIs.

## Wayland and application boundary

The compositor supports or is developing the expected XDG Shell, Layer Shell,
XDG Decoration, SHM, output, data-device, XWayland, dmabuf/sync, session lock,
idle and output-power paths. Protocol correctness takes priority over
application-specific fixes.

GTK, Qt, browsers, Electron and wxWidgets remain external. See `APP_STACK.md`.

## Design-source flow

```text
meridian-tokens + meridian-config
              ├─ native render consumers
              └─ generated CSS variables → shared Web Components
```

There is no hand-maintained parallel CSS palette. The design guard must cover
web assets before production migration.

## Security boundaries

- WebKit runs without root, DRM/input handles or ambient command execution.
- Privileged operations remain in small Rust services/helpers.
- Every bridge call is typed, validated and capability-checked.
- Packaged local content is the default; remote navigation is denied.
- A compromised document must not imply compositor or root compromise.

## Performance-sensitive paths

- compositor DRM/Winit render and damage paths
- decorations, wallpaper and captures
- UI surface commits and WebKit paints
- icon/font decode and launcher population
- bridge event fan-out and state serialization

Static UI must be event-driven. Reusable assets are cached with explicit
theme/scale/content invalidation. The Acer/OpenBSD evaluation provides the
low-end measurement baseline.

## Related documents

- `../MERIDIAN_OS_PLAN.md` — strategy and OS decision
- `../ROADMAP.md` — execution phases
- `UI_PLATFORM.md` — runtime/bridge target
- `PROJECT_STATUS.md` — implemented behavior
- `OPENBSD.md` / `FREEBSD.md` — platform evidence
- `meridian_design_manifest.md` — binding visual specification
