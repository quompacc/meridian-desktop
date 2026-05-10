#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

LOG_FILE="${MERIDIAN_SMOKE_LOG:-/tmp/meridian-smoke-drm.log}"
TIMEOUT_SECONDS="${MERIDIAN_SMOKE_TIMEOUT:-20}"

echo "[smoke-drm] repo: ${REPO_ROOT}"
echo "[smoke-drm] log:  ${LOG_FILE}"
echo "[smoke-drm] timeout: ${TIMEOUT_SECONDS}s"

echo "[smoke-drm] building release..."
cargo build --release

echo "[smoke-drm] stopping old processes (if any)..."
pkill -f 'target/release/meridian|meridian-shell|cargo run.*meridian' || true

echo "[smoke-drm] running compositor..."
set +e
MERIDIAN_DRM_TIMING=1 \
MERIDIAN_DIRTY_STATS=1 \
MERIDIAN_SHELL_RENDER_STATS=1 \
RUST_LOG=info \
timeout "${TIMEOUT_SECONDS}s" target/release/meridian 2>&1 | tee "${LOG_FILE}"
run_exit="${PIPESTATUS[0]}"
set -e

echo
echo "[smoke-drm] summary (grep):"
grep -E "GL Vendor|GL Renderer|drm api selected|drm mode selected|drm timing summary|dirty reasons|shell render summary|too slow|lagging|error|warn" "${LOG_FILE}" || true

echo "[smoke-drm] run exit code: ${run_exit}"
exit "${run_exit}"
