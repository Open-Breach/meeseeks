#!/bin/bash
# Brings up a private test desktop and runs one script inside it: a session
# bus, PipeWire with WirePlumber, and a headless sway with software rendering.
# It uses nothing of the machine's own session.
#
# Usage: env.sh <script> [args...]
set -u
T=$(cd "$(dirname "$0")" && pwd)
export T
mkdir -p "$T/out"

# Its own runtime directory, under a short path: the address of a unix socket
# can only be about a hundred bytes long.
XDG_RUNTIME_DIR=$(mktemp -d /tmp/meeseeks-test.XXXXXX) || exit 1
export XDG_RUNTIME_DIR
unset WAYLAND_DISPLAY DISPLAY SWAYSOCK PIPEWIRE_REMOTE PIPEWIRE_RUNTIME_DIR

dbus-run-session -- bash -c '
  pipewire > "$T/out/pipewire.log" 2>&1 &
  sleep 1
  wireplumber > "$T/out/wireplumber.log" 2>&1 &
  sleep 1
  WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    sway -c "$T/sway.conf" > "$T/out/sway.log" 2>&1 &
  for i in $(seq 50); do
    socket=$(ls "$XDG_RUNTIME_DIR" | grep -m1 "^wayland-[0-9]*$") && break
    sleep 0.1
  done
  if [ -z "${socket:-}" ]; then
    echo "sway did not come up; see $T/out/sway.log" >&2
    exit 1
  fi
  export WAYLAND_DISPLAY=$socket
  export SWAYSOCK=$(ls "$XDG_RUNTIME_DIR"/sway-ipc.*.sock 2>/dev/null | head -1)
  "$@"
  status=$?
  swaymsg exit > /dev/null 2>&1
  pkill -P $$ > /dev/null 2>&1
  exit $status
' env.sh "$@"
status=$?

case "$XDG_RUNTIME_DIR" in
  /tmp/meeseeks-test.*) rm -rf -- "$XDG_RUNTIME_DIR" ;;
esac
exit $status
