#!/bin/bash
# Runs a test on the headless GNOME inside a root made by
# newer-pipewire/make-root.sh, so that it meets a newer GNOME and PipeWire
# than the machine itself has. The repository is mounted into the root; the
# app is the binary built on this machine, which works as long as the root's
# libraries are the newer ones.
#
# Usage: in-root.sh <root> <test under testing/> [args...]
#   in-root.sh /var/tmp/pw-root gnome/tests/input.sh click
#
# SCREEN, SCALE and LAYOUT are passed on to gnome/env.sh, MEESEEKS_TRY to the
# app. Needs root.
#
# On top of what make-root.sh puts into the root, it needs:
#   chroot <root> apt-get install -y --no-install-recommends mutter \
#     xdg-desktop-portal xdg-desktop-portal-gnome gir1.2-gtk-4.0 \
#     gstreamer1.0-plugins-good fuse3
set -u
T=$(cd "$(dirname "$0")/.." && pwd)
usage="usage: in-root.sh <root> <test under testing/> [args...]"
ROOT=${1:?$usage}; TEST=${2:?$usage}; shift 2
if [ ! -x "$ROOT/usr/bin/mutter" ]; then
  echo "$ROOT has no mutter; see the head of this script for what to install" >&2
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
  SCREEN="${SCREEN:-1600x1000}" SCALE="${SCALE:-0}" LAYOUT="${LAYOUT:-}" \
  ${MEESEEKS_TRY:+MEESEEKS_TRY="$MEESEEKS_TRY"} \
  "$IN_ROOT/testing/gnome/env.sh" "$IN_ROOT/testing/$TEST" "$@"
