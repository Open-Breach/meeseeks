#!/bin/bash
# Runs a test on the headless Plasma inside a root that has KWin, the KDE
# portal and what newer-pipewire/make-root.sh installs. The repository is
# mounted into the root; the app is the binary built on this machine.
#
# Usage: in-root.sh <root> <test under testing/> [args...]
#   in-root.sh /var/tmp/kde-new kde/tests/loop.sh
#
# SCREEN is passed on to kde/env.sh, APP, GSK_RENDERER and RUST_LOG to the
# test. Needs root.
#
# On top of what make-root.sh puts into the root, it needs:
#   chroot <root> apt-get install -y --no-install-recommends kwin-wayland \
#     xdg-desktop-portal xdg-desktop-portal-kde plasma-workspace breeze \
#     qml6-module-org-kde-desktop gir1.2-gtk-4.0 gstreamer1.0-plugins-good \
#     libglib2.0-bin fuse3
set -u
T=$(cd "$(dirname "$0")/.." && pwd)
usage="usage: in-root.sh <root> <test under testing/> [args...]"
ROOT=${1:?$usage}; TEST=${2:?$usage}; shift 2
if [ ! -x "$ROOT/usr/bin/kwin_wayland" ]; then
  echo "$ROOT has no kwin_wayland; see the head of this script for what to install" >&2
  exit 1
fi
ROOT=$(cd "$ROOT" && pwd)
IN_ROOT=/opt/meeseeks

mounted=()
mount_into_root() {  # <type or --bind> <source> <path in the root>
  mountpoint -q "$ROOT$3" && return
  mkdir -p "$ROOT$3"
  if [ "$1" = --bind ]; then mount --bind "$2" "$ROOT$3"; else mount -t "$1" "$2" "$ROOT$3"; fi
  mounted+=("$ROOT$3")
}
leave_root() {
  for ((i = ${#mounted[@]} - 1; i >= 0; i--)); do
    umount "${mounted[i]}" 2>/dev/null || umount -l "${mounted[i]}" 2>/dev/null \
      || echo "could not unmount ${mounted[i]}" >&2
  done
}
trap leave_root EXIT
mount_into_root proc proc /proc
mount_into_root sysfs sysfs /sys
mount_into_root --bind /dev /dev
mount_into_root --bind "$T/.." "$IN_ROOT"

chroot "$ROOT" /usr/bin/env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin HOME=/root \
  SCREEN="${SCREEN:-1600x1000}" \
  ${APP:+APP="$APP"} ${GSK_RENDERER:+GSK_RENDERER="$GSK_RENDERER"} ${RUST_LOG:+RUST_LOG="$RUST_LOG"} \
  ${SHELL_TOO:+SHELL_TOO="$SHELL_TOO"} \
  "$IN_ROOT/testing/kde/env.sh" "$IN_ROOT/testing/$TEST" "$@"
