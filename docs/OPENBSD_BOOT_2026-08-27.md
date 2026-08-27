# OpenBSD boot baseline — 2026-08-27

Reference host: Acer Aspire F5-573G, OpenBSD 7.9/amd64 `GENERIC.MP#4`.

## Observed baseline

- `/etc/boot.conf` is absent; the standard five-second loader prompt remains.
- `/etc/rc.conf.local` enables only `seatd` and `messagebus` as package
  services. `meridian-login` and `bootsplash` are not installed system-wide.
- `sshd` is enabled and reachable. `ttyC1`, `ttyC2`, `ttyC3` and `ttyC5` keep
  local recovery gettys enabled.
- Boot diagnostics remain in `/var/run/dmesg.boot`, `/var/log/messages` and
  `/var/log/daemon`.
- OpenBSD uses `/var/run`, not `/run`. The login/compositor handover socket is
  therefore `/var/run/meridian-login.sock` on this target.
- Intel KMS drives the internal 1920x1080 panel. The development compositor is
  currently launched manually; this is not evidence for boot-to-greeter.

## Supported controls and recovery

The OpenBSD 7.9 `boot(8)` interface supports loader timeout, console selection
and explicit boot commands, but has no supported native quiet/splash flag.
Holding either Control key while BOOT starts skips `/etc/boot.conf` and cancels
automatic boot. `boot -s` remains the explicit single-user recovery path.

The first product step therefore installs and proves the native greeter while
leaving all console output visible. `scripts/install-openbsd-boot.ksh` gates
boot enablement behind a running SSH daemon and an enabled `ttyC1` getty, backs
up `/etc/rc.conf.local`, and never edits `/etc/boot.conf`.

## Test order

1. Build and install without enabling boot:

   ```sh
   scripts/install-openbsd-boot.ksh --build
   ```

2. Keep SSH open, stop the development compositor, and start the greeter with
   rc.d debugging visible:

   ```sh
   doas /etc/rc.d/meridian_login -df start
   ```

3. Verify keyboard, pointer, BSD Authentication, desktop handover, logout back
   to the greeter, and `doas rcctl stop meridian_login` recovery.

4. Only after that pass, enable the service:

   ```sh
   scripts/install-openbsd-boot.ksh --enable-boot
   ```

5. Record a normal reboot before investigating console redirection or an early
   userspace splash. Quiet boot must retain the Control-key/single-user and SSH
   recovery paths and must not hide a failed greeter behind a black screen.

# Graphical console isolation

`meridian_login` switches to `ttyC4` before opening the DRM and wscons
devices.  `/etc/ttys` must keep `ttyC4` `off secure`, so no `getty` can consume
credential keystrokes behind the graphical surface.  The service deliberately
keeps `ttyC4` focused when the greeter/compositor session exits or restarts, so
an existing `ttyC0` getty buffer cannot flash through the graphical handover.
`ttyC1` remains the primary recovery console and can be selected explicitly.
