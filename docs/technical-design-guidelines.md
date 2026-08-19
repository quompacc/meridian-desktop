# Technical Design Guidelines

These guidelines define implementation discipline for Meridian patches.

The active product direction is the BSD-capable Rust compositor plus an
unprivileged WebKit UI platform for Meridian-owned surfaces. Target architecture
and current implementation must always be labelled separately.

## Native / Web Boundary
- Keep DRM/KMS, input, Wayland policy, window management, IPC authority and
  privileged helpers in Rust.
- Use WebKit only for Meridian-owned presentation surfaces and system tools.
- Do not move authentication or privileged policy into JavaScript as a side
  effect of UI migration.
- Do not build a general Tauri replacement; expose the smallest required API.

## Bridge Security
- Bridge operations are typed, versioned, capability-scoped and deny-by-default.
- Packaged local content is the default; remote navigation has no privileged
  bridge access.
- No ambient filesystem, network or subprocess access.
- Validate identity, bounds and state at every process boundary.

## Platform Discipline
- Isolate Linux, OpenBSD and FreeBSD adapters explicitly.
- Apply `pledge`/`unveil` and Capsicum/FreeBSD facilities according to their own
  OS semantics, not through misleading equivalence.
- Real hardware is authoritative for GPU, input, suspend and performance claims.

## Protocol Correctness Over App Hacks
- Prefer protocol-correct behavior over application-specific exceptions.
- Do not introduce compatibility behavior that breaks standards semantics.

## Wayland Primary Path
- Implement and validate the Wayland path first.
- Keep Wayland behavior as the reference for product correctness.

## Xwayland Compatibility
- Maintain reliable Xwayland compatibility without turning X11 edge cases into global policy.
- Keep Xwayland-specific handling scoped and explicit.

## No Global SSD Forcing
- Do not force global SSD policy as a workaround for toolkit-specific visuals.
- Respect decoration ownership boundaries unless protocol-level behavior requires otherwise.

## Minimal Settings
- Favor minimal settings surfaces.
- New settings must have clear product value and low long-term maintenance cost.

## Meridian-Owned Shell Components
- Keep panel, launcher, and compositor-owned UI behavior coherent and predictable.
- Avoid unbounded extension points that fragment UX.
- Until the vertical slice is proven, keep the native shell as behavioral
  reference and fallback.

## Toolkit-Neutral Defaults
- Default behavior must remain toolkit-neutral.
- Do not optimize defaults around one toolkit at the expense of others.

## Small Patches Only
- Ship focused, reviewable, testable patches.
- Avoid mixed refactors and behavioral changes in one patch.

## Diagnostics Cleanup
- Add diagnostics only when needed for verification.
- Remove temporary diagnostics or lower their level after the audit is complete.

## Validation Gates
- Rust changes must pass:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `git diff --check`
- Web UI changes additionally require generated-token snapshots, bridge schema
  tests, component interaction/accessibility tests and an idle/performance check.

## Visual Validation
- For rendering or input-adjacent changes, validate affected visual and interaction paths directly.
- Preserve render-order correctness and avoid accidental stacking regressions.

## Stop When Internals Are Unclear
- If Smithay, Xwayland, GTK, COSMIC, KDE, or wlroots internals are unclear, stop and clarify before patching behavior.

## Decision Priority
Apply this priority order:
1. security boundary and protocol correctness
2. desktop coherence and central design source
3. bounded complexity
4. measured performance sanity
5. portability and compatibility scope control

## Product Filter
A technical change is acceptable only if it:
1. aligns with Meridian product direction
2. avoids policy drift and app-specific coupling
3. remains maintainable under small-patch discipline
