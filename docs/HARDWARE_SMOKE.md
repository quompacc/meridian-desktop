# Controlled Hardware Smoke

This runbook separates preflight work from checks that can only be proven on
real DRM/input hardware.

## Goal

Get from boot/login to a usable Meridian desktop once, with recovery available.
Do not combine the first boot with broad hotplug, suspend, or gaming tests.

## Preflight before enabling tty1 login

1. Build and install from the repo:

   ```bash
   scripts/install-deps.sh all
   scripts/install-local.sh --build
   ```

2. Verify recovery:

   ```bash
   ssh <host> true
   systemctl status getty@tty2.service --no-pager
   ```

3. Verify installed files exist:

   ```bash
   command -v meridian meridian-shell meridian-login meridian-lock meridian-portal meridian-polkit-agent
   test -f /etc/pam.d/meridian-login
   test -f /etc/pam.d/meridian-login-password
   test -d /usr/local/share/meridian/themes
   test -f /usr/local/share/xdg-desktop-portal/portals/meridian.portal
   test -f /usr/local/share/dbus-1/services/org.freedesktop.impl.portal.desktop.meridian.service
   ```

4. Only then enable boot login:

   ```bash
   scripts/install-local.sh --enable-boot
   ```

## First boot pass criteria

After reboot, verify from the machine and over SSH:

- bootsplash/login appears, or login appears directly if bootsplash is absent
- password fallback works when no registered YubiKey is ready
- the desktop reaches panel + launcher
- keyboard and pointer input work
- `Super+Space` opens the launcher
- a terminal or simple app launches
- `Super+L` starts `meridian-lock` and unlock succeeds
- logout returns to `meridian-login`

Collect:

```bash
systemctl --failed --no-pager
systemctl status --no-pager meridian-login.service
sudo journalctl -b -u meridian-login.service --no-pager
journalctl --user -b --no-pager | grep -E 'meridian|portal|polkit|autostart' || true
```

## Portal smoke

Inside the Meridian session:

```bash
busctl --user --list | grep -E 'xdg|portal|meridian' || true
busctl --user introspect org.freedesktop.impl.portal.desktop.meridian /org/freedesktop/portal/desktop --no-pager
```

FileChooser requires a real xdg-desktop-portal client. Screenshot requires the
Meridian consent modal and should produce a file URI when allowed.

## Hotplug and mode-change follow-up

Only after the first boot is stable, run the hardware-only checks:

1. Change mode/refresh if available and verify pointer corners, window drag,
   panel hit targets, and launcher positioning.
2. Disconnect a non-primary monitor and verify no crash, no lost windows, layer
   shell recovery, workspace snapshot logs.
3. Reconnect it and verify output add, layer recovery, pointer routing, and
   workspace state.
4. Repeat with the primary monitor if recovery access is solid.

Expected log patterns are documented in `docs/DEBUGGING.md` and
`docs/MULTI_MONITOR.md`.

## Stop conditions

Stop and recover through SSH/tty2 if any of these happen:

- login loop on tty1
- no input in both login and desktop
- black screen with no SSH access
- repeated failed units in `systemctl --failed`
- hotplug causes a compositor crash or unrecoverable blank output
