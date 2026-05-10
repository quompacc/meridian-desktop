# Meridian

Meridian ist ein ressourcenschonender Wayland-Compositor in Rust mit separatem Shell-Prozess (`meridian-shell`).

## Workspace
- `meridian-compositor`
- `meridian-shell`
- `meridian-config`
- `meridian-ipc`
- `meridian-wm`
- `meridian-portal` (Scaffold)

## Schnellstart
- Build: `cargo build`
- Run: `cargo run`
- Tests: `cargo test --workspace`
- Check: `cargo check --workspace`

## DRM/NVIDIA Smoke Test
- Script: `scripts/smoke-drm.sh`
- Regression (default, mit Timeout):
  - `MERIDIAN_SMOKE_TIMEOUT=20 MERIDIAN_SMOKE_LOG=/tmp/meridian-smoke-drm.log scripts/smoke-drm.sh`
- Manueller UX/Launcher-Lauf (ohne Timeout):
  - `scripts/smoke-drm.sh run`
  - oder: `MERIDIAN_SMOKE_MODE=run scripts/smoke-drm.sh`
- Das Script baut `--release`, startet Meridian mit Timing/Dirty/Shell-Render-Stats und schreibt den gesamten Lauf in ein Log.
- Es gibt danach eine kurze grep-Auswertung mit den wichtigsten Indikatoren aus.

## Wichtige Doku
- Status: `docs/PROJECT_STATUS.md`
- Debugging: `docs/DEBUGGING.md`
- NVIDIA-Passthrough: `docs/NVIDIA_PASSTHROUGH.md`
- Multi-Monitor: `docs/MULTI_MONITOR.md`
- Testing-Index: `docs/TESTING.md`
