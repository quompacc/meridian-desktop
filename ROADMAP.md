# Meridian — Active Roadmap

> Updated 2026-08-25. This is the forward-looking execution order for the
> BSD-first native-Rust strategy. `docs/NATIVE_UI_PLAN.md` is binding for UI;
> older WebKit phases are retained only as historical evidence.

## Guiding constraint

No new large desktop feature comes before the native panel → launcher → Quick
Settings quality round. Existing compositor, protocol, login, portal and
FreeBSD work remains maintained.

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
The second `meridian-polkit` pilot is complete as well. Its root-owned but
unprivileged XDG-autostart binary resolves the OpenBSD ConsoleKit cookie,
requires successful polkit registration and Wayland bootstrap before applying
its checked `pledge(2)`/`unveil(2)` profile, and can execute only the packaged
setuid helper path. Real bad/good-password handling, three repeated
authorizations, missing-session startup and agent loss during an outstanding
request passed; client loss returned `not authorized` and never ran the root
action. The packaged helper remains an explicit upstream privileged boundary.
The third `meridian-portal` pilot is complete. Its installed OpenBSD backend owns
only Settings, Screenshot and Access, runs with a checked `pledge(2)`/`unveil(2)`
profile and requires the live compositor socket before entering its event loop.
FileChooser is deliberately routed to OpenBSD's separate GTK portal backend so
the Meridian D-Bus process never receives ambient access to the user's files.
Real frontend Settings, visible FileChooser-cancel, missing-socket,
non-socket-substitution and screenshot consent/deny/allow tests pass. The allow
path produced a valid local 1920x1080 PNG while the backend remained `pU`.
A clean session restart also reactivated both portal backends automatically and
repeated the Settings and FileChooser frontend paths without manual repair.

Port or isolate Linux assumptions without weakening the existing architecture:

- compile the workspace or a documented subset
- establish the seat/input and DRM/KMS path available on OpenBSD
- run a minimal compositor session with Intel HD 620
- validate Wayland clients and XWayland, if available
- define OpenBSD process boundaries using `pledge`, `unveil` and privilege
  separation where they improve the trusted base; the fail-closed rollout begins
  with the completed `meridian-lock`, `meridian-polkit` and `meridian-portal`
  pilots in
  `docs/OPENBSD_SANDBOX_PLAN.md`

Exit criterion: a minimal Meridian session renders, accepts input and can run a
reference client, or the exact upstream blocker is documented.

## Phase 2a — Boot and login experience

Status: **required, not yet evaluated end to end on OpenBSD.** A polished boot
is part of the desktop product, not an optional post-release detail. The normal
path should move from firmware to Meridian login without visible diagnostic
noise, avoid black gaps and unnecessary display-mode changes, and present a
native Meridian splash or at minimum a restrained logo. The visual treatment
must follow `docs/meridian_design_manifest.md` §12: boot and login are the
deliberate branding moments; everyday shell UI remains unbranded.

Work in this order:

1. inventory the OpenBSD boot loader, kernel and `rc` output that can be hidden
   through supported configuration rather than an unmaintainable kernel fork;
2. preserve boot diagnostics in logs and document an explicit verbose/recovery
   boot path before muting the normal console;
3. bring up a cached, idle-free Meridian splash or static logo as early as the
   supported graphics path allows;
4. verify a clean handover from splash to `meridian-login`, then from login to
   the user compositor, without an exposed console, black gap or input loss;
5. test failure behavior so silent boot never turns a broken boot into an
   unexplained permanent black screen.

Exit criterion: a recorded cold boot on the OpenBSD reference machine reaches
the greeter with no routine kernel/`rc` text in the normal path, the recovery
path remains usable, and splash/login geometry and branding match the manifest.

## Phase 3 — WebKit prototype (closed)

Status: **completed as an experiment and retired from production on
2026-08-25.** GTK3/WebKitGTK 4.1 and GTK4/WebKitGTK 6 proved that the desired
panel, launcher and Quick Settings design is practical on OpenBSD. Measurements
also exposed the maintenance and memory cost of a browser stack on the BSD-first
path. The runtime, bridge, CSS exporter and compositor workarounds are no longer
part of the Cargo workspace.

The accepted assets and hardware screenshots remain under
`docs/design-reference/webkit-prototype/` as a read-only visual reference.

## Phase 4 — Native UI quality round

Status: **active.** The native Rust panel, launcher, popups and Settings path are
again the only product implementation. There is no runtime flag or toolkit
fallback. Work follows `docs/NATIVE_UI_PLAN.md`:

1. align native panel composition with the accepted reference;
2. polish native launcher layout, search and keyboard behavior;
3. rebuild the combined Quick Settings card natively on the proven network,
   audio, battery and power backends;
4. validate both themes, scaling, repeated input cycles and idle performance.

Exit criterion: all three native surfaces are daily-testable on OpenBSD, match
the manifest and remain idle when their state does not change.

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

Privileged work remains in small Rust services/helpers. The native shell
receives only the capabilities and data required for the active view.

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

Keep the native shell unprivileged. Privileged actions cross small typed,
deny-by-default service/helper boundaries; platform sandboxing stays explicit.

## Deferred until the native quality round is proven

- new large settings categories
- package-manager or storage-manager product work
- broad visual rewrites outside panel, launcher and Quick Settings
- toolkit-specific compatibility hacks without protocol evidence
- a final choice between OpenBSD and FreeBSD based on preference alone
