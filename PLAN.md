# Meridian — Plan index

> Updated 2026-08-19. The original compositor buildup plan has been completed
> far beyond its initial milestones and is no longer the active task list.

## Active plan

Meridian continues as its own Rust Wayland compositor and adopts a shared
WebKit-based platform for Meridian-owned desktop UI. BSD support is a first-class
goal. OpenBSD is evaluated first on real Intel hardware; FreeBSD remains the
supported alternative if practical compatibility requires it.

The binding execution order is:

1. inventory the Acer test hardware;
2. establish an OpenBSD hardware and desktop baseline;
3. validate Rust, Smithay/DRM and Meridian core paths;
4. prove the minimal WebKit runtime and Rust bridge;
5. migrate panel, launcher and Quick Settings as one vertical slice;
6. migrate further Meridian system tools only after that proof succeeds.

No large unrelated feature work should be scheduled ahead of these steps.

## Document ownership

- Strategy and OS decision: `MERIDIAN_OS_PLAN.md`
- Execution phases and exit criteria: `ROADMAP.md`
- Runtime and bridge design: `docs/UI_PLATFORM.md`
- Current implementation, not future claims: `docs/PROJECT_STATUS.md`
- Current and target architecture: `docs/ARCHITECTURE.md`
- Visual rules: `docs/meridian_design_manifest.md`
- OpenBSD test procedure: `docs/OPENBSD.md`
- FreeBSD install/support path: `docs/FREEBSD.md`

The dated audits, `docs/SSD_FRAME_PLAN.md`, and native-shell refactoring notes
remain historical evidence. They do not override the current strategy.

## Architectural invariants

- Compositor, DRM/KMS, input, window management, policy and privileged helpers
  remain native Rust responsibilities.
- External applications remain normal Wayland/XWayland clients.
- `meridian-tokens` plus `meridian-config` are the only design source. Web UI
  consumes generated CSS variables and shared components.
- The Rust↔UI bridge is typed, capability-scoped and deny-by-default.
- Render order and existing IPC behavior are compatibility contracts.
- OpenBSD uses its own security mechanisms; FreeBSD uses its own mechanisms.
- Real hardware decides hardware readiness. VMs support, but do not replace,
  that evidence.

## Decision gates

OpenBSD becomes the primary target only if the Acer evaluation shows acceptable
GPU, input, network, audio, suspend/resume, browser and WebKit behavior.
Otherwise FreeBSD is selected without trying to make it behave like OpenBSD.

The WebKit migration proceeds beyond the spike only if it demonstrates:

- reliable BSD packaging/runtime availability;
- acceptable cold-start, idle, memory and interaction latency;
- a small security surface with explicit permissions;
- stable Wayland surface/lifecycle integration;
- exact conformance with the Meridian design system.
