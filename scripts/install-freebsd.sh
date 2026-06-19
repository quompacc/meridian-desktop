#!/bin/sh
# Turnkey FreeBSD installer for Meridian.
#
# Installs the compositor/shell/login/lock/portal binaries, themes and PAM
# stacks, registers the rc.d services, and configures the system so the desktop
# comes up at boot — no hand-written scripts required.
#
# Quick start (plug and play):
#   scripts/install-deps-freebsd.sh all
#   scripts/install-freebsd.sh --build --enable-boot --user alice
#   reboot
#
# What it does:
#   * builds (--build) and installs binaries + themes + PAM + rc.d
#   * enables and starts the prerequisites: seatd and dbus
#   * sets the GPU KMS module in kld_list (autodetected, or --gpu)
#   * with --enable-boot: enables the login manager at boot, switches networking
#     to background dhclient, and loads the GPU module now
#
# Recovery: every boot-affecting step is gated behind --enable-boot. Always keep
# an SSH session open the first time you enable boot; recover with:
#   service meridian_login stop
set -eu

PREFIX="/usr/local"
BUILD=0
ENABLE_BOOT=0
QUIET_BOOT=0
GPU="auto"
DESK_USER=""

usage() {
	cat <<EOF
Usage: scripts/install-freebsd.sh [options]

  --build           cargo build --release --workspace before installing
  --enable-boot     enable the login manager + prerequisites at boot
  --quiet-boot      also mute the console for a silent boot (best-effort)
  --user NAME       desktop user (owns appearance state; default: \$SUDO_USER)
  --gpu DRIVER      KMS module: auto|intel|amd|none (default: auto-detect)
  --prefix PATH     install prefix (default: /usr/local)
  -h, --help        show this help
EOF
}

while [ $# -gt 0 ]; do
	case "$1" in
		--build) BUILD=1; shift ;;
		--enable-boot) ENABLE_BOOT=1; shift ;;
		--quiet-boot) QUIET_BOOT=1; shift ;;
		--user) DESK_USER="${2:?missing value for --user}"; shift 2 ;;
		--gpu) GPU="${2:?missing value for --gpu}"; shift 2 ;;
		--prefix) PREFIX="${2:?missing value for --prefix}"; shift 2 ;;
		-h|--help) usage; exit 0 ;;
		*) echo "install-freebsd: unknown option: $1" >&2; usage >&2; exit 2 ;;
	esac
done

if [ "$(uname -s)" != "FreeBSD" ]; then
	echo "install-freebsd: this script targets FreeBSD; use scripts/install-local.sh on Linux" >&2
	exit 2
fi

case "${GPU}" in
	auto|intel|amd|none) ;;
	*) echo "install-freebsd: --gpu must be auto|intel|amd|none" >&2; exit 2 ;;
esac

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
	SUDO="sudo"
fi

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/.." && pwd)
cd "${REPO_ROOT}"

# Resolve the desktop user (owns /var/lib/meridian and becomes meridian_user).
if [ -z "${DESK_USER}" ]; then
	DESK_USER="${SUDO_USER:-$(id -un)}"
fi
if ! id "${DESK_USER}" >/dev/null 2>&1; then
	echo "install-freebsd: user '${DESK_USER}' does not exist (set --user)" >&2
	exit 2
fi

echo "install-freebsd: prefix=${PREFIX} user=${DESK_USER} gpu=${GPU} enable_boot=${ENABLE_BOOT}"

# ---------------------------------------------------------------------------
# 1. Build
# ---------------------------------------------------------------------------
if [ "${BUILD}" -eq 1 ]; then
	# pkg installs libraries under ${PREFIX}/lib, which the linker does not
	# search by default. (XKB_DEFAULT_RULES=evdev matters at runtime, not build;
	# the rc.d service sets it.)
	echo "install-freebsd: building release workspace (this takes a while)..."
	LIBRARY_PATH="${PREFIX}/lib" \
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

# ---------------------------------------------------------------------------
# 2. Binaries, themes, appearance state
# ---------------------------------------------------------------------------
for bin in meridian meridian-shell meridian-login meridian-lock \
	meridian-portal meridian-polkit-agent; do
	require_file "target/release/${bin}"
	${SUDO} install -m 0755 "target/release/${bin}" "${bindir}/${bin}"
done
echo "install-freebsd: installed binaries to ${bindir}"

