# Meridian — Active Roadmap

> Updated 2026-08-21. This is the forward-looking execution order for the
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
`seatd` and D-Bus are enabled. Accelerated Meridian rendering, the OpenBSD
audio backend and WebKitGTK panel/launcher rendering are proven on hardware;
suspend and external-display tests remain.

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
udev/libinput compatibility backends. WM and compositor compile, and all 381
compositor library tests pass on OpenBSD. A controlled native session now opens
wscons, initializes EGL/GBM, completes the first 1920x1080 atomic KMS commit and
starts XWayland. Meridian routes OpenBSD libdrm's privileged device-open hook
through seatd, so the normal user now renders with Intel HD Graphics 620 and 99
DMA-BUF formats instead of `llvmpipe`. Sustained performance remains unmeasured.
The shared-memory boundary is also resolved: OpenBSD shell screencopy, lock and
polkit surfaces use native `shm_mkstemp(3)` with close-on-exec semantics, while
existing targets retain `memfd_create`; all 310 shell tests pass. Login and lock
use BSD Authentication through `auth_userokay(3)`, with PAM retained only on
other targets; Polkit retains its native helper protocol without an unused PAM
dependency. The compositor-supervised lock lifecycle, input, real-password
authentication and explicit unlock are proven on the OpenBSD Intel reference
machine. The Wayland/UI lock process is installed root-owned without special
group privilege; only the minimal `/usr/local/libexec/meridian-openbsd-auth`
helper is setgid `auth` (never setuid root). This split is verified with the real
account password on hardware. `cargo check --workspace` and all Meridian
workspace tests pass on OpenBSD (the vendored Smithay example package is
explicitly excluded). Bad-password retry, repeated cycles and fail-closed client
loss before acquisition, while pending, after acquisition and during the auth
helper are proven on hardware. Interactive login and the remaining lock
multi-output variant are the next runtime gaps. Super+L, the desktop context
menu and the launcher power menu now share the compositor-owned lock supervisor;
dark and light theme cycles pass through each typed entry path.
The `meridian-lock` sandbox pilot is also complete on the reference hardware:
the unprivileged UI and setgid auth helper install separate checked
`pledge(2)`/`unveil(2)` profiles with no unsandboxed fallback. Normal unlock,
bad-password retry, repeated cycles, helper failure/recovery and acquired-client
loss passed; the latter remained compositor-owned `LockedFailClosed`.

Port or isolate Linux assumptions without weakening the existing architecture:

- compile the workspace or a documented subset
- establish the seat/input and DRM/KMS path available on OpenBSD
- run a minimal compositor session with Intel HD 620
- validate Wayland clients and XWayland, if available
- define OpenBSD process boundaries using `pledge`, `unveil` and privilege
  separation where they improve the trusted base; the fail-closed rollout begins
  with the `meridian-lock` pilot in `docs/OPENBSD_SANDBOX_PLAN.md`

Exit criterion: a minimal Meridian session renders, accepts input and can run a
reference client, or the exact upstream blocker is documented.

## Phase 3 — WebKit UI platform spike

Status: **working vertical proof on OpenBSD since 2026-08-20.** The versioned
CSS-token export, ephemeral WebKitGTK 4.1 runtime, bundled assets and typed
deny-by-default bridge are live. The shell supervises a Web panel process and
falls back to its native panel if that process exits. The panel toggles a
separate Web launcher process through authenticated IPC; catalogue loading,
category filtering, search and app activation are wired. Both surfaces render
on the Acer in the live DRM session.

Launcher cold-start was measured on 2026-08-21: catalogue loading costs 1 ms,
while a newly spawned WebKit document costs 362-469 ms. The managed path now
prewarms one hidden launcher and reuses it; toggle-to-layer-map dropped to
0.18-5.1 ms across repeated opens. Its stdin control is event-driven, with no
polling or hidden animation. Remaining performance work is to set a memory
budget: the resident launcher currently accounts for roughly 77 MiB host,
92 MiB Web process and 51 MiB network-process RSS (shared pages included).
Evaluate process consolidation only with proportional-memory evidence.

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

Status: **daily-testable on OpenBSD.** Panel, launcher and Quick Settings are
available together behind `MERIDIAN_WEB_UI_PANEL=1`; appearance, real status
data, volume controls, hardware audio keys and context-aware navigation into
Settings are wired. Hidden persistent popups are input-transparent. The native
panel/launcher paths remain fallback while runtime compatibility and visual
polish are completed.

Migrate in this order:

1. panel/taskbar;
2. launcher;
3. Quick Settings.

Each component must use shared Web Components and generated design tokens. The
native Rust shell remains the fallback until the complete slice is functional.
Render order, compositor policy and IPC compatibility must remain stable.

Exit criterion: all three components can be daily-tested together, survive a
runtime restart and look/behave consistently in both themes.

The secure OpenBSD XWayland server acceleration path is operational. Meridian
opens only the active primary/render pair through seatd and passes those
explicit descriptors to unprivileged Xwayland; device-node permissions remain
unchanged and Xwayland initializes Glamor. The experimental extension of the
bridge to X11 GL clients did produce the Intel HD 620 with `Accelerated: yes`,
but failed the 2026-08-22 interactive stability gate: FreeCAD stopped opening
and Blender crashed when leaving maximized geometry. Automatic client preload
is therefore disabled and applications retain the stable software fallback
until the OpenBSD Mesa/DRI3 path can be fixed without regressions.

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
