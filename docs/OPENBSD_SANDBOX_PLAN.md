# OpenBSD Sandbox Plan

> Status: `meridian-lock` and `meridian-polkit` pilots implemented and verified
> on reference hardware on 2026-08-23 and 2026-08-24. This document defines the
> continuing fail-closed rollout of `pledge(2)` and `unveil(2)` for Meridian
> processes.

## Current assessment

Meridian already separates authentication, policy, portal, shell, compositor
and WebKit UI responsibilities into distinct processes with narrow typed IPC
boundaries. OpenBSD authentication uses `auth_userokay(3)`, and the WebKit UI
does not own raw DRM or input handles.

The unprivileged `meridian-lock` UI and its narrow setgid authentication helper
apply separate `pledge(2)` and `unveil(2)` profiles. Other Meridian processes
remain unsandboxed until their own responsibilities and observed requirements
have been inventoried; the lock pilot is not a generic profile to copy blindly.

## Non-negotiable failure model

- Every failed `pledge()` or `unveil()` call is fatal. The process exits with a
  non-zero status instead of logging and continuing without its sandbox.
- Every path declaration and the final locking `unveil(NULL, NULL)` call is
  checked.
- Production profiles do not use the `error` pledge promise to turn policy
  violations into recoverable `ENOSYS` failures.
- There is no automatic unsandboxed runtime fallback. Recovery means SSH,
  console access, a previously known-good binary or an explicit rollback.
- OpenBSD profiles remain platform-specific. FreeBSD capability work is
  designed independently rather than hidden behind a false common policy.

## Pilot: `meridian-lock`

`meridian-lock` is the first candidate because it is small, security-sensitive
and has a bounded purpose. Sandboxing starts only after its unsandboxed
end-to-end lifecycle has been proven on the OpenBSD reference machine.

Before implementation, verify and record:

1. successful lock and unlock;
2. rejection of an incorrect password followed by a successful retry;
3. repeated lock cycles;
4. theme, output and input behavior;
5. process exit before the session lock is acquired;
6. process exit after acquisition but before the first complete frame;
7. process exit during authentication.

A lock-process failure must never release an acquired session lock. Once the
compositor has accepted the lock, it stays locked or starts a controlled
replacement lock process. This invariant must be covered before the sandbox is
enabled.

Local state-machine coverage was added before the sandbox work: the compositor
reaper delivers `meridian-lock` termination back to the compositor event loop.
An unsuccessful exit while the lock is pending or acquired preserves that
phase, prunes dead client-owned lock surfaces, clears keyboard focus and
requests a compositor-owned cleared frame. Only the protocol's explicit
`unlock_and_destroy` request reaches the unlock transition. Lock refusal and a
Wayland dispatch failure make `meridian-lock` exit unsuccessfully. Unit tests
cover failure before acquisition, while pending and after acquisition. This is
not a substitute for the real-hardware lifecycle and crash matrix above, which
remains required before enabling `pledge` or `unveil`.

OpenBSD reference verification on 2026-08-23: `cargo check --workspace`,
`cargo build --workspace`, the lock-focused tests and
`cargo test --workspace --exclude smithay` all passed. The exclusion is the
documented vendored-Smithay example limitation, not a Meridian test failure.

The same hardware run established the unsandboxed baseline for one Intel
output: `Super+L` reaches the compositor-supervised lock client, the lock
surface appears, keyboard input and the compositor-owned cursor remain usable,
BSD Authentication accepts the real account password, and explicit protocol
unlock returns to the WebKit desktop. Per-keystroke rendering now redraws only
the 460x310 card over a once-initialized background; the installed release
binary removed the visible input lag without adding idle work.

OpenBSD installation is part of the authentication boundary. Build as the
normal user, then install from a privileged, root-owned path:

```sh
env LIBRARY_PATH=/usr/local/lib:/usr/X11R6/lib \
    cargo build --release -p meridian-lock --bins
doas ./scripts/install-openbsd-lock
```