${SUDO} install -d "${datadir}/meridian/themes"
for theme in themes/*; do
	[ -d "${theme}" ] || continue
	name=$(basename "${theme}")
	${SUDO} install -d "${datadir}/meridian/themes/${name}"
	${SUDO} cp -a "${theme}/." "${datadir}/meridian/themes/${name}/"
done
echo "install-freebsd: installed themes to ${datadir}/meridian/themes"

# Appearance state dir, owned by the desktop user.
${SUDO} install -d -o "${DESK_USER}" -g "$(id -gn "${DESK_USER}")" \
	-m 0755 /var/lib/meridian

# ---------------------------------------------------------------------------
# 3. PAM stacks + rc.d services
# ---------------------------------------------------------------------------
# FreeBSD PAM stacks for the login manager (pam_unix; no pam_systemd/pam_u2f).
${SUDO} install -m 0644 packaging/pam/freebsd/meridian-login /etc/pam.d/meridian-login
${SUDO} install -m 0644 packaging/pam/freebsd/meridian-login-password \
	/etc/pam.d/meridian-login-password
echo "install-freebsd: installed PAM stacks to /etc/pam.d"

${SUDO} install -m 0755 packaging/rc.d/meridian "${rcdir}/meridian"
${SUDO} install -m 0755 packaging/rc.d/meridian-login "${rcdir}/meridian-login"
${SUDO} install -m 0755 packaging/rc.d/meridian_quiet "${rcdir}/meridian_quiet"
echo "install-freebsd: installed rc.d services to ${rcdir}"

# ---------------------------------------------------------------------------
# 4. Prerequisite services: seatd + dbus (safe to enable/start always)
# ---------------------------------------------------------------------------
# The compositor seats itself through libseat/seatd and needs a session bus.
# Both are harmless background daemons, so enable + start them unconditionally.
${SUDO} sysrc seatd_enable=YES >/dev/null
${SUDO} sysrc dbus_enable=YES  >/dev/null
${SUDO} service seatd start >/dev/null 2>&1 || true
${SUDO} service dbus  start >/dev/null 2>&1 || true
# The desktop user must be in the 'video' group to open /dev/dri/cardN, and in
# the seatd group/'operator' so libseat can grant the seat.
${SUDO} pw groupmod video -m "${DESK_USER}" 2>/dev/null || true
${SUDO} pw groupmod operator -m "${DESK_USER}" 2>/dev/null || true
echo "install-freebsd: enabled seatd + dbus; added ${DESK_USER} to video/operator"

# ---------------------------------------------------------------------------
# 5. GPU KMS module
# ---------------------------------------------------------------------------
gpu_module=""
case "${GPU}" in
	intel) gpu_module="i915kms" ;;
	amd)   gpu_module="amdgpu" ;;
	none)  gpu_module="" ;;
	auto)
		# Read the vendor off the VGA/display PCI stanza. Match both the vendor
		# name (when the pciids database is installed) and the raw vendor ID
		# (Intel 0x8086, AMD/ATI 0x1002, NVIDIA 0x10de) since pciids is optional.
		_vga=$(pciconf -lv 2>/dev/null | grep -A4 -E '^vgapci|class=0x030000' \
			| grep -i 'vendor' | head -1 || true)
		case "${_vga}" in
			*Intel*|*vendor=0x8086*)
				gpu_module="i915kms" ;;
			*AMD*|*ATI*|*vendor=0x1002*)
				gpu_module="amdgpu" ;;
			*NVIDIA*|*nVidia*|*vendor=0x10de*)
				# nouveau is unreliable on FreeBSD; leave the driver to the user.
				gpu_module=""
				echo "install-freebsd: NVIDIA GPU detected — set the driver manually" >&2 ;;
			*)
				gpu_module="i915kms"
				echo "install-freebsd: could not detect GPU; defaulting to i915kms (override with --gpu)" >&2 ;;
		esac ;;
esac

if [ -n "${gpu_module}" ]; then
	# Persist in kld_list without clobbering existing modules.
	_cur=$(${SUDO} sysrc -n kld_list 2>/dev/null || echo "")
	case " ${_cur} " in
		*" ${gpu_module} "*) : ;;  # already present
		*) ${SUDO} sysrc kld_list="$(echo "${_cur} ${gpu_module}" | sed 's/^ *//')" >/dev/null ;;
	esac
	echo "install-freebsd: GPU KMS module = ${gpu_module} (persisted in kld_list)"
fi

# ---------------------------------------------------------------------------
# 6. Boot enablement (gated)
# ---------------------------------------------------------------------------
if [ "${ENABLE_BOOT}" -eq 1 ]; then
	# Login manager replaces the direct-compositor service — enable exactly one.
	${SUDO} sysrc meridian_login_enable=YES >/dev/null
	${SUDO} sysrc meridian_enable=NO        >/dev/null
	# Background dhclient so networking does not block / spam the boot.
	${SUDO} sysrc background_dhclient=YES   >/dev/null
	# Load the GPU module now so a test start works before the next reboot.
	if [ -n "${gpu_module}" ]; then
		${SUDO} kldload "${gpu_module}" 2>/dev/null || true
	fi
	if [ "${QUIET_BOOT}" -eq 1 ]; then
		${SUDO} sysrc meridian_quiet_enable=YES >/dev/null
	fi
	echo "install-freebsd: enabled meridian_login at boot (background_dhclient=YES)"
	cat <<EOF

install-freebsd: DONE — boot enabled.
  Keep this SSH session open and verify recovery before rebooting:
    service meridian_login start     # bring the greeter up now
    service meridian_login stop      # recover

  Then reboot to confirm boot-to-greeter.
EOF
else
	cat <<EOF

install-freebsd: DONE — installed but boot NOT enabled.
  Test the greeter ad-hoc (keep an SSH session open to recover):
    service meridian_login onestart
    service meridian_login onestop

  To enable boot-to-greeter, re-run with --enable-boot, or:
    sysrc meridian_login_enable=YES
    sysrc meridian_enable=NO
    sysrc background_dhclient=YES
EOF
fi
