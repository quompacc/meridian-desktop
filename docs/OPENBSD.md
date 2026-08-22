# Meridian on OpenBSD — Evaluation Guide

> **STATUS: BASE SYSTEM PATCHED, DEVELOPMENT STACK PROBED.** Hardware was
> inventoried and the first native build matrix was run over SSH on 2026-08-19.
> OpenBSD support is not yet claimed: the native compositor now completes an
> atomic KMS frame with Intel hardware acceleration and opens wscons input on
> the reference laptop. Native shared memory and BSD Authentication adapters
> are in place; full interactive login and daily-driver validation remain.

## Reference hardware

- Device: Acer Aspire F5-573G
- Firmware: Insyde V1.15 (2016-09-19), UEFI 2.5
- CPU: Intel Core i7-7500U
- Memory: 16 GiB DDR4-2400 (2 × 8 GiB)
- Primary GPU: Intel HD Graphics 620 (`8086:5916`, `inteldrm0`, generation 9)
- Internal panel: 1920×1080 through `inteldrm0`
- Discrete GPU: NVIDIA GeForce 940MX (`10de:179c`), unconfigured and
  intentionally unused
- WLAN: Qualcomm Atheros QCA9377 (`168c:0042`), **not configured by a driver**
- Ethernet: Realtek RTL8168/RTL8411B (`10ec:8168`, `re0`), active at 1 Gbit/s
- Audio: Intel HDA (`8086:9d71`) with Realtek ALC255, `azalia0`/`audio0`
- Touchpad: Elantech Clickpad v4 through `pms0`/`wsmouse0`
- Bluetooth: Lite-On USB `04ca:3015`, generic `ugen0`; no Bluetooth driver
  attached
- Webcam: Realtek USB `0bda:57f2`, `uvideo0`/`video0`
- Card reader: Realtek RTL8411B, `rtsx0`/`sdmmc0`
- Storage: 512 GB HFS512G39TND-N21 SATA SSD plus Slimtype DVD drive
- Battery: Panasonic AS16A5K; 2.14 Ah last-full vs. 2.80 Ah design capacity

OpenBSD 7.9/amd64 is already installed. The storage layout uses the standard
separate OpenBSD filesystems with a large `/home`. A backup/recovery plan still
needs to be recorded before destructive experiments.

Device serial number and system UUID were deliberately omitted from this
repository document.

## Phase A — Pre-install inventory

Record:

- exact model/SKU and firmware version
- disk model, partition map and SMART health
- PCI and USB device IDs
- display connectors and native panel mode
- current WLAN firmware/driver
- touchpad transport (I2C/PS2/USB) and audio codec
- suspend/resume behavior on the current OS
- backup verification and bootable recovery media

Store raw command output in a dated test report rather than paraphrasing device
IDs. The commands depend on the current OS; use its native PCI, USB, network,
audio and disk inventory tools.

## Phase B — Base OpenBSD installation

Use the Intel GPU path. Do not spend the first evaluation cycle enabling the
940MX. Preserve a console and network recovery route.

Record:

- OpenBSD release and patch level
- firmware installed by the base system/firmware tooling
- `dmesg` and display modes
- packages and local configuration changes
- any device disabled in firmware

Observed baseline (2026-08-19):

- OpenBSD 7.9 `GENERIC.MP#4`, amd64, after patches `001` through `009`
- Intel microcode, Intel DRM, webcam and VMM firmware installed
- `fw_update -n`: add none, update none
- `syspatch` installed the available base/X/security patches `001` through
  `009`; the machine was rebooted into the patched kernel
- only Ethernet is available as a network interface (`re0`)
- SSH and `doas` work for user `eduard`; root SSH is not required
- `seatd` and `messagebus` are enabled and running; `eduard` remains in
  `wheel` and is additionally in `_seatd`

## Phase C — Hardware matrix

Use `pass`, `partial`, `fail`, `not available` or `not tested`, with evidence.

