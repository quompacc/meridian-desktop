# OpenBSD graphical boot status — 2026-08-27

Reference host: Acer Aspire F5-573G, OpenBSD 7.9/amd64 `GENERIC.MP#4`.

## Observed baseline

- `/etc/boot.conf` is absent; the standard five-second loader prompt remains.
- `/etc/rc.conf.local` enables `seatd`, `messagebus`, `meridian_bootsplash`,
  and `meridian_login`. The splash is ordered first among package services.
- `sshd` is enabled and reachable. `ttyC1`, `ttyC2`, `ttyC3` and `ttyC5` keep
  local recovery gettys enabled.
- Boot diagnostics remain in `/var/run/dmesg.boot`, `/var/log/messages` and
  `/var/log/daemon`.
- OpenBSD uses `/var/run`, not `/run`. The login/compositor handover socket is
  therefore `/var/run/meridian-login.sock` on this target.
- Intel KMS drives the internal 1920x1080 panel. A normal reboot reaches the
  native greeter; login starts the compositor as the selected unprivileged
  user, and logout returns to the greeter.

## Supported controls and recovery

The OpenBSD 7.9 `boot(8)` interface supports loader timeout, console selection
and explicit boot commands, but has no supported native quiet/splash flag.
Holding either Control key while BOOT starts skips `/etc/boot.conf` and cancels
automatic boot. `boot -s` remains the explicit single-user recovery path.

`scripts/install-openbsd-boot.ksh` gates boot enablement behind a running SSH
daemon and an enabled `ttyC1` getty, backs up `/etc/rc.conf.local`, and never
edits `/etc/boot.conf`. With `--bootsplash`, the native splash replaces late
userspace output after the package-service phase begins; loader, kernel, and
early `rc(8)` diagnostics remain visible by design. See
`docs/OPENBSD_QUIET_BOOT.md` for the supported boundary.

## Test order

1. Build and install both native components without enabling boot:

   ```sh
   scripts/install-openbsd-boot.ksh --build --bootsplash ../bootsplash
   ```

2. Keep SSH open, stop the active graphical service, then test the handover:

   ```sh
   doas rcctl start meridian_bootsplash
   doas rcctl start meridian_login
   ```

3. Verify keyboard, pointer, BSD Authentication, desktop handover, logout back
   to the greeter, and `doas rcctl stop meridian_login` recovery.

4. Only after that pass, enable the complete boot chain:

   ```sh
   scripts/install-openbsd-boot.ksh --bootsplash ../bootsplash --enable-boot
   ```

5. Reboot normally and verify splash, greeter, login, logout, SSH, and `ttyC1`
   recovery. Do not redirect the OpenBSD system console: the diagnostic and
   recovery value outweighs hiding its early boot text.

# Graphical console isolation

`meridian_login` switches to `ttyC4` before opening the DRM and wscons
devices.  `/etc/ttys` must keep `ttyC4` `off secure`, so no `getty` can consume
credential keystrokes behind the graphical surface.  The service deliberately
keeps `ttyC4` focused when the greeter/compositor session exits or restarts, so
an existing `ttyC0` getty buffer cannot flash through the graphical handover.
`ttyC1` remains the primary recovery console and can be selected explicitly.