The resulting `/usr/local/libexec/meridian-lock` is the unprivileged Wayland/UI
process and must be owned by `root:wheel` with mode `0555`. Only the narrow
`/usr/local/libexec/meridian-openbsd-auth` helper is owned by `root:auth` with
mode `2555`; it is setgid `auth`, never setuid root. The helper accepts only a
bounded, length-prefixed password over a pipe and authenticates the real caller
UID, so the UI cannot select another account. Neither binary is installed from
or executed with special group privilege in a user-writable Cargo target path.

This privilege split was installed and verified on the reference hardware on
2026-08-23. `Super+L`, real-password authentication, explicit unlock and input
latency remained correct with the UI running without the `auth` group bit.

The same hardware session passed a rejected bad password followed by a good
retry and three consecutive lock/unlock cycles. Controlled `SIGKILL` tests then
covered every supervisor phase: `BeforeAcquisition` left the still-unlocked
desktop usable, while `PendingFailClosed` and `LockedFailClosed` kept the desktop
in a compositor-owned secured state until explicit SSH recovery. The locked
case was also repeated while the setgid auth helper was active. Because the real
Pending window completes in roughly four milliseconds, debug builds expose the
explicit `MERIDIAN_FAULT_INJECT_HOLD_LOCK_PENDING=1` test gate; release builds
compile that gate out.

The desktop context menu and launcher power menu now send the typed
`LockSession` IPC command instead of spawning `meridian-lock` directly. Both
paths passed on hardware through the compositor supervisor, including unlock in
dark and light themes. The built-in Intel output is covered; a real multi-output
lock run remains an expansion of the hardware matrix, not a reason to broaden
the initial single-output sandbox profile.

### Implemented lock profiles

The unprivileged Wayland/UI process installs its sandbox after connecting to
Wayland and resolving configuration, theme and account identity, but before it
requests the session lock. It pledges:

```text
stdio rpath wpath cpath proc exec sendfd recvfd
```

`stdio` covers memory, polling and operations on existing descriptors;
`sendfd`/`recvfd` cover Wayland descriptor transfer; and `proc exec` covers the
fixed authentication helper. `rpath wpath cpath` remain only because OpenBSD's
`shm_mkstemp(3)` uses a randomized backing object below `/tmp`. Its unveiled
view is:

```text
/usr/local/libexec/meridian-openbsd-auth  x
/dev/null                                  w
/tmp                                       rwc
/usr/libexec/ld.so                         rx
/var/run/ld.so.hints                       r
/usr/lib                                   r
```

`/dev/null` is required for the helper's suppressed standard error stream.
The dynamic-loader paths permit execution of the fixed helper. The final
`unveil(NULL, NULL)` call is checked before pledge is applied. The broadest
remaining capability is `/tmp` access; replacing per-buffer `shm_mkstemp(3)`
with a pre-opened, bounded framebuffer allocator is the next tightening
opportunity, not a reason to conceal the current breadth.

The single-request setgid helper reads the bounded password request before
installing its own sandbox. It then unveils read-only `/etc/login.conf` and
`/etc/login.conf.d`, the executable `/usr/libexec/auth/login_passwd`, and the
same runtime-loader paths. It pledges:

```text
stdio rpath getpw proc exec
```

`getpw` is required for the protected account database, while `proc exec`
supports `auth_userokay(3)` launching the machine's configured password
authentication program. No profile uses the `error` promise and no
`execpromises` are imposed: OpenBSD rejects execution of a setgid target under
exec promises, and the helper immediately installs its own stricter profile.

The hardware rollout found two initially missing unveiled paths through
`ktrace`: `/tmp` for `shm_mkstemp(3)` and `/dev/null` for `Command::spawn`.
Both were added for those named operations only. The final combined profile
passed a real-password unlock, bad-password retry, three repeated cycles and
normal input/performance checks. A non-executable UI left the unlocked desktop
usable; a killed acquired client produced `LockedFailClosed`; an unavailable
auth helper kept the lock screen active and successfully retried after mode
`root:auth 2555` was restored. The live UI process reported both pledge and
unveil state (`pU`) in `ps`.

## Pilot: `meridian-polkit`

