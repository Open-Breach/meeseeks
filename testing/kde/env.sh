#!/bin/bash
# Brings up a private headless Plasma and runs one script inside it: a
# session bus, PipeWire with WirePlumber, KWin with one virtual output for a
# screen, and the real xdg-desktop-portal with its KDE backend. It uses
# nothing of the machine's own session.
#
# Usage: env.sh <script> [args...]
#
# SCREEN=<width>x<height> is the screen's size; the default is 1600x1000.
# KDE_BACKEND=<path> is the portal's KDE backend to run instead of the
# root's own, for one built from another release's source.
# SHELL_TOO=1 starts plasmashell as well, for the tests that ask it things.
set -u
T=$(cd "$(dirname "$0")/.." && pwd)
export T
mkdir -p "$T/out"

XDG_RUNTIME_DIR=$(mktemp -d /tmp/meeseeks-test.XXXXXX) || exit 1
export XDG_RUNTIME_DIR
unset WAYLAND_DISPLAY DISPLAY SWAYSOCK PIPEWIRE_REMOTE PIPEWIRE_RUNTIME_DIR
export XDG_CURRENT_DESKTOP=KDE XDG_SESSION_TYPE=wayland KDE_FULL_SESSION=true KDE_SESSION_VERSION=6
export LANG=C.UTF-8
export XDG_CONFIG_HOME=$XDG_RUNTIME_DIR/config
export XDG_DATA_HOME=$XDG_RUNTIME_DIR/data XDG_CACHE_HOME=$XDG_RUNTIME_DIR/cache
mkdir -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME"
screen=${SCREEN:-1600x1000}
export SCREEN_WIDTH=${screen%x*} SCREEN_HEIGHT=${screen#*x}

# There is no GPU: KWin, Qt and GTK draw in software.
export LIBGL_ALWAYS_SOFTWARE=1 QT_QPA_PLATFORM=wayland

# KWin asks for the portal as it comes up, before there is a Wayland display
# for the portal's KDE backend to show its dialogs on, and the bus would
# start both without one. For the bus of this run the portal is something
# that cannot be started; it is started by hand once KWin is there.
mkdir -p "$XDG_RUNTIME_DIR/dbus-1/services"
printf '[D-BUS Service]\nName=%s\nExec=/bin/false\n' org.freedesktop.portal.Desktop \
  > "$XDG_RUNTIME_DIR/dbus-1/services/org.freedesktop.portal.Desktop.service"

dbus-run-session -- bash -c '
  pipewire > "$T/out/pipewire.log" 2>&1 &
  sleep 1
  wireplumber > "$T/out/wireplumber.log" 2>&1 &
  sleep 1
  # The two switches let a test work the keyboard and pointer and take
  # pictures of the screen, which KWin otherwise keeps for programs it knows.
  # Not WAYLAND_DISPLAY: with it KWin would look for a compositor to run in.
  KWIN_WAYLAND_NO_PERMISSION_CHECKS=1 KWIN_SCREENSHOT_NO_PERMISSION_CHECKS=1 \
    kwin_wayland --virtual --width "$SCREEN_WIDTH" --height "$SCREEN_HEIGHT" \
    --no-lockscreen --socket wayland-0 > "$T/out/kwin.log" 2>&1 &
  for i in $(seq 300); do
    [ -S "$XDG_RUNTIME_DIR/wayland-0" ] && break
    sleep 0.1
  done
  if [ ! -S "$XDG_RUNTIME_DIR/wayland-0" ]; then
    echo "kwin did not come up; see $T/out/kwin.log" >&2
    exit 1
  fi
  export WAYLAND_DISPLAY=wayland-0
  export GSK_RENDERER=${GSK_RENDERER:-cairo}
  sleep 2
  # The backend first, so that the portal finds it running.
  backend=${KDE_BACKEND:-$(ls /usr/lib/*/libexec/xdg-desktop-portal-kde /usr/libexec/xdg-desktop-portal-kde 2>/dev/null | head -1)}
  "$backend" > "$T/out/portal-kde.log" 2>&1 &
  for i in $(seq 120); do
    gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner org.freedesktop.impl.portal.desktop.kde 2>/dev/null | grep -q true && break
    sleep 0.5
  done
  /usr/libexec/xdg-desktop-portal > "$T/out/portal.log" 2>&1 &
  for i in $(seq 120); do
    gdbus call --session --dest org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop \
      --method org.freedesktop.DBus.Properties.Get org.freedesktop.portal.ScreenCast AvailableSourceTypes \
      > /dev/null 2>&1 && break
    sleep 0.5
  done
  if [ -n "${SHELL_TOO:-}" ]; then
    plasmashell > "$T/out/plasmashell.log" 2>&1 &
    sleep 6
  fi
  "$@"
  status=$?
  pkill -P $$ > /dev/null 2>&1
  pkill -x kwin_wayland > /dev/null 2>&1
  exit $status
' env.sh "$@"
status=$?

case "$XDG_RUNTIME_DIR" in
  /tmp/meeseeks-test.*)
    fusermount3 -u "$XDG_RUNTIME_DIR/doc" 2> /dev/null || fusermount -u "$XDG_RUNTIME_DIR/doc" 2> /dev/null
    rm -rf -- "$XDG_RUNTIME_DIR" 2> /dev/null ;;
esac
exit $status