| Area | Result | Evidence / blocker |
|---|---|---|
| Intel KMS / native panel | pass (detection/modeset) | `inteldrm0`, DRM/render nodes, 1920×1080 console |
| accelerated graphics | pass (first frame) | EGL/GBM 25.0.7 selects Intel HD Graphics 620 as user `eduard`; 99 DMA-BUF formats and the first atomic KMS frame are proven |
| hardware cursor | not tested | |
| keyboard | partial | `pckbd0`/`wskbd0` attached; desktop interaction test pending |
| touchpad move/click/scroll | partial | Elantech v4 attached as `pms0`/`wsmouse0`; gestures pending |
| WLAN | fail | QCA9377 is present but `not configured`; no WLAN interface |
| Ethernet | pass | `re0`, active 1000baseT full duplex, IPv4/IPv6 configured |
| audio output/input | partial | ALC255 mixer/play/record paths exposed; audible playback/record pending |
| Bluetooth | fail/unsupported baseline | `04ca:3015` only attaches as generic `ugen0` |
| webcam | pass (detection) | Realtek device attaches as `uvideo0`/`video0` |
| external display | not tested | |
| display hotplug | not tested | |
| suspend/resume | not tested | `apmd` is not enabled/running yet |
| lid close/open | not tested | |

## Phase D — Desktop/toolkit matrix

| Component | Result | Version / evidence |
|---|---|---|
| Firefox Wayland | available, not installed/tested | package `firefox-154.0` |
| Chromium Wayland/X11 | available, not installed/tested | package `chromium-147.0.7727.101p0` |
| GTK3 reference app | not tested | |
| GTK4 reference app | available, not installed/tested | package `gtk+4-4.22.3` |
| Qt5 reference app | not tested | |
| Qt6 reference app | available, not installed/tested | package `qt6-qtbase-6.10.2` |
| wxWidgets reference app | not tested | |
| XWayland | installed, runtime not tested | `xwayland-24.1.12` |
| WebKitGTK 4.1 | pass: native panel and launcher render as layer-shell clients in the DRM session | `webkitgtk41-2.52.5`; panel first document load about 320 ms |
| WebKitGTK 6.0 | available, not installed/tested | package `webkitgtk60-2.52.5` |

Test native Wayland first where available, then document XWayland fallback.
Application-specific failures are not automatically compositor bugs.

## Phase E — Development stack

Record exact versions and results for:

- Rust compiler, Cargo and target triple
- C/C++ toolchain, pkg-config equivalent and linker
- Wayland, libinput, xkbcommon, DRM/KMS, GBM/EGL/GLES libraries
- Smithay dependency resolution and platform-specific compile failures
- workspace crates that compile unchanged
- crates blocked by Linux-only APIs

Installed development baseline (2026-08-19):

- Rust/Cargo `1.94.1`, target `x86_64-unknown-openbsd`, LLVM `20.1.8`
- Git `2.53.0`; base Clang plus packaged LLVM/libclang `20.1.8`
- Wayland `1.24.0`, libinput-openbsd `1.30.2`, xkbcommon `1.13.1`, seatd/libseat
  `0.9.3`, EGL/GBM `25.0.7`, libdrm `2.4.123`, XWayland `24.1.12`
- WebKitGTK 4.1 `2.52.5` and GTK3 `3.24.52`
- `doas pkg_check` completed cleanly after installation (the unprivileged run
  cannot read the intentionally protected D-Bus launch helper)

The WebKit C smoke compiled, linked and returned version `2.52.5`. The later
Meridian hardware run also proved display creation, Wayland layer-shell
integration and interactive panel/launcher rendering. Detailed steady-state
GPU/memory and launcher-start performance budgets remain open.

### WebKit diagnostic runtime

`meridian-ui-runtime` provides the Rust panel and launcher surface proof. Run a
standalone surface from an existing Meridian Wayland session so
`WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR` refer to that session:

```sh
cargo run -p meridian-ui-runtime -- --surface=panel --theme=dark
cargo run -p meridian-ui-runtime -- --surface=launcher --theme=light
```

For the managed integration, start the session with
`MERIDIAN_WEB_UI_PANEL=1`; `meridian-shell` then owns the panel runtime and
launcher lifecycle. Expected evidence:

- the panel is a 66-logical-pixel bottom layer surface in both themes
- the terminal logs `first Panel document load finished in ... ms`
- the panel button opens one Web launcher and a second click closes it
- search, categories and catalogue-approved app activation work
- no HTTP server, file asset lookup or network navigation is involved
- a panel-runtime exit falls back to the native shell panel