The second pilot keeps the long-lived Wayland authentication UI unprivileged
and retains polkit's packaged setuid helper protocol. On OpenBSD the package
helper is `/usr/local/lib/polkit-1/polkit-agent-helper-1`; the agent grants
execute-only access to that fixed path and does not probe alternative paths
after installing its sandbox.

OpenBSD graphical sessions must be created through `ck-launch-session`.
ConsoleKit supplies `XDG_SESSION_COOKIE`, not a usable `XDG_SESSION_ID`, so the
agent resolves the cookie through `org.freedesktop.ConsoleKit.Manager` and
registers the complete `/org/freedesktop/ConsoleKit/SessionN` object path with
polkit. Registration is now a synchronous startup condition with a ten-second
bound: missing session identity, registration failure, timeout, missing
Wayland globals or sandbox installation all terminate the process with a
non-zero status. There is no guessed `c1` fallback and no registered-but-dead
UI process.

Build and install on OpenBSD as the normal user plus the narrow root install
step:

```sh
env LIBRARY_PATH=/usr/local/lib:/usr/X11R6/lib \
    cargo build --release -p meridian-polkit
doas ./scripts/install-openbsd-polkit
```

The installer places `/usr/local/bin/meridian-polkit-agent` as
`root:wheel 0555`, installs both Meridian theme tables read-only below
`/usr/local/share/meridian/themes`, and renders the existing XDG autostart
entry to `/etc/xdg/autostart/meridian-polkit-agent.desktop` as
`root:wheel 0444`. Starting through the shell's XDG autostart path is part of
correctness: an SSH-launched process is not a member of the graphical
ConsoleKit session and polkit rejects its registration.

After D-Bus registration, two Wayland roundtrips and required-global checks,
the agent locks this unveiled view:

```text
/usr/local/lib/polkit-1/polkit-agent-helper-1  x
/dev/null                                      w
/tmp                                           rwc
~/.config/meridian                             r   (when present)
~/.local/share/meridian/themes                 r   (when present)
$MERIDIAN_THEME_DIR(S)                         r   (existing entries only)
$XDG_DATA_DIRS/meridian/themes                 r   (existing entries only)
/usr/libexec/ld.so                             rx
/var/run/ld.so.hints                           r
/usr/lib                                       r
/usr/local/lib                                 r
```

Optional config/theme paths are collected before the first `unveil()` call;
probing them afterwards would incorrectly see still-hidden paths as absent.
The broad `/tmp` entry remains necessary for OpenBSD `shm_mkstemp(3)`. The
runtime-loader paths are required to execute the packaged dynamic helper. The
agent pledges:

```text
stdio rpath wpath cpath getpw proc exec sendfd recvfd
```

`getpw` resolves polkit's authorised Unix identities, `proc exec` launches the
fixed helper, and descriptor passing remains required by Wayland. No
`execpromises` are set because OpenBSD blocks setuid execution when they are
present. The packaged setuid helper starts outside Meridian's pledge profile
and owns its protected BSD Authentication and system-D-Bus access. Those paths
cannot be pre-unveiled by the unprivileged UI (`/usr/libexec/auth` is not
traversable by it); this is an explicit upstream trust boundary and residual
risk, not hidden UI access.

Reference-hardware verification on 2026-08-24 used a no-I/O `ktrace` inventory,
13 passing crate tests and a release build. The installed agent registered for
the real ConsoleKit session, loaded and live-reloaded the installed dark theme,
and reported pledged/unveiled state (`pU`) in `ps`. A rejected bad password
followed by the real password passed, as did three consecutive successful
authorizations. Starting without either XDG session variable exited with
status 1. Killing the agent while `pkexec` awaited input removed the dialog and
made `pkexec` report `not authorized`; no root action ran. A clean session
restart restored the autostarted sandboxed agent.

## Pilot: `meridian-portal`

The third pilot separates the narrow Meridian policy backend from the broad
filesystem view required by an interactive file chooser. On OpenBSD,
`meridian-portal` publishes only Settings, Screenshot and Access. The official
`xdg-desktop-portal-gtk` package owns FileChooser in a separate process;
Meridian therefore does not unveil the user's home directory and does not keep
`proc` or `exec` promises merely to launch a picker.

Build and install the backend as the normal user plus the narrow root step:

