#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

if [[ "$(uname -s)" == "OpenBSD" ]]; then
  export LIBRARY_PATH="${LIBRARY_PATH:-/usr/local/lib:/usr/X11R6/lib}"
  export RUSTFLAGS="${RUSTFLAGS:--L native=/usr/X11R6/lib}"
  export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/meridian-runtime-$(id -u)}"
  mkdir -p "${XDG_RUNTIME_DIR}"
  chmod 700 "${XDG_RUNTIME_DIR}"
fi

RUNNER=(target/release/meridian)
if command -v dbus-run-session >/dev/null 2>&1; then
  RUNNER=(dbus-run-session -- "${RUNNER[@]}")
fi
if [[ "$(uname -s)" == "OpenBSD" ]] && command -v ck-launch-session >/dev/null 2>&1; then
  RUNNER=(ck-launch-session "${RUNNER[@]}")
fi

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
cargo build --release --workspace

echo "[smoke-drm] stopping old Meridian session processes (if any)..."
pkill -x meridian-shell 2>/dev/null || true
pkill -x meridian 2>/dev/null || true
for _ in {1..50}; do
  if ! pgrep -x meridian-shell >/dev/null 2>&1 && ! pgrep -x meridian >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
if pgrep -x meridian-shell >/dev/null 2>&1 || pgrep -x meridian >/dev/null 2>&1; then
  echo "[smoke-drm] old session did not stop cleanly" >&2
  exit 1
fi
# seatd/libdrm can outlive the process table transition briefly while the old
# DRM master and atomic state are released.
sleep 1

echo "[smoke-drm] running compositor..."
echo "[smoke-drm] runtime profile: release (compositor and shell)"
set +e
if [[ "${MODE}" == "smoke" ]]; then
  MERIDIAN_DRM_TIMING=1 \
  MERIDIAN_DIRTY_STATS=1 \
  MERIDIAN_SHELL_RENDER_STATS=1 \
  RUST_LOG=info \
  timeout "${TIMEOUT_SECONDS}s" "${RUNNER[@]}" 2>&1 | tee "${LOG_FILE}"
else
  MERIDIAN_DRM_TIMING=1 \
  MERIDIAN_DIRTY_STATS=1 \
  MERIDIAN_SHELL_RENDER_STATS=1 \
  RUST_LOG=info \
  "${RUNNER[@]}" 2>&1 | tee "${LOG_FILE}"
fi
run_exit="${PIPESTATUS[0]}"
set -e

echo
echo "[smoke-drm] summary (grep):"
grep -E "GL Vendor|GL Renderer|drm api selected|drm mode selected|drm timing summary|dirty reasons|shell render summary|too slow|lagging|error|warn" "${LOG_FILE}" || true

echo "[smoke-drm] run exit code: ${run_exit}"
exit "${run_exit}"
