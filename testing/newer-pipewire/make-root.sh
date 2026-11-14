#!/bin/bash
# Builds an Ubuntu root with a newer PipeWire, GStreamer and GTK than the
# machine itself has, for run.sh and consumer.sh to chroot into.
#
# Usage: make-root.sh <directory> [suite]
#
# The suite defaults to stonking, Ubuntu 26.10, which had PipeWire 1.6.8 when
# this was written. Needs root, debootstrap and a few minutes.
set -eu
root=${1:?usage: make-root.sh <directory> [suite]}
suite=${2:-stonking}

if [ -e "$root" ] && [ -n "$(ls -A "$root" 2>/dev/null)" ]; then
  echo "$root exists and is not empty" >&2
  exit 1
fi
mkdir -p "$root"
root=$(cd "$root" && pwd)

# An older debootstrap does not know newer suite names; every Ubuntu suite is
# built by the same script, which it has under the name gutsy.
script=/usr/share/debootstrap/scripts/$suite
[ -e "$script" ] || script=/usr/share/debootstrap/scripts/gutsy
debootstrap --variant=minbase --components=main,universe "$suite" "$root" \
  http://archive.ubuntu.com/ubuntu "$script"

mount -t proc proc "$root/proc"
mount --bind /dev "$root/dev"
trap 'umount "$root/proc" "$root/dev"' EXIT

# The PipeWire stack and what compiles the test programs against it; Python
# with the bindings the mock portal and the consumer scripts use; GTK and the
# GTK 4 sink for the app itself, with a font so the button has a label.
chroot "$root" /usr/bin/env DEBIAN_FRONTEND=noninteractive \
  apt-get install -y --no-install-recommends \
  pipewire pipewire-bin wireplumber gstreamer1.0-pipewire gstreamer1.0-tools \
  gstreamer1.0-plugins-base libpipewire-0.3-dev gcc libc6-dev pkg-config \
  dbus dbus-user-session procps \
  python3-gi gir1.2-gstreamer-1.0 python3-dbus \
  libgtk-4-1 gstreamer1.0-gtk4 fonts-dejavu-core

chroot "$root" pipewire --version