```sh
env LIBRARY_PATH=/usr/local/lib:/usr/X11R6/lib \
    cargo build --release -p meridian-portal
doas pkg_add xdg-desktop-portal xdg-desktop-portal-gtk
doas ./scripts/install-openbsd-portal
```

The installer requires both packaged portal daemons, installs the Meridian
binary as `root:wheel 0555`, and installs D-Bus activation, the OpenBSD-only
portal descriptor, routing config and both central theme tables as
`root:wheel 0444`. Settings and Screenshot route to Meridian; FileChooser
routes only to GTK.

After acquiring its session-bus name, the backend verifies that the Meridian
IPC path already exists as a real Unix socket, locks this unveiled view and
fails startup on every setup error:

```text
~/.config/meridian                     r   (when present)
~/.config/meridian/themes              r   (when present)
~/.local/share/meridian/themes         r   (when present)
$MERIDIAN_THEME_DIR(S)                 r   (existing entries only)
$XDG_DATA_HOME/meridian/themes         r   (when configured and present)
$XDG_DATA_DIRS/meridian/themes         r   (existing entries only)
$XDG_RUNTIME_DIR/meridian.sock         rw  (required Unix socket)
```

It pledges:

```text
stdio rpath unix sendfd recvfd
```

`rpath` supports config/theme reloads; `unix` supports the established D-Bus
connection and per-request compositor connections. Descriptor passing remains
available to the D-Bus transport. There is no process execution, general
write/create access, runtime-directory access or ambient home-directory view.

A no-I/O `ktrace` inventory on 2026-08-24 confirmed the config/theme reads and
D-Bus traffic. The installed release reports `pU` in `ps`; real
`org.freedesktop.portal.Settings.ReadOne` returns the active dark value `1`,
and a real frontend FileChooser request reaches the separate GTK backend and
cancels visibly. Startup without the compositor socket and with a regular file
substituted at its path both exit with status 1. A visible screenshot denial
returned response code 1 without a result; the allow path returned code 0 and
a valid local 1920x1080 PNG. The backend remained `pU`, and the temporary image
was removed after type, ownership and size verification. A clean Meridian
session restart with the production environment automatically activated the
Meridian and GTK backends without display/proxy errors; Settings and a new
FileChooser request still passed without any manual D-Bus environment update.

## Implementation method

1. Inventory actual filesystem, descriptor, authentication, Wayland, shared
   memory and process needs from source and an observed hardware run.
2. Document the proposed profile before adding it to code.
3. Apply OpenBSD-only `unveil` declarations, check every result, then lock the
   view with `unveil(NULL, NULL)`.
4. Apply an initially sufficient `pledge` profile and reduce it only from
   observed evidence. Later calls may narrow promises further after startup.
5. Treat sandbox installation errors as startup failures and pledge violations
   as defects, not as requests to broaden the profile automatically.
6. Re-run the complete lifecycle and crash matrix with SSH or console recovery
   available.
7. Record the final promises, unveiled paths, rationale and remaining breadth.

The sandbox must not make normal desktop behavior silently unreliable. A new
permission is added only for a named operation demonstrated by code or trace.

## Rollout order

1. `meridian-lock` (complete)
2. `meridian-polkit` (complete)
3. `meridian-portal` (complete)
4. `meridian-ui-runtime`
5. `meridian-login`
6. `meridian-shell`
7. Meridian compositor

Polkit and the portal refine the process and tooling before the WebKit runtime,
whose JIT memory, GTK/font access and subprocess model make it the most complex
unprivileged candidate. Login, shell and compositor follow later because they
change identities, launch arbitrary session programs or own broad device and
process responsibilities. If a useful profile remains too broad, split the
responsibility into a smaller helper instead of presenting a weak profile as
complete isolation.

## Definition of done per process

- required promises and paths are recorded with their reasons;
- all sandbox setup calls fail closed;
- normal behavior and relevant failure paths pass on real OpenBSD hardware;
- no acquired lock or authorization state becomes permissive on process exit;
- idle CPU/GPU behavior and startup latency remain within the existing budget;
- documentation identifies deliberate residual access and the next tightening
  opportunity.
