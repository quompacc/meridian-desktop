#!/bin/sh
# Install Meridian's FreeBSD package dependencies via pkg(8). FreeBSD analogue of
# scripts/install-deps.sh (which covers pacman/apt only).
#
# Usage: scripts/install-deps-freebsd.sh [all|build|runtime|apps|hardware-test]
#
#   all            build + runtime + apps (default; everything for a desktop)
#   build          toolchain + headers to compile the workspace
#   runtime        libraries, KMS driver, session bus, icons, cursor, fonts
#   apps           the bundled default apps (terminal, file manager, browser)
#   hardware-test  extras for DRM/PCI/USB inspection on real hardware
set -eu

MODE="${1:-all}"

case "${MODE}" in
	all|build|runtime|apps|hardware-test) ;;
	-h|--help)
		echo "Usage: $0 [all|build|runtime|apps|hardware-test]"; exit 0 ;;
	*)
		echo "install-deps-freebsd: invalid mode: ${MODE}" >&2
		echo "Usage: $0 [all|build|runtime|apps|hardware-test]" >&2
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

# Runtime libraries and assets:
#   dbus               session bus (FreeBSD has no systemd user bus; GTK/Qt apps
#                      and Meridian's own notification services need one)
#   drm-kmod           Intel/AMD KMS modules (i915kms / amdgpu / radeonkms)
#   seatd              libseat — the compositor seats itself without logind
#   papirus-icon-theme matches Meridian's default icon theme (Papirus-Dark)
#   plasma6-breeze     provides the Breeze_Light Xcursor (Meridian's default
#                      cursor); pulls KDE deps — drop it for a lean install and
#                      Meridian falls back to its embedded cursor
#   xwayland           run X11 clients under the Wayland compositor
#   dejavu/noto-basic  baseline UI + fallback fonts
#   xdg-utils          xdg-open, used to resolve default-app handlers
runtime_pkgs="dbus drm-kmod seatd papirus-icon-theme plasma6-breeze xwayland \
	dejavu noto-basic xdg-utils"

# The default apps Meridian's launcher/panel expect (terminal, file manager,
# browser). chromium is intentionally omitted: its sandbox/GPU broker does not
# work on FreeBSD yet — firefox is the working default browser.
apps_pkgs="foot pcmanfm firefox"

# Hardware-test extras for DRM/PCI/USB inspection.
hardware_pkgs="drm-kmod libinput usbutils pciutils"

case "${MODE}" in
	build)         pkgs="${build_pkgs}" ;;
	runtime)       pkgs="${runtime_pkgs}" ;;
	apps)          pkgs="${apps_pkgs}" ;;
	hardware-test) pkgs="${hardware_pkgs}" ;;
	all)           pkgs="${build_pkgs} ${runtime_pkgs} ${apps_pkgs}" ;;
esac

echo "install-deps-freebsd: installing (${MODE}) via pkg"
# shellcheck disable=SC2086
${SUDO} pkg install -y ${pkgs}

cat <<'EOF'

install-deps-freebsd: done.

Next: build and install Meridian (sets up services + config in one step):
  scripts/install-freebsd.sh --build --enable-boot --user <youruser>

The pkg 'rust' already provides a stable rustc/cargo — no rustup needed.
EOF
