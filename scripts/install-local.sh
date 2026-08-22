#!/usr/bin/env bash
set -euo pipefail

PREFIX="/usr/local"
BUILD=0
ENABLE_BOOT=0
WITH_BOOT_SPLASH=""
DESKTOP_USER="${SUDO_USER:-${USER}}"

usage() {
  cat <<'EOF'
Usage: scripts/install-local.sh [options]

Installs the Meridian binaries, PAM files, themes, portal metadata, and autostart
metadata from this checkout. It does not install OS packages; run
scripts/install-deps.sh first on Arch/pacman or apt-based systems.

Options:
  --build                 Run cargo build --release --workspace before install
  --prefix PATH           Install prefix for binaries/data (default: /usr/local)
  --desktop-user USER     User that owns /var/lib/meridian (default: sudo user)
  --enable-boot           Enable meridian-login.service and disable getty@tty1
  --bootsplash PATH       Also install sibling bootsplash checkout from PATH
  -h, --help              Show this help

Examples:
  scripts/install-local.sh --build
  scripts/install-local.sh --build --enable-boot --bootsplash ../bootsplash
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --build)
      BUILD=1
      shift
      ;;
    --prefix)
      PREFIX="${2:?missing value for --prefix}"
      shift 2
      ;;
    --desktop-user)
      DESKTOP_USER="${2:?missing value for --desktop-user}"
      shift 2
      ;;
    --enable-boot)
      ENABLE_BOOT=1
      shift
      ;;
    --bootsplash)
      WITH_BOOT_SPLASH="${2:?missing value for --bootsplash}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "install-local: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

SUDO=()
if [[ "${EUID}" -ne 0 ]]; then
  SUDO=(sudo)
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

if [[ "${BUILD}" -eq 1 ]]; then
  cargo build --release --workspace
fi

require_file() {
  if [[ ! -e "$1" ]]; then
    echo "install-local: missing required file: $1" >&2
    echo "install-local: run with --build or build the release workspace first" >&2
    exit 1
  fi
}

bindir="${PREFIX}/bin"
datadir="${PREFIX}/share"
libdir="${PREFIX}/lib"

binaries=(
  meridian
  meridian-shell
  meridian-login
  meridian-lock
  meridian-portal
  meridian-polkit-agent
)

for bin in "${binaries[@]}"; do
  require_file "target/release/${bin}"
  "${SUDO[@]}" install -Dm755 "target/release/${bin}" "${bindir}/${bin}"
done

if [[ "$(uname -s)" == "OpenBSD" ]]; then
  drm_bridge="libmeridian_xwayland_drm_bridge.so"
  require_file "target/release/${drm_bridge}"
  # The compositor resolves the bridge beside its own executable. Keep this
  # OpenBSD-only artifact out of global loader configuration.
  "${SUDO[@]}" install -Dm755 "target/release/${drm_bridge}" "${bindir}/${drm_bridge}"
fi
"${SUDO[@]}" install -Dm755 scripts/meridian-file-picker "${bindir}/meridian-file-picker"

"${SUDO[@]}" install -d "${datadir}/meridian/themes"
for theme in themes/*; do
  [[ -d "${theme}" ]] || continue
  name="$(basename "${theme}")"
  "${SUDO[@]}" install -d "${datadir}/meridian/themes/${name}"
  "${SUDO[@]}" cp -a "${theme}/." "${datadir}/meridian/themes/${name}/"
done

"${SUDO[@]}" install -d -o "${DESKTOP_USER}" -g "${DESKTOP_USER}" -m 0755 /var/lib/meridian
"${SUDO[@]}" install -Dm644 crates/meridian-login/config/meridian-login.service /etc/systemd/system/meridian-login.service
"${SUDO[@]}" install -Dm644 crates/meridian-login/config/meridian-login.pam /etc/pam.d/meridian-login
"${SUDO[@]}" install -Dm644 packaging/pam/meridian-login-password /etc/pam.d/meridian-login-password

install_template() {
  local src="$1"
  local dest="$2"
  local tmp
  tmp="$(mktemp)"
  sed "s#@PREFIX@#${PREFIX}#g" "${src}" > "${tmp}"
  "${SUDO[@]}" install -Dm644 "${tmp}" "${dest}"
  rm -f "${tmp}"
}

install_template packaging/xdg-autostart/meridian-polkit-agent.desktop /etc/xdg/autostart/meridian-polkit-agent.desktop
install_template packaging/dbus-1/services/org.freedesktop.impl.portal.desktop.meridian.service "${datadir}/dbus-1/services/org.freedesktop.impl.portal.desktop.meridian.service"
install_template packaging/systemd-user/meridian-portal.service "${libdir}/systemd/user/meridian-portal.service"
# meridian-session.target pulls graphical-session.target up at login (started by
# the shell) so xdg-desktop-portal + meridian-portal run and apps follow the
# theme. Without it the portal services never start in a Meridian session.
install_template packaging/systemd-user/meridian-session.target "${libdir}/systemd/user/meridian-session.target"
"${SUDO[@]}" install -Dm644 packaging/xdg-desktop-portal/portals/meridian.portal "${datadir}/xdg-desktop-portal/portals/meridian.portal"
"${SUDO[@]}" install -Dm644 packaging/xdg-desktop-portal/meridian-portals.conf "${datadir}/xdg-desktop-portal/meridian-portals.conf"

if [[ -n "${WITH_BOOT_SPLASH}" ]]; then
  boot_root="$(cd "${WITH_BOOT_SPLASH}" && pwd)"
  if [[ "${BUILD}" -eq 1 ]]; then
    cargo build --release --manifest-path "${boot_root}/Cargo.toml"
  fi
  require_file "${boot_root}/target/release/bootsplash"
  require_file "${boot_root}/systemd/bootsplash.service"
  "${SUDO[@]}" install -Dm755 "${boot_root}/target/release/bootsplash" "${bindir}/bootsplash"
  "${SUDO[@]}" install -Dm644 "${boot_root}/systemd/bootsplash.service" /etc/systemd/system/bootsplash.service
fi

"${SUDO[@]}" systemctl daemon-reload
if command -v systemctl >/dev/null 2>&1; then
  systemctl --user daemon-reload >/dev/null 2>&1 || true
fi

if [[ "${ENABLE_BOOT}" -eq 1 ]]; then
  "${SUDO[@]}" systemctl disable getty@tty1.service
  if [[ -n "${WITH_BOOT_SPLASH}" ]]; then
    "${SUDO[@]}" systemctl enable bootsplash.service
  fi
  "${SUDO[@]}" systemctl enable meridian-login.service
else
  cat <<'EOF'
install-local: installed files but did not enable the boot login service.
To enable after verifying recovery access:
  sudo systemctl disable getty@tty1.service
  sudo systemctl enable meridian-login.service
  # plus bootsplash.service if installed
EOF
fi

cat <<EOF
install-local: installed Meridian to ${PREFIX}
install-local: desktop user for /var/lib/meridian: ${DESKTOP_USER}
EOF
