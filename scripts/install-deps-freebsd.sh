#!/bin/sh
# Install Meridian's FreeBSD package dependencies via pkg(8). FreeBSD analogue of
# scripts/install-deps.sh (which covers pacman/apt only).
#
# Usage: scripts/install-deps-freebsd.sh [all|build|runtime|hardware-test]
set -eu

MODE="${1:-all}"

case "${MODE}" in
	all|build|runtime|hardware-test) ;;
	-h|--help)
		echo "Usage: $0 [all|build|runtime|hardware-test]"; exit 0 ;;
	*)
		echo "install-deps-freebsd: invalid mode: ${MODE}" >&2
		echo "Usage: $0 [all|build|runtime|hardware-test]" >&2
		exit 2 ;;
esac

if [ "$(uname -s)" != "FreeBSD" ]; then
	echo "install-deps-freebsd: this script targets FreeBSD" >&2
	exit 2
fi

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
	SUDO="sudo"
fi

# Build headers/toolchain. libudev-devd is the udev shim smithay's backend_udev
# links against; seatd provides libseat for backend_session_libseat.
build_pkgs="rust pkgconf seatd libudev-devd mesa-libs mesa-dri libdrm wayland \
	libxkbcommon libinput pixman freetype2 fontconfig libglvnd"

# Runtime: D-Bus, the Intel/AMD KMS modules (drm-kmod), an Xcursor theme (Breeze
# is not packaged on FreeBSD), fonts, and Xwayland for X11 clients.
runtime_pkgs="dbus drm-kmod bibata-cursor-theme dejavu noto-basic xwayland"

# Hardware-test extras for DRM/PCI/USB inspection.
hardware_pkgs="drm-kmod libinput"

case "${MODE}" in
	build)         pkgs="${build_pkgs}" ;;
	runtime)       pkgs="${runtime_pkgs}" ;;
	hardware-test) pkgs="${hardware_pkgs}" ;;
	all)           pkgs="${build_pkgs} ${runtime_pkgs} ${hardware_pkgs}" ;;
esac

echo "install-deps-freebsd: installing (${MODE}) via pkg"
# shellcheck disable=SC2086
${SUDO} pkg install -y ${pkgs}

cat <<'EOF'

install-deps-freebsd: done.
Set the default Rust toolchain if you use rustup instead of the pkg rust:
  (the pkg 'rust' already provides a stable rustc/cargo)

Graphics: load the KMS driver and persist it across reboots, e.g. Intel:
  sysrc kld_list+=i915kms && kldload i915kms
(AMD: amdgpu / radeonkms; older Intel: i915kms covers gen4+)
EOF
