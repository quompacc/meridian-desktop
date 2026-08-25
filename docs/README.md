# Meridian Documentation Index

> Updated 2026-08-25 for the BSD/native-Rust strategy.

## Precedence

When documents disagree, use this order:

1. the root BSD handoff, `MERIDIAN_OS_PLAN.md` and `ROADMAP.md`
2. `meridian_design_manifest.md` for every visual decision
3. `PROJECT_STATUS.md` for implemented behavior
4. `ARCHITECTURE.md` and `NATIVE_UI_PLAN.md` for current/target boundaries
5. focused technical documents
6. dated audits and superseded plans as historical evidence

`AGENTS.md` and `CLAUDE.md` define patch discipline and validation rules.

## Active strategy and architecture

- `../MERIDIAN_OS_PLAN.md` — BSD and UI master plan
- `../ROADMAP.md` — execution phases and decision gates
- `ARCHITECTURE.md` — current native architecture
- `NATIVE_UI_PLAN.md` — binding native UI sequence and acceptance gates
- `PROJECT_STATUS.md` — current implemented state
- `GUI_CENTRALIZATION_PLAN.md` — binding native token pipeline and DoD
- `meridian_design_manifest.md` — binding visual specification
- `technical-design-guidelines.md` — engineering decision rules

## Platforms and compatibility

- `OPENBSD.md` — evidence-first Acer evaluation
- `FREEBSD.md` — existing FreeBSD install/support path
- `APP_STACK.md` — external Wayland/XWayland application matrix
- `FRAME_STRATEGY.md` — decoration findings; deferred compatibility context
- `NVIDIA_PASSTHROUGH.md` — Linux/NVIDIA-specific evidence
- `HARDWARE_SMOKE.md` — current Linux controlled-hardware smoke

## Current subsystem references

- `CODE_INDEX.md` — generated source map of the native implementation
- `CONFIGURATION.md` — config format and reload
- `DEBUGGING.md` — current diagnostics and manual test procedures
- `TESTING.md` — native and platform validation
- `PERFORMANCE_RULES.md` / `VISUAL_PERFORMANCE.md` — native UI budgets
- `DESKTOP_SETTINGS_CONTRACT.md` — settings ownership and toolkit export
- `MERIDIAN_LOGIN.md` — current login architecture
- `MULTI_MONITOR.md` / `WORKSPACES.md` — compositor output/workspace policy
- `XDG_PORTALS.md` — portal architecture

## Historical or deferred documents

- `AUDIT_2026-05-25.md`
- `AUDIT_2026-06-20.md`
- `SSD_FRAME_PLAN.md`
- `REFACTORING_PLAN.md` (native-shell maintenance only)
- `UI_PLATFORM.md` (retired WebKit prototype architecture)
- `design-reference/webkit-prototype/` (nonnormative visual reference)

Historical documents are kept because their measurements and failed approaches
are useful. Their priority lists do not override the active roadmap.
