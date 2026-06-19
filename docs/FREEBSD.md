# Meridian on FreeBSD

Meridian builds and runs on FreeBSD (verified on 15.1-RELEASE, amd64, Intel KMS).
FreeBSD has no systemd or logind, so the systemd units, `pam_systemd` PAM stacks,
and `install-local.sh` boot wiring described in [INSTALL.md](../INSTALL.md) do
**not** apply. The compositor seats itself through libseat/seatd directly and
does not need logind.

## 1. Dependencies

```sh
scripts/install-deps-freebsd.sh all
```

This installs the build toolchain (`rust`, `pkgconf`, Mesa, Wayland,
libxkbcommon, libinput, `seatd`, `libudev-devd`, libdrm, pixman, freetype2,
fontconfig) plus runtime extras (`dbus`, `drm-kmod`, a Bibata Xcursor theme,
fonts, Xwayland).

## 2. Build

The linker does not search `/usr/local/lib` by default:

```sh
export LIBRARY_PATH=/usr/local/lib
cargo build --release --workspace
cargo test --workspace          # tests
```

At **runtime** the compositor needs `XKB_DEFAULT_RULES=evdev` (the rc.d service
sets this): libinput feeds evdev keycodes, and only the evdev ruleset maps them
to the right keysyms — including the multimedia/volume keys. The `xorg` ruleset
uses the old xfree86 keycodes and silently drops the volume keys.

## 3. Runtime services and graphics

```sh
sysrc seatd_enable=YES  && service seatd start
sysrc dbus_enable=YES   && service dbus start
sysrc kld_list+=i915kms && kldload i915kms     # Intel; AMD: amdgpu/radeonkms
```

After `kldload`, `/dev/dri/card0` and `/dev/dri/renderD128` should exist. A
non-root desktop user must be in the `video` group (the seatd socket is
`group video`):

```sh
pw groupmod video -m youruser
```

## 4. Install

```sh
scripts/install-freebsd.sh --build
```

Installs the binaries to `/usr/local/bin`, themes to
`/usr/local/share/meridian/themes`, and the rc.d service to
`/usr/local/etc/rc.d/meridian` (not enabled).

## 5. Run a session

Verify SSH recovery first — starting a session takes over the GPU and console.

Ad-hoc (ignores `rc.conf`):

```sh
service meridian onestart
service meridian onestop        # recover over SSH if needed
```

At boot:

```sh
sysrc meridian_enable=YES
sysrc meridian_user=youruser    # in the "video" group
```

Cursor: the default `Breeze_Light` is not packaged on FreeBSD. Point Meridian at
an installed theme in `~/.config/meridian/config.toml`:

```toml
[cursor]
theme = "Bibata-Modern-Classic"
size = 24
```

## Silent boot

For a console-free boot straight into the desktop (`/boot/loader.conf`):

```
autoboot_delay="-1"      # no boot-menu countdown
beastie_disable="YES"    # no boot menu
boot_mute="YES"          # mute kernel console during boot
i915kms_load="YES"       # bring KMS up early for a clean logo->desktop handoff
```

Then mute the console for the rest of boot (occasional driver messages slip past
`boot_mute`) via the bundled rc.d service, and silence rc start messages:

```sh
sysrc rc_startmsgs=NO
sysrc meridian_quiet_enable=YES   # rc.d/meridian_quiet: conscontrol mute on
```

Recover console output over SSH with `conscontrol mute off`.

Note on a boot splash: an animated, GPU-accelerated splash at the firmware/logo
stage (PlayStation-style) is not achievable on a stock PC — the EFI firmware and
FreeBSD loader own that stage and there is no GPU/KMS yet. The bundled DRM
`bootsplash` can only run from `rc` (after KMS), so it appears late and renders
on the CPU; a silent boot to the compositor's first frame is the better PC path.

## Known gaps on FreeBSD

- **No boot-to-greeter.** `meridian-login` still assumes `pam_systemd`/logind for
  session/seat setup; the rc.d service launches the compositor directly instead
  of going through the greeter. Porting the greeter is the remaining boot work.
- **Bootsplash** (the sibling repo) is systemd-only and not ported.
- **Input/seat** rely on FreeBSD `evdev` (`/dev/input/event*`) and `seatd`; there
  is no `logind` fallback.
