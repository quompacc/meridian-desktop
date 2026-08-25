# Meridian — BSD and Native UI Master Plan

> **STATUS: ACTIVE.** Updated 2026-08-25 from
> `Meridian - BSD - Übergabe für ChatGPT Desktop.md`. This replaces the former
> Arch/KDE-based MeridianOS proposal. Meridian's own compositor is not parked.

## 1. North star

Meridian is a polished Unix desktop with a small, understandable native core
and one coherent Rust UI platform. Its product quality should approach modern
macOS/Windows surfaces without trading away Wayland correctness, security or
idle efficiency.

Rust owns both the operating-system-facing core and Meridian's product UI.
External applications remain free to use their own toolkits.

## 2. Architecture decision

### Native Rust responsibilities

- Wayland compositor and XWayland integration
- DRM/KMS, GPU buffers, input and output management
- window/workspace management and compositor effects
- IPC schemas, policy and lifecycle supervision
- system services and platform integration
- small privileged helpers and security boundaries

### Shared Meridian UI platform

- reusable native layout, widgets, effects and input behavior in `meridian-ui`
- panel, launcher, Quick Settings, notifications, overview and Settings
- shared icons and typography from the central design system
- direct consumption of `meridian-tokens` and `meridian-config`

The archived WebKit prototype is evidence and a design reference only. It is
not a fallback, optional runtime or future target.

## 3. External applications

GTK, Qt, Firefox, Chromium/Electron, wxWidgets and other third-party software
stay independent Wayland clients. Legacy/problematic applications may use
XWayland. Meridian does not replace their toolkits or render their contents.

Compatibility policy:

1. implement protocols correctly;
2. test representative applications on real hardware;
3. use XWayland as a legitimate fallback;
4. avoid toolkit-wide hacks;
5. introduce app quirks only with evidence, tight scope and tests.

## 4. BSD platform decision

### OpenBSD evaluation

OpenBSD is evaluated for its defensive architecture, safe defaults, `pledge`,
`unveil` and culture of privilege separation. Meridian processes should receive
the smallest practical rights, but the desktop must remain useful as a daily
driver.

The Acer laptop is the reference machine:

- Intel Core i7-7500U
- Intel HD Graphics 620, the only GPU used for the evaluation
- NVIDIA GeForce 940MX, intentionally ignored

The exact WLAN, Ethernet, audio, touchpad and Bluetooth devices must be recorded
before installation.

### FreeBSD alternative

FreeBSD remains a serious target and already has repository-specific installer
work. If OpenBSD is blocked by hardware or desktop compatibility,
Meridian will use FreeBSD's own mechanisms—Capsicum, jails, MAC, securelevel,
ZFS and privilege separation—rather than emulating OpenBSD.

### Linux role

Linux remains a development and compatibility platform. Existing Arch and
systemd documentation describes today's implementation, not the final OS
commitment.

## 5. Evidence model

VMs provide fast regression tests, architecture experiments and reproducible
environments. The Acer provides authoritative evidence for DRM/KMS, pageflips,
cursor, input, touchpad, displays, suspend/resume, hotplug, audio, WLAN, browsers
and perceived performance.

The main PC remains untouched during the initial evaluation.

## 6. Execution order

1. inventory and back up the Acer;
2. install OpenBSD and validate base hardware;
3. validate representative browsers, GTK and Qt clients;
4. establish the Rust toolchain and Meridian/Smithay path;
5. polish the native panel, launcher and Quick Settings vertical slice;
6. validate the native UI performance and input path on OpenBSD;
7. decide OpenBSD versus FreeBSD from recorded evidence;
8. expand native Settings and other system tools.

Detailed exit criteria live in `ROADMAP.md` and `docs/OPENBSD.md`.

## 7. Native UI acceptance gates

- deterministic startup without an optional toolkit runtime
- light/dark themes from the single Rust token source
- cached icons/assets and explicit invalidation
- measured cold start, resident memory, idle CPU/GPU and interaction latency
- correct Wayland sizing, scale, input and lifecycle behavior
- small auditable modules and platform boundaries a small project can sustain

## 8. Native quality round

```text
Meridian compositor
       │ typed IPC / policy
       ▼
native meridian-shell
       │ meridian-ui + direct Rust tokens
       ├── panel
       ├── launcher
       └── Quick Settings
```

The native shell is the product implementation. Work is incremental and must
not silently break compositor/shell IPC.

## 9. Security model

- The native shell runs unprivileged and never owns DRM devices or authentication.
- Privileged actions cross a narrow Rust service/helper boundary.
- IPC methods validate types, caller capability, object identity and state.
- OpenBSD sandbox profiles and FreeBSD capability models are platform-specific.
- Shell compromise must not imply root compromise.

## 10. Performance model

- event-driven updates; no polling render loop for static UI
- cached decoded icons, fonts and reusable visual assets
- invalidation only on theme, scale, content or state changes
- compositor effects remain compositor-owned when that avoids duplicated work
- animation/blur/shadow work requires a measured budget and fallback
- Acer hardware is the minimum practical performance reference

## 11. Explicit non-goals

- rebuilding a general Tauri-like framework
- rendering third-party application UIs inside Meridian
- replacing Wayland with a private window protocol
- choosing a BSD by ideology without hardware evidence
- broad new features before the native quality round
- a second independent design-token source
