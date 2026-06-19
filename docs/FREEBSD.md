# Meridian on FreeBSD

Meridian builds and runs as a full desktop on FreeBSD (verified on 15.1-RELEASE,
amd64, Intel KMS): boot → login manager → PAM login → compositor + shell +
panel + launcher + apps. FreeBSD has no systemd or logind, so the systemd units,
`pam_systemd`/logind PAM stacks, and `install-local.sh` boot wiring in
[INSTALL.md](../INSTALL.md) do **not** apply — the compositor seats itself
through libseat/seatd directly.

## Quick start (plug and play)

Three commands from a fresh FreeBSD install to a desktop that boots itself:

```sh
scripts/install-deps-freebsd.sh all
scripts/install-freebsd.sh --build --enable-boot --user youruser
reboot
```

The first command installs every dependency (toolchain, libraries, GPU driver,
session bus, icon theme, cursor, fonts, and the default apps). The second builds
the release binaries, installs them with their themes/PAM/rc.d services,
enables the prerequisites (`seatd`, `dbus`), detects and persists the GPU KMS
module, adds the user to the `video`/`operator` groups, and enables the login
manager at boot.

> **Keep an SSH session open the first time you enable boot.** Starting the
> desktop takes over the GPU and console. Recover with `service meridian_login
> stop`. Verify recovery before you reboot.

That is the whole install. The sections below explain what each step does and
how to do it by hand.

## 1. Dependencies

```sh
scripts/install-deps-freebsd.sh all          # build + runtime + apps
```

Sub-targets: `build` (toolchain + headers), `runtime` (libraries, `drm-kmod`,
`dbus`, `papirus-icon-theme`, `plasma6-breeze` cursor, fonts, Xwayland,
`xdg-utils`), `apps` (`foot`, `pcmanfm`, `firefox`), `hardware-test` (DRM/PCI/USB
inspection tools).

`papirus-icon-theme` matches Meridian's default icon theme (`Papirus-Dark`) and
`plasma6-breeze` provides the default `Breeze_Light` Xcursor, so the shipped
defaults resolve out of the box with no config edits. `plasma6-breeze` pulls KDE
dependencies — if you want a lean install, drop it from `runtime_pkgs` and
Meridian falls back to its embedded cursor.

## 2. Install

```sh
scripts/install-freebsd.sh --build --enable-boot --user youruser
```

Options:

| Flag | Effect |
| --- | --- |
| `--build` | `cargo build --release --workspace` first (sets `LIBRARY_PATH=/usr/local/lib` so the linker finds pkg libraries) |
| `--enable-boot` | enable `meridian_login` at boot, switch to `background_dhclient`, load the GPU module now |
| `--quiet-boot` | also enable console muting for a silent boot (best-effort) |
| `--user NAME` | desktop user — owns `/var/lib/meridian` and becomes `meridian_user` (default: `$SUDO_USER`) |
| `--gpu auto\|intel\|amd\|none` | KMS module; `auto` reads the PCI vendor (default) |
| `--prefix PATH` | install prefix (default `/usr/local`) |

It installs binaries to `/usr/local/bin`, themes to
`/usr/local/share/meridian/themes`, PAM stacks to `/etc/pam.d/meridian-login*`,
and the rc.d services to `/usr/local/etc/rc.d/`. It always enables and starts
`seatd` + `dbus` (harmless prerequisites) and persists the GPU module in
`kld_list`. Without `--enable-boot` nothing that affects the next boot is
touched — test ad-hoc with `service meridian_login onestart`.

**Release builds matter.** The compositor and shell do heavy CPU work (scene and
damage management, tiny-skia panel/launcher rendering); a debug build is 10–50×
slower and feels janky. The installer always builds `--release`.

## 3. What the install configures

Equivalent manual steps, for reference or a custom setup:

```sh
# Prerequisites (the compositor seats itself via libseat/seatd, needs a bus)
sysrc seatd_enable=YES && service seatd start
sysrc dbus_enable=YES  && service dbus  start
pw groupmod video    -m youruser     # open /dev/dri/cardN; seatd socket is group video
pw groupmod operator -m youruser

# GPU KMS module (Intel shown; AMD: amdgpu / radeonkms)
sysrc kld_list+=i915kms && kldload i915kms
# After kldload, /dev/dri/card0 and /dev/dri/renderD128 should exist.

# Boot-to-greeter: enable the login manager, disable the direct-compositor
# service (they are mutually exclusive), and background dhclient so networking
# does not block / spam the boot.
sysrc meridian_login_enable=YES
sysrc meridian_enable=NO
sysrc background_dhclient=YES
```

