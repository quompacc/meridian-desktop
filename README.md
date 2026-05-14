# Meridian Desktop

Meridian is a calm, modern Wayland desktop positioned between GNOME and KDE.

Wayland-first, toolkit-neutral, and opinionated in sensible defaults, Meridian focuses on productive workflows without configuration overload.

Meridian is a calm, Wayland-first desktop for users who want polish without rigidity and power without clutter.

## Vision

Meridian aims to be a full desktop environment, not just a compositor plus loose utilities.
The project focuses on a curated UX, strong runtime behavior, and practical performance on real hardware.

Long-term, Meridian targets a polished Linux desktop that feels coherent out of the box and remains responsive for everyday use and gaming-oriented workflows.

## Design Manifesto

- [Meridian Design Manifesto](docs/design-manifesto.md)
- [Technical Design Guidelines](docs/technical-design-guidelines.md)

## Current Status

Meridian is active and moving fast, but still experimental.
It is not yet ready as a daily driver for most users.

### Working now

- Wayland compositor core
- DRM/KMS backend
- Shell process with panel and launcher
- XWayland support
- IPC foundations between compositor and shell
- Screenshot/portal groundwork
- Ongoing NVIDIA timing and mode-selection stability work

### Experimental / in progress

- Multi-monitor polish and hotplug edge cases
- Launcher UX evolution (favorites, categories, richer layout)
- Icon pipeline and visual refinement
- Settings UI
- Power/session controls
- Gaming-oriented UX features

## Features Overview

- Rust workspace with separated compositor, shell, config, IPC, and WM logic
- Wayland-first architecture with a dedicated shell client
- Focus on correctness in render/input paths and explicit testing discipline
- Practical diagnostics for DRM/runtime issues during development

## Screenshots

Coming soon.

Real screenshots will be added once current shell UX and visual baselines are finalized.

## Build & Run

Dependencies are the usual Rust + Linux Wayland/DRM development stack (tooling and headers vary by distro).

```bash
cargo build --workspace
cargo test --workspace
cargo build --release --workspace
```

For release runs, use the matching binaries from this workspace so `meridian` and `meridian-shell` stay in sync:

```bash
PATH="$PWD/target/release:$PATH" target/release/meridian
```

## Development Workflow

Before opening or updating a patch:

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
git diff --check
```

## Roadmap

- Foundation and stability hardening
- Shell UI quality and consistency
- Launcher improvements
- Panel/taskbar and pinned-app workflows
- Settings, power, and session management
- Gaming-friendly features and performance polish

## Contributing

Contributors are welcome.

Good first areas include:
- targeted bug fixes
- test coverage improvements
- launcher/panel UX polish
- documentation cleanup and accuracy updates

Please prefer focused, small patches with clear scope and tests where applicable.

## Philosophy / Non-goals

Meridian is intentionally opinionated:
- a fixed high-quality UI baseline
- limited, purposeful customization
- no fragmented widget/plugin wildgrowth

The goal is cohesion and reliability over endless surface-level tweakability.

## Documentation

- [Meridian Design Manifesto](docs/design-manifesto.md)
- [Technical Design Guidelines](docs/technical-design-guidelines.md)
- [Project status](docs/PROJECT_STATUS.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Testing guide](docs/TESTING.md)
- [Configuration](docs/CONFIGURATION.md)
- [Debugging guide](docs/DEBUGGING.md)
- [NVIDIA passthrough notes](docs/NVIDIA_PASSTHROUGH.md)
- [Multi-monitor audit](docs/MULTI_MONITOR.md)
- [Workspace policy](docs/WORKSPACES.md)
- [XDG portals plan](docs/XDG_PORTALS.md)

## License

Meridian is licensed under GPL-3.0-or-later.
