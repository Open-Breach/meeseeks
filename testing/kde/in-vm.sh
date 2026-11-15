#!/bin/bash
# Runs a test on the headless Plasma inside an emulated machine that boots
# the root in-root.sh uses.
#
# Usage: in-vm.sh <root> <test under testing/> [args...]
#   in-vm.sh /var/tmp/kde-new kde/tests/loop.sh
#
# Why a machine: KWin's virtual backend composites with OpenGL only where
# there is a DRM device, and its screen cast refuses the QPainter compositing
# it falls back to without one. A container has no DRM device and cannot
# load a kernel module for one. A machine with a kernel of its own can: it
# loads vgem, and Mesa renders on that in software.
#
# Needs qemu-system-x86_64 here (Ubuntu: qemu-system-x86) and, in the root,
# on top of what in-root.sh lists:
#   chroot <root> apt-get install -y --no-install-recommends linux-image-generic kmod
#
# The root is packed into <root>.img the first time, with vm-init.sh in it
# as the machine's first program. Remove that file to have the root packed
# again after it has changed. The machine sees the repository as it is, so
# the app is the binary built here, and logs and pictures land in out/.
#
# Where this machine has no KVM, QEMU translates every instruction and the
# desktop is slow. The KDE portal gives KWin three seconds to make a stream
# and fails the Start otherwise, so the screen is small and the tests wait
# three times as long: SCREEN defaults to 1100x700 and SLOW to 3. A Start can
# still miss the three seconds; kde/tests/input.sh then presses it again.
# LIMIT is the number of seconds after which the machine is switched off.
#
# SCREEN and SLOW are passed on to kde/env.sh and the tests, and so are APP
# (a binary inside the repository), GSK_RENDERER, RUST_LOG, KDE_BACKEND and
# SHELL_TOO. A further line for the machine's job can be given as SETUP.
set -u
T=$(cd "$(dirname "$0")/.." && pwd)
REPO=$(cd "$T/.." && pwd)
usage="usage: in-vm.sh <root> <test under testing/> [args...]"
ROOT=${1:?$usage}; TEST=${2:?$usage}; shift 2
ROOT=$(cd "$ROOT" && pwd) || exit 1
IMAGE=$ROOT.img
IN_VM=/opt/meeseeks

if ! command -v qemu-system-x86_64 > /dev/null; then
  echo "no qemu-system-x86_64 here" >&2
  exit 1
fi
if [ ! -e "$ROOT/boot/vmlinuz" ] || [ ! -x "$ROOT/usr/sbin/modprobe" ]; then
  echo "$ROOT has no kernel or no modprobe; see the head of this script for what to install" >&2
  exit 1
fi
app=$(realpath --relative-to="$REPO" "${APP:-$REPO/target/debug/meeseeks}")
case $app in
  ../*) echo "the app has to be inside the repository, which is all the machine sees of this one" >&2; exit 1 ;;
esac

if [ ! -e "$IMAGE" ]; then
  echo "packing $ROOT into $IMAGE"
  install -D -m 755 "$T/kde/vm-init.sh" "$ROOT/opt/vm-init.sh"
  megabytes=$(( $(du -sm --one-file-system "$ROOT" | cut -f1) * 3 / 2 + 1024 ))
  mke2fs -q -t ext4 -d "$ROOT" "$IMAGE" "${megabytes}M" || exit 1
fi

# What the machine does once it is up: vm-init.sh runs this and switches off.
mkdir -p "$T/out"
{
  echo "export SLOW=${SLOW:-3} SCREEN=${SCREEN:-1100x700}"
  printf 'export APP=%q\n' "$IN_VM/$app"
  for name in GSK_RENDERER RUST_LOG KDE_BACKEND SHELL_TOO; do
    [ -n "${!name:-}" ] && printf 'export %s=%q\n' "$name" "${!name}"
  done
  [ -n "${SETUP:-}" ] && echo "$SETUP"
  printf '%q %q' "$IN_VM/testing/kde/env.sh" "$IN_VM/testing/$TEST"
  [ $# -gt 0 ] && printf ' %q' "$@"
  echo
} > "$T/out/vm-job.sh"
: > "$T/out/vm-job.log"

# KVM where there is one; otherwise QEMU says so and translates.
timeout "${LIMIT:-1500}" qemu-system-x86_64 -machine q35,accel=kvm:tcg -cpu max -smp 4 -m 7G \
  -nographic -no-reboot \
  -kernel "$ROOT/boot/vmlinuz" \
  -append "root=/dev/vda rw console=ttyS0 init=/opt/vm-init.sh panic=-1 loglevel=3" \
  -drive "file=$IMAGE,format=raw,if=virtio,cache=unsafe" \
  -virtfs "local,path=$REPO,mount_tag=repo,security_model=none,id=repo" \
  -device virtio-rng-pci 2>&1 | tr -d '\r' | grep -e "^vm:" -e "kvm"
grep -v -e "^dbus-daemon" -e "a11y" -e "^$" "$T/out/vm-job.log"
