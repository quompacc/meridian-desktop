#!/usr/bin/env bash
set -euo pipefail

MODE="all"
MANAGER="auto"

usage() {
  cat <<EOF_USAGE
Usage: scripts/install-deps.sh [options] [all|build|runtime|hardware-test]

Installs Meridian dependencies on Arch/pacman and apt-based systems.

Options:
  --manager auto|pacman|apt  Package manager to use (default: auto)
  -h, --help                 Show this help

Modes:
  build          Rust/C build headers needed for cargo build/test
  runtime        Packages needed by an installed Meridian session
  hardware-test  Runtime extras useful for real DRM/input/portal validation
  all            build + runtime + hardware-test (default)

Examples:
  scripts/install-deps.sh --manager pacman all
  scripts/install-deps.sh all
  scripts/install-deps.sh build
EOF_USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --manager)
      MANAGER="${2:?missing value for --manager}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    all|build|runtime|hardware-test)
      MODE="$1"
      shift
      ;;
    *)
      echo "install-deps: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "${MODE}" in
  all|build|runtime|hardware-test) ;;
  *)
    echo "install-deps: invalid mode: ${MODE}" >&2
    usage >&2
    exit 2
    ;;
esac

case "${MANAGER}" in
  auto|pacman|apt) ;;
  *)
    echo "install-deps: invalid package manager: ${MANAGER}" >&2
    usage >&2
    exit 2
    ;;
esac

if [[ "${MANAGER}" == "auto" ]]; then
  if command -v pacman >/dev/null 2>&1; then
    MANAGER="pacman"
  elif command -v apt-get >/dev/null 2>&1; then
    MANAGER="apt"
  else
    echo "install-deps: could not detect pacman or apt-get; pass --manager explicitly or install packages manually" >&2
    exit 2
  fi
fi

SUDO=()
if [[ "${EUID}" -ne 0 ]]; then
  SUDO=(sudo)
fi

packages=()
add_packages() {
  packages+=("$@")
}

case "${MANAGER}" in
  pacman)
    build_packages=(
      base-devel
      pkgconf
      rustup
      clang
      pam
      libseat
      systemd-libs
      fontconfig
      freetype2
      pixman
      wayland
      libxkbcommon
      libinput
      mesa
      libglvnd
      libdrm
    )

    runtime_packages=(
      dbus
      networkmanager
      breeze                # Breeze_Light cursor theme (theme.toml default)
      papirus-icon-theme    # Papirus / Papirus-Dark icon set (theme.toml default)
      xkeyboard-config
      ttf-dejavu
      noto-fonts
      inter-font            # the central UI font (theme.toml ui = "Inter 11")
      xdg-utils
      python-gobject
      gtk3
      xdg-desktop-portal
      polkit
      pam-u2f
      cups
      pipewire
      pipewire-pulse        # PulseAudio compat so apps + the audio OSD get sound
      pipewire-alsa         # ALSA compat
      wireplumber
      xorg-xwayland
    )

    hardware_packages=(
      libinput
      drm_info
      pciutils
      usbutils
      mesa-utils
      libnotify
      jq
    )
    ;;
  apt)
    if ! command -v apt-get >/dev/null 2>&1; then
      echo "install-deps: apt-get not found" >&2
      exit 2
    fi

    build_packages=(
      build-essential
      pkg-config
      libpam0g-dev
      libclang-dev
      libseat-dev
      libudev-dev
      libfontconfig-dev
      libfreetype-dev
      libpixman-1-dev
      libwayland-dev
      libxkbcommon-dev
      libinput-dev
      libegl-dev
      libgles-dev
      libgbm-dev
      libdrm-dev
    )

    runtime_packages=(
      dbus-user-session
      network-manager
      breeze-cursor-theme
      xkb-data
      fonts-dejavu
      fonts-noto-core
      xdg-utils
      python3-gi
      gir1.2-gtk-3.0
      xdg-desktop-portal
      polkitd
      libpam-u2f
      cups-client
      wireplumber
    )

    hardware_packages=(
      libinput-tools
      drm-info
      pciutils
      usbutils
      mesa-utils
      libnotify-bin
      jq
    )
    ;;
esac

case "${MODE}" in
  build)
    add_packages "${build_packages[@]}"
    ;;
  runtime)
    add_packages "${runtime_packages[@]}"
    ;;
  hardware-test)
    add_packages "${hardware_packages[@]}"
    ;;
  all)
    add_packages "${build_packages[@]}" "${runtime_packages[@]}" "${hardware_packages[@]}"
    ;;
esac

unique=()
for pkg in "${packages[@]}"; do
  seen=0
  for existing in "${unique[@]}"; do
    if [[ "${existing}" == "${pkg}" ]]; then
      seen=1
      break
    fi
  done
  if [[ "${seen}" -eq 0 ]]; then
    unique+=("${pkg}")
  fi
done

echo "install-deps: installing ${#unique[@]} package(s) via ${MANAGER}"
case "${MANAGER}" in
  pacman)
    "${SUDO[@]}" pacman -Syu --needed --noconfirm "${unique[@]}"
    ;;
  apt)
    "${SUDO[@]}" env DEBIAN_FRONTEND=noninteractive apt-get update
    "${SUDO[@]}" env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "${unique[@]}"
    ;;
esac
