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

The linker does not search `/usr/local/lib` by default, and libxkbcommon needs
the xorg ruleset to compile a keymap:

```sh
export LIBRARY_PATH=/usr/local/lib
export XKB_DEFAULT_RULES=xorg
cargo build --release --workspace
cargo test --workspace          # 939 tests
```

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

## Known gaps on FreeBSD

- **No boot-to-greeter.** `meridian-login` still assumes `pam_systemd`/logind for
  session/seat setup; the rc.d service launches the compositor directly instead
  of going through the greeter. Porting the greeter is the remaining boot work.
- **Bootsplash** (the sibling repo) is systemd-only and not ported.
- **Input/seat** rely on FreeBSD `evdev` (`/dev/input/event*`) and `seatd`; there
  is no `logind` fallback.
