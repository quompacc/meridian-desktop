#!/bin/ksh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)

if [ "$(uname -s)" != "OpenBSD" ]; then
    echo "restart-openbsd-shell.ksh is only for the OpenBSD hardware session." >&2
    exit 1
fi

compositor_pid=$(pgrep -xo meridian 2>/dev/null || true)
if [ -z "${compositor_pid}" ]; then
    echo "No running Meridian compositor; start scripts/smoke-drm.sh run first." >&2
    exit 1
fi

compositor_command=$(ps -o command= -p "${compositor_pid}")
case "${compositor_command}" in
    *target/release/meridian*) ;;
    *)
        echo "Refusing shell update: compositor is not running from target/release." >&2
        echo "Stop it and start scripts/smoke-drm.sh run to get a performance-valid session." >&2
        exit 1
        ;;
esac

export LIBRARY_PATH=${LIBRARY_PATH:-/usr/local/lib:/usr/X11R6/lib}
export RUSTFLAGS=${RUSTFLAGS:--L native=/usr/X11R6/lib}

cd "${REPO_ROOT}"
echo "[openbsd-shell] building optimized shell..."
cargo build --release -p meridian-shell

shell_pid=$(pgrep -xo meridian-shell 2>/dev/null || true)
if [ -n "${shell_pid}" ]; then
    echo "[openbsd-shell] stopping shell pid ${shell_pid}; compositor watchdog will restart it..."
    kill "${shell_pid}"
else
    echo "[openbsd-shell] shell is not running; compositor watchdog will start it."
fi

echo "[openbsd-shell] release shell ready: ${REPO_ROOT}/target/release/meridian-shell"
