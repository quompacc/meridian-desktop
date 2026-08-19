# Meridian Desktop

Meridian is an experimental Wayland desktop and compositor written in Rust.
Its long-term direction is a polished BSD-capable desktop: native Rust where
system ownership, security and performance matter; WebKit-based UI where HTML
and CSS make a coherent modern interface practical for a small team.

> **Strategy status (2026-08-19):** the current Rust compositor and native shell
> remain the working implementation. The new WebKit UI platform is the active
> direction, but has not yet replaced the shell. OpenBSD is the next hardware
> evaluation target; FreeBSD remains the supported BSD fallback.

## Product direction

Rust continues to own:

- Wayland/XWayland, DRM/KMS and input
- window and workspace management
- IPC, system services and platform integration
- privileged helpers, policy and security boundaries
- compositor-level effects and final surface composition

Meridian-owned desktop surfaces will converge on a shared UI platform:

- a small WebKit runtime, not a general Tauri clone
- HTML, CSS and Web Components
- a deliberately small TypeScript/JavaScript layer
- a typed, capability-scoped Rust bridge
- CSS generated from `meridian-tokens` and `meridian-config`
- shared components, icons, typography and interaction primitives

The first proof is deliberately narrow: runtime and bridge, then panel,
launcher and Quick Settings. Settings and other Meridian system tools follow
only after that vertical slice proves visual quality, responsiveness, security
and BSD viability.

External GTK, Qt, Firefox, Chromium/Electron and wxWidgets applications remain
ordinary Wayland or XWayland clients. Meridian does not render or replace them.

## Current implementation

The repository already contains:

- a Smithay-based Wayland compositor with DRM/KMS and Winit backends
- XDG Shell, Layer Shell, XWayland and compositor IPC
- a separate native Rust shell with panel, launcher, popups and settings
- login, lock, portal and polkit processes
- multi-monitor/workspace infrastructure and diagnostics
- centralized design tokens guarded against render-code hardcodes
- Linux installation support and a FreeBSD installer path

This implementation is retained as the behavioral reference while the new UI
platform is proven incrementally. No big feature expansion should precede the
vertical slice.

## Platform strategy

- **OpenBSD:** next real-hardware evaluation on an Acer laptop using only its
  Intel HD 620. The NVIDIA 940MX is intentionally ignored.
- **FreeBSD:** maintained alternative when OpenBSD hardware, WebKit or desktop
  compatibility is insufficient. Existing installer work remains valuable.
- **Linux:** current development and compatibility platform; no longer the only
  product assumption.
- **VMs:** fast and reproducible regression checks, never the final authority
  for DRM/KMS, input, suspend/resume or performance.

See [the roadmap](ROADMAP.md), [the active master plan](MERIDIAN_OS_PLAN.md),
[the UI platform design](docs/UI_PLATFORM.md), and
[the OpenBSD evaluation guide](docs/OPENBSD.md).

## Design system

[The Meridian Design Manifest](docs/meridian_design_manifest.md) is binding.
There are exactly two themes, light and dark, identical except for colors.
`meridian-tokens` plus `meridian-config` remain the single source of truth;
Web UI consumes generated CSS variables rather than defining a second token
system.

## Build and test

The current implementation is a Rust workspace. On a supported Unix build host:

```bash
cargo build --workspace
cargo test --workspace
cargo check --workspace
```

Before a patch is considered ready:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p meridian-tokens --test design_guard
```

Platform-specific prerequisites and installation paths are documented in
[INSTALL.md](INSTALL.md) and [docs/FREEBSD.md](docs/FREEBSD.md). OpenBSD does
not yet have a turnkey installer; its first phase is evidence gathering.

## Documentation map

Use this precedence when documents disagree:

1. [current BSD handoff](Meridian%20-%20BSD%20%E2%80%93%20%C3%9Cbergabe%20f%C3%BCr%20ChatGPT%20Desktop.md)
2. [active master plan](MERIDIAN_OS_PLAN.md) and [roadmap](ROADMAP.md)
3. [design manifest](docs/meridian_design_manifest.md) for every visual decision
4. [architecture](docs/ARCHITECTURE.md) and [project status](docs/PROJECT_STATUS.md)
5. focused technical documents
6. dated audits and superseded plans, which are historical evidence only

Important references:

- [UI platform](docs/UI_PLATFORM.md)
- [OpenBSD evaluation](docs/OPENBSD.md)
- [FreeBSD support](docs/FREEBSD.md)
- [external application compatibility](docs/APP_STACK.md)
- [testing](docs/TESTING.md)
- [configuration](docs/CONFIGURATION.md)
- [debugging](docs/DEBUGGING.md)
- [technical design guidelines](docs/technical-design-guidelines.md)

## Philosophy

**Native where it matters. Web where it shines.**

Meridian values protocol correctness, a small and understandable trusted base,
real-hardware performance, coherent design and bounded maintenance cost. It is
not trying to replace third-party application toolkits or rebuild a general web
application framework.

## License

Meridian is licensed under GPL-3.0-or-later.
