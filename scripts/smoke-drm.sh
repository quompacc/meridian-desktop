#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

LOG_FILE="${MERIDIAN_SMOKE_LOG:-/tmp/meridian-smoke-drm.log}"
TIMEOUT_SECONDS="${MERIDIAN_SMOKE_TIMEOUT:-20}"
MODE="${MERIDIAN_SMOKE_MODE:-smoke}"

usage() {
  cat <<'EOF'
Usage: scripts/smoke-drm.sh [smoke|run|--help]

Modes:
  smoke (default)  Run compositor under timeout for regression checks.
  run              Run compositor without timeout for manual tests.

Environment:
  MERIDIAN_SMOKE_TIMEOUT   Timeout seconds for smoke mode (default: 20)
  MERIDIAN_SMOKE_LOG       Log file path (default: /tmp/meridian-smoke-drm.log)
  MERIDIAN_SMOKE_MODE      Default mode if no positional mode is passed
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

if [[ -n "${1:-}" ]]; then
  MODE="$1"
fi

case "${MODE}" in
  smoke|default) MODE="smoke" ;;
  run) ;;
  *)
    echo "[smoke-drm] invalid mode: ${MODE}" >&2
    usage >&2
    exit 2
    ;;
esac

echo "[smoke-drm] repo: ${REPO_ROOT}"
echo "[smoke-drm] log:  ${LOG_FILE}"
if [[ "${MODE}" == "smoke" ]]; then
  echo "[smoke-drm] mode: smoke (timeout=${TIMEOUT_SECONDS}s)"
else
  echo "[smoke-drm] mode: run (no timeout; stop with Ctrl+C or pkill)"
fi

echo "[smoke-drm] building release..."
cargo build --release

echo "[smoke-drm] stopping old processes (if any)..."
pkill -f 'target/release/meridian|meridian-shell|cargo run.*meridian' || true

echo "[smoke-drm] running compositor..."
set +e
if [[ "${MODE}" == "smoke" ]]; then
  MERIDIAN_DRM_TIMING=1 \
  MERIDIAN_DIRTY_STATS=1 \
  MERIDIAN_SHELL_RENDER_STATS=1 \
  RUST_LOG=info \
  timeout "${TIMEOUT_SECONDS}s" target/release/meridian 2>&1 | tee "${LOG_FILE}"
else
  MERIDIAN_DRM_TIMING=1 \
  MERIDIAN_DIRTY_STATS=1 \
  MERIDIAN_SHELL_RENDER_STATS=1 \
  RUST_LOG=info \
  target/release/meridian 2>&1 | tee "${LOG_FILE}"
fi
run_exit="${PIPESTATUS[0]}"
set -e

echo
echo "[smoke-drm] summary (grep):"
grep -E "GL Vendor|GL Renderer|drm api selected|drm mode selected|drm timing summary|dirty reasons|shell render summary|too slow|lagging|error|warn" "${LOG_FILE}" || true

echo "[smoke-drm] run exit code: ${run_exit}"
exit "${run_exit}"