Hardware evidence from 2026-08-20: on the 1920x1080 Acer output, the corrected
panel layer is `y=1014, h=66` and its first document load completes in about
320 ms. The 2026-08-21 cold-launch measurement separated 1 ms catalogue work
from 362-469 ms WebKit document startup. Meridian now prewarms one hidden
launcher and controls visibility through a GLib-observed stdin FD. Repeated
toggle-to-layer-map time is 0.18-5.1 ms with no process or document reload and
no hidden-launcher CPU-time increase over a five-second idle sample. Resident
launcher RSS was about 77 MiB for the GTK host, 92 MiB for WebProcess and
51 MiB for NetworkProcess; those figures include multiply counted shared pages
and are an upper-bound signal, not proportional memory.

Native Meridian build matrix on 2026-08-19:

| Crate/path | Result | Evidence / first blocker |
|---|---|---|
| `meridian-tokens` | pass | `cargo check`; design guard test passes |
| `meridian-ipc` | pass | `cargo check` |
| `meridian-config` | pass | `cargo check`, including xkbcommon |
| `meridian-portal` | pass | `cargo check` |
| `meridian-boot-common` | pass | `cargo check` |
| `meridian-compass-render` | pass | `cargo check` |
| `meridian-freetype` / `meridian-ui` | pass | built directly and as shell dependencies |
| `meridian-wm` / `meridian-compositor` | pass | native OpenBSD build succeeds; DRM devices are enumerated from `/dev/dri`, keyboard/pointer input uses wscons directly, and `linux-drm-syncobj-v1` is target-disabled |
| `meridian-shell` | pass | OpenBSD screencopy uses native `shm_mkstemp(3)` with explicit `FD_CLOEXEC`; 310 unit tests and the centralization guard pass |
| `meridian-login` / `meridian-lock` | pass | OpenBSD password authentication uses native `auth_userokay(3)`; PAM remains target-scoped to non-OpenBSD systems; 52 login and 9 lock tests pass |
| `meridian-polkit` | pass | retains PolicyKit's native setuid helper protocol and no longer declares its unused PAM dependency; 11 tests pass |

`cargo check --workspace` now passes on OpenBSD. The Meridian workspace tests
also pass when the excluded vendored Smithay package is omitted explicitly with
`--exclude smithay`. A plain `cargo test --workspace` still asks Cargo to build
Smithay's feature-gated example programs and fails there because Vulkan and the
example-only CLI/image dependencies are intentionally disabled; this is not a
Meridian test failure.

**Link-environment prerequisites (verified 2026-08-20).** OpenBSD packages ship
only versioned shared objects and no unversioned `.so` symlinks, and the linker
does not search `/usr/local/lib` by default. Native test linking (`-lxkbcommon`,
`-lwayland-client`, …) therefore needs both of:

1. Unversioned symlinks next to the versioned libraries (once, via `doas`):
   `/usr/local/lib`: `libxkbcommon.so`, `libwayland-client.so`,
   `libwayland-server.so`, `libwayland-cursor.so`, `libwayland-egl.so`,
   `libseat.so`, `libinput.so`; `/usr/X11R6/lib`: `libfreetype.so`,
   `libfontconfig.so`, `libEGL.so`, `libGLESv2.so`, `libdrm.so`, `libgbm.so`
   — each pointing at the installed versioned file. A package update can remove
   them again; re-check when linking suddenly fails.
2. `LIBRARY_PATH=/usr/local/lib:/usr/X11R6/lib` exported for the cargo run
   (lld honours it at link time without invalidating the compile cache).