The `meridian-login` rc.d service authenticates the user through PAM
(`/etc/pam.d/meridian-login-password`) and spawns the compositor as that user.
It sets `XKB_DEFAULT_RULES=evdev` and `XCURSOR_PATH`, aliases `/run`→`/var/run`
(the login IPC socket lives at `/run/meridian-login.sock`), and the compositor
it spawns wraps itself in `dbus-run-session` for the per-session bus.

### Why these specific knobs

- **`XKB_DEFAULT_RULES=evdev`** — libinput feeds evdev keycodes; only the evdev
  ruleset maps them to the right keysyms, including the multimedia/volume keys.
  The `xorg`/xfree86 ruleset silently drops the volume keys.
- **PAM `session` uses `pam_permit`** — FreeBSD's `pam_unix` has no session
  module (unlike Linux-PAM); the stacks in `packaging/pam/freebsd/` reflect this.
- **`XDG_DATA_DIRS` includes `/usr/local/share`** — pkg installs `.desktop`
  files and icon themes there, not just `/usr/share`. The compositor sets this
  for the session; without it the launcher (which hides icon-less apps) is empty.
- **Session D-Bus via `dbus-run-session`** — FreeBSD has no systemd user bus, so
  GTK/Qt apps and Meridian's own notification/status-notifier services need one.

## 4. Silent boot (optional)

A console-free boot straight into the desktop. In `/boot/loader.conf`:

```
autoboot_delay="-1"      # no boot-menu countdown
beastie_disable="YES"    # no boot menu
boot_mute="YES"          # mute kernel console during boot
i915kms_load="YES"       # bring KMS up early for a clean logo->desktop handoff
```

Then silence the rest of `rc` (some driver/`devmatch`/`dhclient` messages slip
past `boot_mute`):

```sh
sysrc rc_startmsgs=NO
scripts/install-freebsd.sh --quiet-boot ...   # or: sysrc meridian_quiet_enable=YES
```

`rc.d/meridian_quiet` runs `conscontrol mute on` before `devmatch`/`NETWORKING`.
Recover console output over SSH with `conscontrol mute off`. This is still rough:
the `getty` login prompt on `ttyv0` can flash up before the greeter takes over.

A PlayStation-style animated splash at the firmware/logo stage is **not**
achievable on a stock PC — the EFI firmware and FreeBSD loader own that stage and
there is no GPU/KMS yet. A silent boot to the compositor's first frame is the
better PC path; the sibling `bootsplash` repo is intentionally left out of the
FreeBSD install.

## 5. Known gaps on FreeBSD

- **Chromium** does not start — its sandbox/GPU broker is broken on FreeBSD
  (a Chromium issue, not Meridian). `firefox` is the working default browser.
- **Greeter cursor lag** — the login screen redraws the full frame on the CPU on
  every mouse move; smooth but slightly laggy. (Caching the static layer and
  redrawing only the cursor region is the fix.)
- **Silent boot** is incomplete (see §4): the `ttyv0` getty can appear briefly.
- A few web/folder launcher shortcuts have no resolved icon.

## 6. Troubleshooting

| Symptom | Cause / fix |
| --- | --- |
| Greeter never appears, screen stays on console | GPU module not loaded — `kldload i915kms`, check `/dev/dri/card0` exists |
| Greeter is janky / unusably slow | debug build deployed — reinstall with `--build` (release) |
| Launcher empty, no icons | `papirus-icon-theme` not installed, or `XDG_DATA_DIRS` missing `/usr/local/share` |
| Volume keys do nothing | `XKB_DEFAULT_RULES` not `evdev` (the rc.d service sets it) |
| Apps don't launch / no notifications | no session bus — `dbus` not installed, or `dbus-run-session` missing |
| Login fails with PAM error | wrong PAM stack — `session` must use `pam_permit` (no `pam_unix` session on FreeBSD) |
| `permission denied` on `/dev/dri/cardN` | user not in `video` group — `pw groupmod video -m youruser`, re-login |

Logs: `/var/log/meridian-login.log` (greeter + compositor), `/var/log/meridian.log`
(direct-compositor service).
