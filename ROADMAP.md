# Meridian — Active Roadmap

> Updated 2026-08-19. This is the forward-looking execution order for the
> BSD/WebKit strategy. Completed native-shell work remains documented in
> `docs/PROJECT_STATUS.md`; older phase estimates are no longer scheduling
> commitments.

## Guiding constraint

No new large desktop feature comes before the UI platform proof. Existing
compositor, protocol, login, portal and FreeBSD work is maintained, but product
expansion waits until the runtime → bridge → panel → launcher → Quick Settings
slice is credible.

## Phase 0 — Acer hardware inventory

Status: **substantially complete on 2026-08-19; backup/recovery record remains.**
The exact model and devices are now recorded in `docs/OPENBSD.md`. The internal
QCA9377 WLAN and Bluetooth functions have no attached OpenBSD driver; Ethernet
is operational.

Identify and record the exact laptop model and these devices before changing
its operating system:

- Intel HD 620 PCI identity and active display connectors
- NVIDIA 940MX identity, while planning not to use it
- WLAN, Ethernet, audio, touchpad and Bluetooth chipsets
- firmware dependencies, suspend state and external-display ports

Exit criterion: `docs/OPENBSD.md` contains the actual device inventory and a
backup/recovery plan.

## Phase 1 — OpenBSD real-hardware baseline

Status: **development baseline active.** OpenBSD patches `001`–`009`, Rust
1.94.1, Wayland/input/seat libraries, XWayland and WebKitGTK 4.1 are installed.
`seatd` and D-Bus are enabled. WebKit compiles and links; rendered GUI,
audio, suspend and external-display tests remain.

Install OpenBSD on the Acer and validate the base system before Meridian:

- Intel GPU acceleration, modesetting and cursor
- keyboard, touchpad, WLAN, Ethernet, audio and Bluetooth as applicable
- external display, hotplug and suspend/resume
- Firefox, Chromium availability, GTK, Qt and WebKit runtime behavior
- Rust toolchain and build prerequisites

VM runs remain useful for repeatability, but hardware results are authoritative.

Exit criterion: a completed pass/fail matrix with blockers classified as
hardware, OS, port/package, upstream protocol or Meridian issues.

## Phase 2 — Meridian core on OpenBSD

Status: **native compositor and input compile paths established.** Portable
tokens/config/IPC/UI/portal/boot crates compile unchanged. The OpenBSD Smithay
port disables only the unavailable `linux-drm-syncobj-v1` eventfd contract;
DRM/KMS, GBM and EGL remain enabled. OpenBSD discovers DRM cards under
`/dev/dri` and reads keyboard/pointer input directly from wscons, without the
udev/libinput compatibility backends. WM and compositor compile, and all 380
compositor library tests pass on OpenBSD. A controlled native session now opens
wscons, initializes EGL/GBM, completes the first 1920x1080 atomic KMS commit and
starts XWayland. Mesa still selects `llvmpipe`; Intel hardware acceleration must
be established before performance claims. The next compile boundary is shell
shared memory (`memfd_create`), followed by BSD Authentication instead of PAM.

Port or isolate Linux assumptions without weakening the existing architecture:

- compile the workspace or a documented subset
- establish the seat/input and DRM/KMS path available on OpenBSD
- run a minimal compositor session with Intel HD 620
- validate Wayland clients and XWayland, if available
- define OpenBSD process boundaries using `pledge`, `unveil` and privilege
  separation where they improve the trusted base

Exit criterion: a minimal Meridian session renders, accepts input and can run a
reference client, or the exact upstream blocker is documented.

## Phase 3 — WebKit UI platform spike

Build the smallest runtime that can prove the architecture:

1. create and manage a WebKit-backed Wayland surface;
2. load bundled HTML/CSS/components without an HTTP server;
3. expose one typed, capability-scoped Rust bridge operation;
4. export light/dark CSS variables from Meridian's central tokens;
5. demonstrate deterministic lifecycle, crash handling and diagnostics;
6. measure cold start, steady idle CPU/GPU, memory and input-to-paint latency.

Tauri may be reused only if its required subset is maintainable on the chosen
BSD target. Meridian will not recreate Tauri wholesale.

Exit criterion: the spike works on the selected BSD reference path and meets a
written performance/security budget.

## Phase 4 — First vertical slice

Migrate in this order:

1. panel/taskbar;
2. launcher;
3. Quick Settings.

Each component must use shared Web Components and generated design tokens. The
native Rust shell remains the fallback until the complete slice is functional.
Render order, compositor policy and IPC compatibility must remain stable.

Exit criterion: all three components can be daily-tested together, survive a
runtime restart and look/behave consistently in both themes.

## Phase 5 — Meridian system applications

Only after Phase 4 succeeds:

- Settings
- package management UI
- ZFS/storage tooling where supported
- system monitor
- notifications and overview migration

Privileged work remains in small Rust services/helpers. UI processes receive
only the capabilities and data required for the active view.

## Continuous tracks

### External client compatibility

Maintain a matrix for Firefox, Chromium/Electron, GTK3/4, Qt5/6, wxWidgets and
representative XWayland applications. Prefer protocol correctness; add
application-specific quirks only as a documented last resort.

### FreeBSD fallback

Keep the existing FreeBSD install path buildable. Evaluate Capsicum, jails, MAC,
securelevel and ZFS according to FreeBSD's own model rather than imitating
OpenBSD APIs.

### Performance

The Acer is the low-end benchmark. Cache decoded icons and reusable visual
assets; invalidate on explicit state changes. No animation, blur or shadow is
accepted without an idle-cost and cache strategy.

### Security

Keep WebKit and untrusted content outside privileged processes. The bridge is
deny-by-default, typed and auditable. Remote navigation and arbitrary command
execution are not implicit runtime features.

## Deferred until the vertical slice is proven

- new large settings categories
- package-manager or storage-manager product work
- broad visual rewrites of the native shell
- toolkit-specific compatibility hacks without protocol evidence
- a final choice between OpenBSD and FreeBSD based on preference alone