**Shell audio backend (implemented 2026-08-20).** OpenBSD has no PipeWire and
no FreeBSD `mixer(8)`; `meridian-shell` drives the kernel mixer through base
`mixerctl(8)` (`audio/mixerctl.rs`): it picks the playback volume control
(`outputs.master`, with fallbacks to `outputs.dac`/`outputs.speaker`/
`outputs.spkr`/`outputs.volume` or the first numeric `outputs.*` control),
scales against the `mixerctl -v` range when printed (azalia(4) on the Acer
uses 0..255 and clamps out-of-range writes), and toggles `<control>.mute`.
Choosing a default output is not supported (audio(4)/sndiod(8) routing).
Runtime prerequisite: the session user must be able to open `/dev/audioN`,
i.e. be in the `_sndiop` group (on the Acer done via
`doas user mod -G wheel,_seatd,_sndiop eduard`; group changes apply at next
login). Without access the snapshot reports "unavailable" gracefully.
Runtime-Verifikat 2026-08-20: ein kontrollierter 25-s-Session-Lauf auf dem
Acer meldete im Shell-Log `openbsd audio backend active via mixerctl:
control=outputs.master volume=Some(49)% muted=false` (das Backend loggt
seinen Zustand genau einmal, damit der Audio-Pfad beobachtbar ist).

Do not paper over failures with broad `cfg` removal. Classify each dependency as
portable core, Linux adapter, OpenBSD adapter or currently unsupported.

## Phase F — Meridian core smoke

Target the smallest useful progression:

1. compile pure/config/IPC crates;
2. start a nested or diagnostic compositor path if available;
3. open the Intel DRM device through the correct OpenBSD ownership model;
4. render one output and cursor;
5. deliver keyboard and pointer input;
6. run a reference XDG client;
7. test XWayland only after native Wayland is understood.

## Phase G — Security experiment

After a process works without sandboxing, record its actual file, device,
process and network needs. Then design the smallest useful `pledge` and `unveil`
profile. Keep privileged helpers separate and narrow. A sandbox profile is not
accepted if it makes normal desktop behavior silently unreliable.

## Decision record

OpenBSD is selected as primary only when the evidence supports acceptable:

- Intel graphics and input reliability
- WLAN/audio/suspend daily-driver behavior
- browser and external app compatibility
- Rust/Smithay maintenance cost
- WebKit runtime availability and performance
- practical privilege separation

If a critical area fails, compare the same requirement on FreeBSD and choose
the more workable platform. The decision and blockers belong in this file.

## Current blocker assessment

1. **WLAN is the first concrete daily-driver blocker.** The internal QCA9377
   has no attached driver in this OpenBSD 7.9 boot. Ethernet is currently the
   only network path.
2. **Bluetooth is also unavailable in the baseline.** Its USB function is only
   exposed through the generic USB driver.
3. **Suspend is unknown, not failed.** `apmd` has not been enabled, so a
   controlled test with SSH/console recovery is required.
4. **The Smithay DRM syncobj compile blocker is resolved natively.** Meridian's
   pinned Smithay port layer does not compile or advertise
   `linux-drm-syncobj-v1` on OpenBSD because its kernel contract requires
   Linux `eventfd`. DRM/KMS, GBM, EGL and implicit client synchronization remain
   available. `meridian-wm` and `meridian-compositor` now pass `cargo check`.
   OpenBSD selects DRM card nodes directly and consumes `wskbd`/`wsmouse`
   records through event-driven calloop sources, so the OpenBSD compositor no
   longer links Smithay's udev or libinput backends. OpenBSD libdrm's weak
   `priv_open_device` hook is exported by the Meridian binary and routed through
   the existing seatd session with a `/dev/dri/card*` and `/dev/dri/renderD*`
   allowlist; neither a root compositor nor relaxed device permissions are
   required. All 381 compositor library
   tests pass. A controlled 15-second hardware run opened both wscons devices,
   selected `Mesa Intel(R) HD Graphics 620 (KBL GT2)`, exposed 99 DMA-BUF
   formats, initialized EGL 1.5/GBM at 1920x1080@60 Hz, completed the initial
   atomic KMS commit and brought XWayland up. Keyboard key mapping, pointer
   direction and scrolling still need an interactive run. This proves the
   accelerated first frame, not sustained performance. The Smithay boundary
   should be proposed upstream and the vendored source removed when accepted.
5. **The shell shared-memory compile blocker is resolved natively.** Linux and
   existing targets keep `memfd_create`; OpenBSD uses its documented
   `shm_mkstemp(3)` facility and sets `FD_CLOEXEC` before exposing the descriptor
   to Wayland. The OpenBSD kernel test proves that the descriptor is resizable
   and close-on-exec. A controlled 20-second full-session run started
   `meridian-shell`, authenticated its IPC connection, mapped the desktop and
   panel surfaces, and configured the 1920x66 panel plus 880x620 launcher. An
   end-to-end screenshot still needs an interactive run. The same smoke also
   exposed the next session-integration gap: no D-Bus session bus was present,
   so notification and status-notifier services disabled themselves cleanly.
