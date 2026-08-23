# OpenBSD Sandbox Plan

> Status: agreed design, not yet implemented. This document defines the
> fail-closed rollout of `pledge(2)` and `unveil(2)` for Meridian processes.

## Current assessment

Meridian already separates authentication, policy, portal, shell, compositor
and WebKit UI responsibilities into distinct processes with narrow typed IPC
boundaries. OpenBSD authentication uses `auth_userokay(3)`, and the WebKit UI
does not own raw DRM or input handles.

No Meridian process currently applies `pledge(2)` or `unveil(2)`. The existing
architecture is therefore the prerequisite for OpenBSD-style self-restriction,
not its completion.

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

Still open before sandbox implementation: deliberately reject one bad password
then accept a good retry, run repeated lock cycles, cover output/theme variants,
execute the three real-hardware process-exit cases above, and route the remaining
shell/menu lock entry points through the same compositor supervisor.

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

1. `meridian-lock`
2. `meridian-polkit`
3. `meridian-portal`
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
