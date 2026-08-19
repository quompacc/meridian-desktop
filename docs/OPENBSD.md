# Meridian on OpenBSD — Evaluation Guide

> **STATUS: BASE SYSTEM PATCHED, DEVELOPMENT STACK PROBED.** Hardware was
> inventoried and the first native build matrix was run over SSH on 2026-08-19.
> OpenBSD support is not yet claimed: the portable Meridian crates compile, but
> compositor, shell and authentication paths have concrete OS/API blockers.

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
| accelerated graphics | partial | Intel DRM nodes and EGL/GBM 25.0.7 are present; native link probes pass, but no rendered frame/session yet |
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
| WebKitGTK 4.1 | compile/link smoke passed, runtime surface pending | `webkitgtk41-2.52.5`; reports 2.52.5 through its C API |
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

The WebKit C smoke compiled, linked and returned version `2.52.5`. This proves
headers, pkg-config metadata, linker and loader availability; it does not yet
prove display creation, Wayland integration, GPU acceleration or performance.

Native Meridian build matrix from Git revision `664b5ba`:

| Crate/path | Result | Evidence / first blocker |
|---|---|---|
| `meridian-tokens` | pass | `cargo check`; design guard test passes |
| `meridian-ipc` | pass | `cargo check` |
| `meridian-config` | pass | `cargo check`, including xkbcommon |
| `meridian-portal` | pass | `cargo check` |
| `meridian-boot-common` | pass | `cargo check` |
| `meridian-compass-render` | pass | `cargo check` |
| `meridian-freetype` / `meridian-ui` | pass | built directly and as shell dependencies |
| `meridian-wm` / `meridian-compositor` | pass | native OpenBSD build succeeds; `linux-drm-syncobj-v1` is target-disabled while DRM/KMS, GBM and EGL remain enabled |
| `meridian-shell` | blocked | screencopy uses Linux `libc::memfd_create` and `MFD_CLOEXEC` |
| `meridian-login` / `meridian-lock` / `meridian-polkit` | blocked | `pam-sys` requires `security/pam_appl.h`; OpenBSD uses BSD Authentication rather than a native PAM base |

LLVM/libclang was installed to distinguish a missing bindgen tool from the
actual authentication blocker. With `LIBCLANG_PATH` and `LD_LIBRARY_PATH` set,
the PAM builds advance to the missing PAM header and fail there as expected.

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
   available. `meridian-wm` and `meridian-compositor` now pass `cargo check`,
   and all 378 compositor library tests pass on OpenBSD. The boundary should be
   proposed upstream and the vendored source removed when accepted.
5. **Shell screencopy has a direct Linux memory-file assumption.** Replace or
   isolate `memfd_create` with an OpenBSD-capable shared-memory abstraction;
   do not merely remove the screencopy path silently.
6. **Authentication needs an OpenBSD adapter.** Login, lock and polkit currently
   depend on PAM/pam_systemd semantics. The OpenBSD path should be designed
   around BSD Authentication and native session/process handling.
7. **Graphics/WebKit packaging is positive but runtime proof is incomplete.**
   EGL/GBM and WebKit compile/link successfully. A real Wayland surface,
   accelerated frame, cursor/input path and WebKit render remain pending behind
   the compositor/runtime work.
