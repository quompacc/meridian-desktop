# Meridian UI Platform

> **STATUS: TARGET ARCHITECTURE.** Updated 2026-08-19. The native Rust shell is
> still the current implementation. This document defines the proof and
> migration target; it does not claim that the runtime already exists.

## Purpose

Meridian needs one UI implementation model for panel, launcher, Quick Settings,
notifications, overview, Settings and future system tools. The platform uses
WebKit for layout and presentation while keeping operating-system ownership,
policy and privileged work in Rust.

The goal is not “web apps everywhere.” The goal is one small, offline,
Meridian-specific presentation runtime with a narrow native boundary.

## Process model

```text
untrusted external apps
  └─ Wayland / XWayland ───────────────┐
                                       ▼
                               Meridian compositor
                               - DRM/KMS and input
                               - window/workspace policy
                               - final composition/effects
                                       │ typed IPC
                                       ▼
                               Meridian UI runtime
                               - WebKit process
                               - surface lifecycle
                               - capability broker
                                       │ local bridge
                                       ▼
                               Meridian UI document
                               - CSS variables
                               - Web Components
                               - minimal TypeScript
```

Privileged operations use separate, small Rust helpers. The WebKit process is
never privileged and never receives raw DRM/input handles.

## Runtime responsibilities

- create and supervise one or more Wayland surfaces
- load versioned bundled assets from an application-owned read-only location
- apply output scale, size and theme updates
- deliver typed state snapshots/events to the document
- validate bridge calls against the surface's capability set
- contain WebKit/document crashes and restart cleanly
- expose structured logs and performance counters

The runtime does not provide an ambient shell, arbitrary subprocess API,
general filesystem access or unrestricted network navigation.

## Bridge contract

Every operation must define:

- request and response schema
- caller/surface capability
- validation and bounds
- timeout/cancellation behavior
- error type
- audit/logging policy
- compatibility/versioning rule

State is snapshot-plus-event based. The document does not poll continuously.
Messages use stable semantic IDs rather than DOM details.

Initial proof operations should be intentionally harmless, for example reading
the active theme or requesting a launcher toggle through existing compositor
policy. Process launch, power, storage and package operations come later and
must cross dedicated policy services.

## Asset and navigation policy

- Primary documents, components, icons and fonts are packaged with Meridian.
- No HTTP server is required to render the shell.
- Top-level navigation outside the packaged origin is denied.
- Remote content, if a future tool needs it, is isolated from the privileged
  bridge and uses an explicit allowlist/content boundary.
- Development inspector/debug modes are never silently enabled in production.

## Design-token pipeline

`meridian-tokens` and `meridian-config` remain authoritative:

```text
Palette / Interaction / Elevation / Radius / Decorations
                         │
                         ├─ native Rust consumers
                         └─ generated, versioned CSS custom properties
                                      │
                                      └─ shared Web Components
```

CSS may compose tokens but must not introduce independent color, alpha, radius,
mix or geometry values. Exceptions require the same explicit guard rationale as
native render code. Light and dark themes differ only in their color table.

The design guard must be extended to packaged CSS/TypeScript before the first
production UI migration.

## Component model

The first shared component set is limited to what the vertical slice needs:

- button, icon button and toggle
- text/search input
- list/grid item and app tile
- menu/popover/dialog primitives
- slider and status row
- focus ring, tooltip and keyboard navigation helpers

Components own accessibility semantics, focus behavior, input states and token
usage. Product surfaces compose them; they do not fork their CSS.

## Product interaction models

Shared components do not imply identical information density on every surface.

The launcher is a complete, familiar application catalogue: search is always
available, favourites are the stable landing view, all installed applications
remain browsable alphabetically and by category, and session actions keep a
fixed location. It is compact and keyboard-first. Search is capability-scoped;
it does not become an ambient command shell or remote-content entry point.

Settings is a curated decision surface inspired by the restraint of GNOME and
macOS without copying either visual system. It uses strong defaults, a small
number of top-level areas and progressive disclosure. Frequent transient
changes belong in Quick Settings; durable system choices belong in Settings;
task-specific choices remain with the task they affect. Changes apply directly
except where a safe confirmation/rollback flow is required.

Both models use the same Meridian tokens, materials, controls and focus rules.
Meridian identity comes from consistent proportion and behaviour, never from
repeated compass, map or logo decoration in daily UI.

## Surface integration

The runtime must preserve existing Wayland roles and compositor stacking:

- panel/taskbar: layer-shell top layer with exclusive-zone policy
- launcher and Quick Settings: overlay/popover surfaces according to current
  compositor policy
- normal Meridian tools: ordinary XDG toplevels

Moving the renderer does not authorize changing render order, focus, workspace
or IPC semantics.

## Performance model

Measure at minimum:

- runtime cold start and first meaningful paint
- input-to-paint latency
- steady idle CPU and GPU utilization
- baseline and per-surface resident memory
- theme/scale switch cost
- launcher population and icon decode cost
- crash/restart recovery time

Rules:

- no periodic animation/timer when state is unchanged
- decode icons and static assets once, then cache by identity/scale/theme
- invalidate caches only for the dependency that changed
- virtualize lists/grids when their size requires it
- avoid layout thrash and synchronous bridge round trips in input handlers
- reduced-motion and low-capability fallbacks are first-class

Numeric acceptance budgets are recorded after the Acer baseline so they reflect
real hardware rather than guesses.

The first live runtime has no polling animation and assembles its HTML/CSS and
generated token sheet once before the initial load. WebKit uses an ephemeral
context, so it adds no persistent cookie/storage state. Bridge traffic is
event-driven and limited to launcher lifecycle plus catalogue-validated app
activation. The panel process remains resident; the launcher is currently
spawned on demand. Its noticeable cold-open delay is the next measurement and
optimization target.

## Security model by platform

Common rules:

- unprivileged runtime
- deny-by-default bridge
- no raw device access
- separate helpers for privileged actions
- validate all data at every process boundary
- fail closed when policy or identity is unavailable

OpenBSD may apply `pledge`/`unveil` per process. FreeBSD may use Capsicum and
other native facilities. Platform adapters implement the common capability
model without pretending the underlying OS APIs are identical.

## Migration and rollback

1. keep the native shell functional;
2. prove the runtime with a diagnostic surface;
3. migrate panel behind an explicit development switch;
4. add launcher, then Quick Settings;
5. test the complete slice in both themes and on the BSD reference hardware;
6. retire native paths only after behavioral parity and a rollback window.

IPC additions remain backward-compatible during migration. A runtime crash must
not crash the compositor; supervision may restart the UI or fall back to the
native shell.

## Open decisions for the spike

- the current proof targets OpenBSD's installed WebKitGTK 4.1/GTK3 ABI through
  `webkit2gtk` 2.0.2; the archived GTK3 Rust stack is an explicit maintenance
  risk and not yet the final runtime commitment
- exact WebKit port/API on FreeBSD and whether WebKitGTK 6.0 becomes practical
  on the selected reference path
- whether a maintainable Tauri subset exists on the selected target
- process-per-surface versus shared runtime isolation
- binary bridge encoding after the first typed JSON prototype
- numeric performance budgets from the Acer baseline
- packaging/update format for UI assets

These are evidence questions, not invitations to expand scope before the
hardware and runtime tests.
