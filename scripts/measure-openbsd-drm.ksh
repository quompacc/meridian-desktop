#!/bin/ksh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
RESULT_DIR=${MERIDIAN_PERF_RESULT_DIR:-/tmp/meridian-perf-${STAMP}}
RUNTIME_DIR=${XDG_RUNTIME_DIR:-/tmp/meridian-runtime-$(id -u)}
BENCH_BIN=${RESULT_DIR}/x11-motion-bench

mkdir -p "${RESULT_DIR}" "${RUNTIME_DIR}"
chmod 700 "${RUNTIME_DIR}"

export XDG_RUNTIME_DIR=${RUNTIME_DIR}
export LD_LIBRARY_PATH=${LD_LIBRARY_PATH:-/usr/local/llvm20/lib}
export LIBCLANG_PATH=${LIBCLANG_PATH:-/usr/local/llvm20/lib}
export LIBRARY_PATH=${LIBRARY_PATH:-/usr/X11R6/lib:/usr/local/lib}

cc -O2 -I/usr/X11R6/include -L/usr/X11R6/lib \
    "${SCRIPT_DIR}/x11-motion-bench.c" -lX11 -o "${BENCH_BIN}"

if pgrep -x meridian >/dev/null; then
    echo "A Meridian process is already running; stop it before measuring." >&2
    exit 1
fi

RUN_PID=
stop_case() {
    if [ -n "${RUN_PID}" ]; then
        kill "${RUN_PID}" 2>/dev/null || true
        wait "${RUN_PID}" 2>/dev/null || true
    fi
    # dbus-run-session can exit before its children. Stop only the exact
    # processes started by this harness so seatd and the DRM device are free
    # before the next case begins.
    pkill -x meridian-shell 2>/dev/null || true
    pkill -x Xwayland 2>/dev/null || true
    pkill -x meridian 2>/dev/null || true
    RUN_PID=
    sleep 2
}

cleanup() {
    stop_case
}
trap cleanup EXIT INT TERM HUP

wait_for_xwayland() {
    log_file=$1
    attempts=0
    while [ "${attempts}" -lt 100 ]; do
        display=$(sed -n 's/.*XWayland ready on DISPLAY=\(:[0-9][0-9]*\).*/\1/p' "${log_file}" | tail -1)
        if [ -n "${display}" ]; then
            echo "${display}"
            return 0
        fi
        sleep 0.1
        attempts=$((attempts + 1))
    done
    return 1
}

run_case() {
    case_name=$1
    shell_mode=$2
    workload=$3
    compositor_log=${RESULT_DIR}/${case_name}.log
    workload_log=${RESULT_DIR}/${case_name}.phases

    echo "[perf] case=${case_name} shell=${shell_mode} workload=${workload}"
    if [ "${shell_mode}" = "off" ]; then
        shell_env=MERIDIAN_DRM_DISABLE_SHELL=1
    else
        shell_env=MERIDIAN_DRM_DISABLE_SHELL=0
    fi

    env "${shell_env}" MERIDIAN_DRM_TIMING=1 MERIDIAN_DIRTY_STATS=1 \
        MERIDIAN_SHELL_RENDER_STATS=0 RUST_LOG=info \
        dbus-run-session -- "${REPO_ROOT}/target/release/meridian" \
        >"${compositor_log}" 2>&1 &
    RUN_PID=$!

    display=$(wait_for_xwayland "${compositor_log}") || {
        echo "XWayland did not become ready for ${case_name}" >&2
        return 1
    }

    if [ "${workload}" = "x11" ]; then
        DISPLAY=${display} "${BENCH_BIN}" >"${workload_log}" 2>&1
    else
        date -u +%s >"${workload_log}"
        echo "idle-start" >>"${workload_log}"
        sleep 8
        date -u +%s >>"${workload_log}"
        echo "idle-end" >>"${workload_log}"
    fi

    stop_case
}

run_case native_idle on idle
run_case compositor_idle off idle
run_case x11_motion_resize on x11
run_case x11_motion_resize_no_shell off x11

echo "[perf] results=${RESULT_DIR}"