6. **The authentication compile boundary is resolved natively.** Login and lock
   use OpenBSD `auth_userokay(3)` with the user's configured default BSD
   Authentication style; their PAM implementation and dependency remain intact
   on non-OpenBSD targets. Smartcard login reports an explicit unsupported
   configuration on OpenBSD instead of silently falling back. Polkit already
   delegates authorization to `polkit-agent-helper-1`, so its unused direct PAM
   dependency was removed. The successful-password login/session lifecycle and
   an actual unlock still require an interactive test; no password was supplied
   to automated checks. The launcher uses a private, ownership-checked
   `/tmp/meridian-runtime-<uid>` directory and includes `/usr/X11R6/bin` in the
   sanitized OpenBSD session `PATH`, so XWayland remains discoverable.
7. **Graphics/WebKit runtime proof is complete for the first panel/launcher
   slice.** EGL/GBM and WebKit compile/link successfully; the Intel-accelerated
   compositor renders real WebKit layer-shell clients and panel/launcher input
   is interactive. Launcher process reuse removes its measured cold-open delay;
   proportional-memory/idle-GPU budgets and the broader external-client matrix
   remain pending.
8. **The first complete shell slice is interactively usable.** Panel, launcher,
   Quick Settings and Settings navigation were exercised together in dark and
   light themes. Audio state, slider, mute and wscons hardware keys work on the
   Acer. Persistent hidden popups no longer intercept pointer or keyboard
   input. Normal Wayland/XWayland windows now start centered in the panel-safe
   workarea. Blender and FreeCAD now keep their splash screens undecorated and
   show the Meridian frame on their maximized main windows. This was
   interactively accepted on the Acer on 2026-08-22.
9. **Qt 6 application launch currently defaults to XCB/XWayland.** A controlled
   2026-08-22 comparison showed FreeCAD's native Wayland path stopping after
   the splash with `QOpenGLWidget`/QRhi context creation failures, while the
   identical desktop command under `QT_QPA_PLATFORM=xcb` created its complete
   main window without those errors. Meridian applies this default only to
   OpenBSD-launched applications and respects an explicit session override.
   Re-evaluate it when Qt Wayland OpenGL works on the reference stack. A second
   controlled run on 2026-08-22 narrowed the remaining performance risk:
   FreeCAD maps its normal main window and stays responsive as a process, but
   The secure XWayland server path was completed on 2026-08-22 without changing
   DRM-node permissions. Meridian opens exactly the active primary and render
   nodes through seatd and passes only those descriptors to the still
   unprivileged XWayland process. The OpenBSD bridge validates the canonical
   device path and `st_rdev`; Xwayland initializes Glamor and exposes DRI3. An
   experimental client-side preload also made `glxinfo -B` report the Mesa
   Intel HD 620 with `Accelerated: yes`, but was rolled back immediately after
   the hardware test: FreeCAD no longer opened and Blender crashed when leaving
   maximized mode. X11 applications therefore retain the stable software client
   path for now. The bridge remains required beside `meridian` for Xwayland's
   own Glamor initialization, and the local installer handles that artifact.
10. **Thunar uses a tightly scoped XWayland SSD compatibility path.** Its
   native GTK3 Wayland backend does not negotiate `xdg-decoration`, and
   `GTK_CSD=0` alone therefore cannot hand the frame to Meridian. The launcher
   sets `GDK_BACKEND=x11` and `GTK_CSD=0` only for the Thunar executable. This
   keeps every other GTK application on its normal native Wayland path. The
   resulting frame, maximize workarea, square maximized corners and drag-restore
   behavior were interactively accepted on the Acer on 2026-08-22.
11. **The compositor-owned SSD frame is visually accepted in both themes.**
   Geometry and effects come from `meridian-tokens` plus
   `meridian-config::Decorations`; the light/dark switch changes only colors.
   The decoration icon cache includes the resolved tint in its key, preventing
   stale low-contrast glyphs after a theme switch. Hover assets remain cached
   and no idle work was added.
