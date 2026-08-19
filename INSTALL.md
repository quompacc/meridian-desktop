# Meridian - Installation

This guide documents the **currently implemented Linux installation path**. It
does not decide Meridian's long-term operating system. OpenBSD is the next
real-hardware evaluation target; FreeBSD remains the existing BSD install path.

The WebKit UI runtime described in the active plan is not installed by these
steps yet. They deploy the current native Rust shell.

> **FreeBSD?** FreeBSD has no systemd/logind and uses its own turnkey installer —
> see [docs/FREEBSD.md](docs/FREEBSD.md). The steps below do not apply there.

> **OpenBSD?** Support is not yet claimed and there is no installer. Follow the
> evidence-first checklist in [docs/OPENBSD.md](docs/OPENBSD.md); do not adapt
> Linux commands blindly.

For the full boot experience keep the sibling checkouts next to each other:

```text
~/bootsplash
~/meridian-desktop
```

`bootsplash` owns the early DRM splash and hands over to `meridian-login`.
`meridian-desktop` owns the login manager, compositor, shell, lock screen,
portal backend, polkit agent, PAM files, themes, and install metadata.

## 1. Install dependencies on Arch

From the Meridian checkout:

```bash
cd ~/meridian-desktop
scripts/install-deps.sh --manager pacman all
rustup default stable
```

The helper also auto-detects `pacman`, so this is equivalent on a normal Arch
host:

```bash
scripts/install-deps.sh all
```

Dependency modes:

```bash
scripts/install-deps.sh build
scripts/install-deps.sh runtime
scripts/install-deps.sh hardware-test
```

What the dependency sets cover:

- `build`: `base-devel`, `pkgconf`, Rust via `rustup`, PAM, libseat/logind,
  Wayland, libinput, EGL/GLES/GBM/DRM, font and pixman development libraries.
- `runtime`: D-Bus, NetworkManager, Breeze cursor theme, fonts/xkb data,
  Python GTK3 for `meridian-file-picker`, xdg-desktop-portal, polkit, pam_u2f,
  CUPS client tools, PipeWire/WirePlumber, and XWayland.
- `hardware-test`: libinput diagnostics, DRM/PCI/USB inspection tools,
  `mesa-utils`, `notify-send`, and `jq`.

On Debian/apt systems use the same script with `--manager apt` or auto-detect:

```bash
scripts/install-deps.sh --manager apt all
```

## 2. Build and install Meridian

The normal local install path is:

```bash
cd ~/meridian-desktop
scripts/install-local.sh --build
```

To also install the sibling bootsplash checkout:

```bash
scripts/install-local.sh --build --bootsplash ../bootsplash
```

This installs:

- binaries: `meridian`, `meridian-shell`, `meridian-login`, `meridian-lock`,
  `meridian-portal`, `meridian-polkit-agent`, `meridian-file-picker`
- PAM: `meridian-login`, `meridian-login-password`
- themes: `${prefix}/share/meridian/themes`
- portal metadata: D-Bus service, systemd user unit, `.portal`,
  `meridian-portals.conf`
- polkit autostart: `/etc/xdg/autostart/meridian-polkit-agent.desktop`
- login service: `/etc/systemd/system/meridian-login.service`
- appearance state dir: `/var/lib/meridian`, owned by the desktop user

Default prefix is `/usr/local`. Override only if the corresponding XDG and D-Bus
search paths on the target system include that prefix:

```bash
scripts/install-local.sh --build --prefix /usr/local
```

The script does not enable the boot login service by default. First verify SSH
or another recovery route.

## 3. Prepare recovery before taking over tty1

Enable a regular getty on tty2 and verify SSH:

```bash
sudo systemctl enable --now getty@tty2.service
systemctl status getty@tty2.service --no-pager
ssh <host> true
```

Verify installed files:

```bash
command -v meridian meridian-shell meridian-login meridian-lock meridian-portal meridian-polkit-agent
command -v meridian-file-picker
if command -v bootsplash >/dev/null; then bootsplash --help >/dev/null || true; fi
test -f /etc/pam.d/meridian-login
test -f /etc/pam.d/meridian-login-password
test -d /usr/local/share/meridian/themes
test -f /usr/local/share/xdg-desktop-portal/portals/meridian.portal
test -f /usr/local/share/dbus-1/services/org.freedesktop.impl.portal.desktop.meridian.service
```

## 4. Enable boot login

Without bootsplash:

```bash
cd ~/meridian-desktop
scripts/install-local.sh --enable-boot
sudo reboot
```

With bootsplash:

```bash
cd ~/meridian-desktop
scripts/install-local.sh --enable-boot --bootsplash ../bootsplash
sudo reboot
```

The boot chain is:

- `bootsplash.service` starts early, opens the DRM card, and listens on
  `/run/bootsplash.sock`.
- `meridian-login.service` replaces `getty@tty1`, authenticates through PAM,
  opens a logind session, and starts `meridian` as the authenticated user.
- `meridian` starts `meridian-shell`; the shell autostarts the polkit agent and
  apps from XDG autostart directories.
- `meridian-portal` is activated by xdg-desktop-portal through the installed
  portal metadata.

Recovery if login fails: `Ctrl+Alt+F2` should bring up a regular getty on tty2.

## 5. Post-boot checks

After the first reboot:

```bash
systemctl --failed --no-pager
systemctl status --no-pager bootsplash.service meridian-login.service
sudo journalctl -b -u bootsplash.service -u meridian-login.service --no-pager
pgrep -a meridian
```

Inside the logged-in Meridian session, or over SSH with the user bus exported:

```bash
busctl --user list | grep -E "xdg|portal|meridian" || true
busctl --user introspect org.freedesktop.impl.portal.desktop.meridian /org/freedesktop/portal/desktop --no-pager
```

For the first controlled hardware pass, follow `docs/HARDWARE_SMOKE.md`.

## 6. NetworkManager

The panel network tray queries `nmcli`, so NetworkManager must manage the active
interface.

On Arch, enable NetworkManager if it is not already active:

```bash
sudo systemctl enable --now NetworkManager.service
nmcli general status
```

If another network stack owns the interface, migrate deliberately and keep SSH
recovery available. For a simple wired interface:

```bash
ip link
sudo nmcli connection add type ethernet ifname enp1s0 con-name Wired autoconnect yes
sudo nmcli connection up Wired
```

Adjust `enp1s0` to the real interface name.

## 7. Cursor theme

Meridian defaults to `Breeze_Light` at size `24`. The Arch runtime dependency
installs Breeze. To make it explicit:

```toml
# ~/.config/meridian/config.toml
[cursor]
theme = "Breeze_Light"
size = 24
```

## Build-only systems

If you only need to compile and run tests:

```bash
scripts/install-deps.sh build
cargo build --workspace
cargo test --workspace
```
