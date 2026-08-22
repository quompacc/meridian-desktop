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
