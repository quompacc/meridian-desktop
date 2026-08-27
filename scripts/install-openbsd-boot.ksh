#!/bin/ksh
# Install Meridian's native OpenBSD boot-to-greeter chain. Enabling the boot
# service is deliberately separate and requires --enable-boot.
set -eu

build=0
enable_boot=0
prefix=/usr/local

usage() {
	cat <<EOF
Usage: scripts/install-openbsd-boot.ksh [--build] [--enable-boot] [--prefix PATH]

  --build        build the release binaries before installing
  --enable-boot  enable meridian_login after recovery checks
  --prefix PATH  installation prefix (default: /usr/local)

This installer does not mute the OpenBSD console. Prove the greeter and
recovery path first; quiet-boot work is a separate, later step.
EOF
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--build) build=1; shift ;;
	--enable-boot) enable_boot=1; shift ;;
	--prefix) prefix="${2:?missing value for --prefix}"; shift 2 ;;
	-h|--help) usage; exit 0 ;;
	*) echo "install-openbsd-boot: unknown option: $1" >&2; usage >&2; exit 2 ;;
	esac
done

[[ "$(uname -s)" == OpenBSD ]] || {
	echo "install-openbsd-boot: this script targets OpenBSD" >&2
	exit 2
}

script_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "${script_dir}/.." && pwd)
cd "${repo_root}"

if [[ $(id -u) -eq 0 ]]; then
	doas_cmd=""
else
	doas_cmd=doas
	${doas_cmd} -n true >/dev/null 2>&1 || {
		echo "install-openbsd-boot: non-interactive doas is required" >&2
		exit 1
	}
fi

if [[ ${build} -eq 1 ]]; then
	echo "install-openbsd-boot: building release workspace..."
	env LIBRARY_PATH="${prefix}/lib:/usr/X11R6/lib" cargo build --release --workspace
fi

require_file() {
	[[ -f "$1" ]] || {
		echo "install-openbsd-boot: missing $1; run with --build" >&2
		exit 1
	}
}

for binary in meridian meridian-shell meridian-login meridian-lock \
	meridian-portal meridian-polkit-agent; do
	require_file "target/release/${binary}"
	${doas_cmd} install -m 0755 "target/release/${binary}" "${prefix}/bin/${binary}"
done

${doas_cmd} install -d -m 0755 "${prefix}/share/meridian/themes"
for theme in themes/*; do
	[[ -d "${theme}" ]] || continue
	name=${theme##*/}
	${doas_cmd} install -d -m 0755 "${prefix}/share/meridian/themes/${name}"
	${doas_cmd} cp -R "${theme}/." "${prefix}/share/meridian/themes/${name}/"
done

${doas_cmd} install -m 0755 packaging/openbsd/rc.d/meridian_login \
	/etc/rc.d/meridian_login
echo "install-openbsd-boot: binaries, themes and rc.d service installed"

if [[ ${enable_boot} -eq 0 ]]; then
	cat <<EOF

Boot remains disabled. Safe next step:
  doas /etc/rc.d/meridian_login -df start

Only after the greeter and SSH/tty recovery are proven:
  scripts/install-openbsd-boot.ksh --enable-boot
EOF
	exit 0
fi

${doas_cmd} rcctl check sshd >/dev/null || {
	echo "install-openbsd-boot: sshd is not running; refusing boot enablement" >&2
	exit 1
}
grep -Eq '^ttyC1[[:space:]].*[[:space:]]on[[:space:]]+secure' /etc/ttys || {
	echo "install-openbsd-boot: ttyC1 recovery getty is not enabled" >&2
	exit 1
}
grep -Eq '^ttyC4[[:space:]].*[[:space:]]off[[:space:]]+secure' /etc/ttys || {
	echo "install-openbsd-boot: ttyC4 must remain the getty-free graphical VT" >&2
	exit 1
}

backup="/etc/rc.conf.local.meridian-backup.$(date +%Y%m%d%H%M%S)"
${doas_cmd} cp -p /etc/rc.conf.local "${backup}"
${doas_cmd} rcctl enable seatd messagebus meridian_login

cat <<EOF

install-openbsd-boot: boot-to-greeter enabled.
Recovery:
  Hold either Control key while BOOT starts to skip /etc/boot.conf.
  Switch to ttyC1 and disable: doas rcctl disable meridian_login
  Or recover over SSH: doas rcctl stop meridian_login
  rc.conf.local backup: ${backup}

No console output was muted by this installer.
EOF
