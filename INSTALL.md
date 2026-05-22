# Meridian — Installation

Tested on Debian 13 (Trixie) with virtio-gpu. Should adapt straightforwardly
to other systemd + Wayland-capable distros; package names vary.

For day-to-day project status and history see `README.md` and `ROADMAP.md`.

## 1. Build-time dependencies (apt)

```bash
sudo apt install -y \
    build-essential pkg-config \
    libpam0g-dev libclang-dev \
    libseat-dev libudev-dev \
    libfontconfig-dev libfreetype-dev \
    libpixman-1-dev \
    libwayland-dev libxkbcommon-dev libinput-dev \
    libegl-dev libgles-dev \
    libgbm-dev libdrm-dev
```

Plus a Rust toolchain — install via [rustup](https://rustup.rs/) and stick
to the version pinned by `rust-toolchain.toml`.

## 2. Runtime dependencies (apt)

```bash
sudo apt install -y \
    network-manager \
    dmz-cursor-theme \
    xkb-data \
    fonts-dejavu fonts-noto-core \
    xdg-utils
```

What each one is for:

- **network-manager** — the panel tray queries `nmcli`. Without it the tray
  is permanently "disconnected" even when the host is online. On Debian the
  out-of-the-box network manager is `systemd-networkd`; the install below
  switches to NetworkManager so the tray works. SSH stays up across the
  switch as long as you create the connection profile *before* disabling
  networkd (the install steps below do this).
- **dmz-cursor-theme** — provides the `DMZ-White` and `DMZ-Black` cursor
  themes. Meridian's default cursor theme is `default` which always exists,
  but DMZ is the polish we recommend. See "Cursor theme" below.
- **xkb-data** — keyboard layout databases for `xkbcommon`. Usually already
  installed by another package but explicit here so a minimal install does
  not stall on missing layouts.
- **fonts-dejavu, fonts-noto-core** — the in-binary fonts already cover the
  splash + shell text rendering; these are for the fontconfig fallback path
  used by `crates/meridian-shell/src/draw/text.rs` for non-bundled glyphs.
- **xdg-utils** — `xdg-open` for the launcher's "open with default app"
  action.

## 3. Build

```bash
cd /path/to/meridian-desktop
cargo build --release --workspace
```

`cargo build -p meridian --release` alone is *not* enough — it does not pull
in `meridian-shell` or `meridian-login`. Always use `--workspace` (or
`scripts/smoke-drm.sh`, which does the right thing).

The compositor will look for `meridian-shell` next to its own binary
(`target/release/`), so building the workspace is enough — no install step
needed for the shell binary to be picked up.

## 4. Install login service

```bash
sudo cp crates/meridian-login/config/meridian-login.service \
        /etc/systemd/system/meridian-login.service
sudo cp crates/meridian-login/config/meridian-login.pam \
        /etc/pam.d/meridian-login
sudo systemctl daemon-reload
sudo systemctl disable getty@tty1.service
sudo systemctl enable meridian-login.service
```

The unit:
- runs on `tty1`, replaces `getty@tty1`
- spawns `meridian` (the compositor) as the authenticated user after PAM
- IPC-hands over to the compositor and the shell

Recovery if `meridian-login.service` ever fails to start: `Ctrl+Alt+F2`
brings up a regular getty on `tty2`.

## 5. Network: switch from systemd-networkd to NetworkManager

The panel tray needs NetworkManager. On a fresh Debian:

```bash
# Pre-create an NM ethernet profile so the interface stays up across the
# switchover. Adjust the interface name (find with `ip link`).
sudo nmcli connection add type ethernet ifname enp1s0 con-name Wired autoconnect yes

# Force NM to manage the device (override any "unmanaged by udev" rule).
sudo tee /etc/NetworkManager/conf.d/10-manage-enp1s0.conf <<'EOF'
[device]
match-device=interface-name:enp1s0
managed=true
EOF

# Stop and mask networkd so it doesn't fight NM after reboot.
sudo systemctl mask systemd-networkd.service systemd-networkd.socket

# Reload NM and bring up the connection.
sudo systemctl reload NetworkManager
sudo nmcli device set enp1s0 managed yes
sudo nmcli connection up Wired
```

Verify: `nmcli general status` should report `connected` (possibly with a
suffix like `(local only)` which Meridian now handles correctly).

## 6. Cursor theme

Meridian defaults to `default` (always exists). For the DMZ arrow look from
the package above:

```bash
# Either pick DMZ-White (Debian package name) by editing your config:
#   ~/.config/meridian/config.toml
#   [cursor]
#   theme = "DMZ-White"

# Or symlink Vanilla-DMZ -> DMZ-White if you have configs that name the
# Ubuntu/legacy theme:
sudo ln -s DMZ-White /usr/share/icons/Vanilla-DMZ
```

## 7. Reboot

```bash
sudo systemctl reboot
```

You should land on the bootsplash compass, transition into the login card,
and end up on the meridian desktop with panel + launcher.

## Known optional packages

Not required for the desktop to work, but useful in practice:

- `libnotify-bin` — provides `notify-send` for testing the notification
  daemon. Without it use `gdbus call --session --dest org.freedesktop.Notifications ...`.
- `network-manager-gnome` — GUI for NM connection editing (`nm-connection-editor`).
- A real terminal emulator like `kitty`, `alacritty`, or `foot` — Meridian's
  default panel has a Term entry that runs `kitty`.

## Build-only systems

If you only need to compile (no run), step 1 is enough. Skip the runtime
packages and the systemd/PAM setup.
