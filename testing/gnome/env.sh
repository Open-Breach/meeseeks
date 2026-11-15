#!/bin/bash
# Brings up a private headless GNOME and runs one script inside it: a session
# bus, PipeWire with WirePlumber, Mutter with one virtual monitor for a
# screen, and the real xdg-desktop-portal with its GNOME backend. It uses
# nothing of the machine's own session.
#
# Usage: env.sh <script> [args...]
#
# SCREEN=<width>x<height> is the screen's size in pixels; the default is
# 1600x1000. SCALE=<n> has Mutter scale every monitor by that. LAYOUT=logical
# has it lay monitors out in scaled units, as it does with fractional scaling
# switched on, instead of in pixels.
set -u
T=$(cd "$(dirname "$0")/.." && pwd)
export T
mkdir -p "$T/out"

XDG_RUNTIME_DIR=$(mktemp -d /tmp/meeseeks-test.XXXXXX) || exit 1
export XDG_RUNTIME_DIR
unset WAYLAND_DISPLAY DISPLAY SWAYSOCK PIPEWIRE_REMOTE PIPEWIRE_RUNTIME_DIR
export XDG_CURRENT_DESKTOP=GNOME XDG_SESSION_TYPE=wayland
# Settings, the app's and the desktop's, go where the run's other files are.
export XDG_CONFIG_HOME=$XDG_RUNTIME_DIR/config
export XDG_DATA_HOME=$XDG_RUNTIME_DIR/data XDG_CACHE_HOME=$XDG_RUNTIME_DIR/cache
export GSETTINGS_BACKEND=keyfile
mkdir -p "$XDG_CONFIG_HOME/glib-2.0/settings"
{
  echo "[org/gnome/desktop/interface]"
  echo "scaling-factor=uint32 ${SCALE:-0}"
  echo "[org/gnome/mutter]"
  [ "${LAYOUT:-}" = logical ] && echo "experimental-features=['scale-monitor-framebuffer']"
} > "$XDG_CONFIG_HOME/glib-2.0/settings/keyfile"

dbus-run-session -- bash -c '
  pipewire > "$T/out/pipewire.log" 2>&1 &
  sleep 1
  wireplumber > "$T/out/wireplumber.log" 2>&1 &
  sleep 1
  # There is no GPU: Mutter and GTK draw in software.
  LIBGL_ALWAYS_SOFTWARE=1 mutter --headless --wayland --no-x11 \
    --virtual-monitor "${SCREEN:-1600x1000}" > "$T/out/mutter.log" 2>&1 &
  for i in $(seq 100); do
    [ -S "$XDG_RUNTIME_DIR/wayland-0" ] && break
    sleep 0.1
  done
  if [ ! -S "$XDG_RUNTIME_DIR/wayland-0" ]; then
    echo "mutter did not come up; see $T/out/mutter.log" >&2
    exit 1
  fi
  export WAYLAND_DISPLAY=wayland-0
  export GSK_RENDERER=${GSK_RENDERER:-cairo}
  # The backend first, so that the portal finds it running and does not have
  # the bus start one without this environment.
  /usr/libexec/xdg-desktop-portal-gnome > "$T/out/portal-gnome.log" 2>&1 &
  sleep 1
  /usr/libexec/xdg-desktop-portal > "$T/out/portal.log" 2>&1 &
  sleep 2
  "$@"
  status=$?
  pkill -P $$ > /dev/null 2>&1
  exit $status
' env.sh "$@"
status=$?

case "$XDG_RUNTIME_DIR" in
  /tmp/meeseeks-test.*)
    # The portal mounts its documents here.
    fusermount3 -u "$XDG_RUNTIME_DIR/doc" 2> /dev/null || fusermount -u "$XDG_RUNTIME_DIR/doc" 2> /dev/null
    rm -rf -- "$XDG_RUNTIME_DIR" 2> /dev/null ;;
esac
exit $status
