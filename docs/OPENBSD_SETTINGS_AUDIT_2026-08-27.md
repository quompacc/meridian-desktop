# OpenBSD Settings audit — 2026-08-27

Reference host: Acer Aspire F5-573G, OpenBSD 7.9/amd64.

## Capability matrix

| Settings area | OpenBSD status | Required follow-up |
|---|---|---|
| Theme | Native Meridian config works | Keep toolkit export best-effort and platform-neutral |
| Wallpaper | Native config and cached thumbnails work | Visual QA only |
| Cursor | Native config and installed theme discovery work | Verify toolkit export |
| Pinned apps | Native Meridian config works | Visual QA only |
| Default apps | `xdg-mime` is installed | Verify every MIME write and fallback |
| Displays | Typed compositor IPC is platform-neutral | Hardware-test mode, scale, rotation and primary output |
| System overview | OpenBSD `sysctl` backend works | Add `kern.boottime`; never fall through to `/proc` semantics |
| Network | Wired read-only state from `ifconfig -a` works | Split OpenBSD from FreeBSD; remove `wlan*`, `ifconfig -l` and `wpa_cli` assumptions |
| Bluetooth | `bluetoothctl` is absent | Add an explicit OpenBSD unsupported capability instead of probing BlueZ |
| Audio | Native `mixerctl` backend works | Replace remaining PipeWire-only copy and verify device labels |
| Printers | CUPS `lpstat` is absent on the reference host | Present CUPS as an optional unavailable service, not an error |
| Energy/session | **Regression: launcher/system shutdown currently does not execute** | Re-audit pointer dispatch, confirmation state and ConsoleKit IPC end to end before treating any system action as working |
| Users | `/etc/passwd` read-only view works | Verify OpenBSD account classification and current-session identity |
| Updates | Linux-only `apt` backend is wrong | Add OpenBSD read-only status via `syspatch`, `pkg_info` and `fw_update` boundaries |

## Reference-host evidence

- Network devices currently visible: `lo0`, `re0`, `enc0`, `pflog0`; `re0` is active.
- `/sbin/ifconfig`, `/usr/bin/mixerctl`, `/usr/local/bin/xdg-mime`,
  `/usr/sbin/pkg_info`, `/usr/sbin/syspatch`, and `/usr/sbin/fw_update` exist.
- `wpa_cli`, `bluetoothctl`, `lpstat`, and `apt` do not exist.

## Implementation order

1. Finish and visually approve the native WebKit-reference hierarchy.
2. Introduce explicit per-platform capability reporting in Settings data.
3. Split the current non-Linux network module into real OpenBSD and FreeBSD
   backends before exposing Wi-Fi mutations.
4. Replace the updates probe with platform-specific read-only backends.
5. Make Bluetooth and printing show an honest optional/unavailable state.
6. Re-test every visible control on the OpenBSD reference hardware.

No unsupported control may silently run a Linux command, and no missing
optional service may be presented as broken hardware.

## Native Settings checkpoint

The current working tree contains the native Settings visual overhaul based on
the Meridian design manifest rather than the archived WebKit implementation:

- opaque launcher/settings surfaces using the central `meridian-tokens` palette;
- a full-height navigation sidebar with grouped categories;
- page headings and persistent search in the content column;
- shared native group cards across all Settings categories;
- revised appearance, wallpaper and display layouts;
- display identity, mode and control geometry centralized in
  `meridian-tokens::Settings`;
- the wallpaper picker forced onto the Wayland GDK backend and its extensionless
  helper kept as LF on Windows checkouts.

The wallpaper picker was manually confirmed working on the reference host. The
visual hierarchy was judged substantially improved, but the checkpoint is not a
finished or releasable Settings implementation.

## Known regressions at this checkpoint

Manual hardware testing after the visual overhaul still shows two blocking
regressions:

1. The launcher and Settings interaction feel globally slow. Category hover is
   especially delayed because pointer-state changes currently trigger a full
   Settings render, including widget-tree construction and layout.
2. The launcher/system power buttons are not reliable. Shutdown was manually
   tested on the reference host and did not execute. The other system actions
   must be considered unverified until the complete input -> widget action ->
   confirmation -> privileged ConsoleKit helper path has been traced again.

Do not describe this checkpoint as performance-complete and do not infer that
the power actions work merely because their unit tests or earlier greeter tests
pass.

## Performance experiment history

Several uncommitted attempts were evaluated on the OpenBSD machine and then
removed because they either failed to improve latency or introduced wider
regressions:

- omitting the active page content while hit-testing the sidebar;
- suppressing intermediate press/release renders for category selection;
- repainting only old/new sidebar rows after still rebuilding the widget tree
  and Taffy layout;
- bypassing the widget tree with independently calculated sidebar hit geometry;
- using a cached launcher frame and a dedicated partial-update Wayland buffer.

The direct-hit and partial-buffer variants caused missing hover feedback and a
slower launcher. The repository checkpoint intentionally contains none of these
performance experiments; it is back on the straightforward full widget/render
path so the next diagnosis starts from one understandable call flow.

## Verification of the recorded checkpoint

The exact checkpoint was copied to `/home/eduard/meridian-build`, built and run
on the OpenBSD 7.9 reference host. The following automated checks passed:

```text
cargo fmt --all -- --check
cargo check --workspace
env LIBRARY_PATH=/usr/local/lib:/usr/X11R6/lib cargo test --workspace --exclude smithay
cargo test -p meridian-tokens --test design_guard --test source_size_guard
```

These checks establish build and test health only. They do not override the
manual findings that runtime performance is poor and shutdown is broken.
