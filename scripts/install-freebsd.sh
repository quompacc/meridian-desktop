#!/bin/sh
# FreeBSD installer for Meridian. Installs the compositor/shell/lock/portal
# binaries, themes, and the rc.d service from this checkout. It deliberately
# does NOT install the systemd units, pam_systemd/logind PAM stacks, or enable
# the boot session -- on FreeBSD the compositor seats itself via libseat/seatd,
# and boot-to-greeter still needs the logind-free meridian-login work.
#
# Install OS packages first with: scripts/install-deps-freebsd.sh
#
# Usage:
#   scripts/install-freebsd.sh [--build] [--prefix /usr/local]
set -eu

PREFIX="/usr/local"
BUILD=0

usage() {
	cat <<EOF
Usage: scripts/install-freebsd.sh [--build] [--prefix PATH]

  --build         cargo build --release --workspace before installing
  --prefix PATH   install prefix (default: /usr/local)
  -h, --help      show this help
EOF
}

while [ $# -gt 0 ]; do
	case "$1" in
		--build) BUILD=1; shift ;;
		--prefix) PREFIX="${2:?missing value for --prefix}"; shift 2 ;;
		-h|--help) usage; exit 0 ;;
		*) echo "install-freebsd: unknown option: $1" >&2; usage >&2; exit 2 ;;
	esac
done

if [ "$(uname -s)" != "FreeBSD" ]; then
	echo "install-freebsd: this script targets FreeBSD; use scripts/install-local.sh on Linux" >&2
	exit 2
fi

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
	SUDO="sudo"
fi

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/.." && pwd)
cd "${REPO_ROOT}"

if [ "${BUILD}" -eq 1 ]; then
	# pkg installs libraries under ${PREFIX}/lib, which the linker does not
	# search by default; libxkbcommon needs the xorg ruleset at build time too.
	LIBRARY_PATH="${PREFIX}/lib" XKB_DEFAULT_RULES=xorg \
		cargo build --release --workspace
fi

bindir="${PREFIX}/bin"
datadir="${PREFIX}/share"
rcdir="${PREFIX}/etc/rc.d"

require_file() {
	if [ ! -e "$1" ]; then
		echo "install-freebsd: missing $1; run with --build first" >&2
		exit 1
	fi
}

for bin in meridian meridian-shell meridian-login meridian-lock \
	meridian-portal meridian-polkit-agent; do
	require_file "target/release/${bin}"
	${SUDO} install -m 0755 "target/release/${bin}" "${bindir}/${bin}"
done

${SUDO} install -d "${datadir}/meridian/themes"
for theme in themes/*; do
	[ -d "${theme}" ] || continue
	name=$(basename "${theme}")
	${SUDO} install -d "${datadir}/meridian/themes/${name}"
	${SUDO} cp -a "${theme}/." "${datadir}/meridian/themes/${name}/"
done

# Appearance state dir, owned by the invoking (non-root) user when possible.
state_user="${SUDO_USER:-$(id -un)}"
${SUDO} install -d -o "${state_user}" -g "$(id -gn "${state_user}")" \
	-m 0755 /var/lib/meridian

# rc.d service (installed but NOT enabled; see the header of the script).
${SUDO} install -m 0755 packaging/rc.d/meridian "${rcdir}/meridian"

cat <<EOF
install-freebsd: installed Meridian to ${PREFIX}
install-freebsd: rc.d service at ${rcdir}/meridian (not enabled)

Next:
  service seatd enable && service seatd start
  service dbus  enable && service dbus  start
  sysrc kld_list+=i915kms && kldload i915kms     # Intel; amdgpu/radeonkms for AMD

Start a session (after verifying SSH recovery):
  service meridian onestart        # ad-hoc, ignores rc.conf
  # or, to start at boot:
  sysrc meridian_enable=YES
  sysrc meridian_user=<youruser>   # must be in the "video" group
EOF
