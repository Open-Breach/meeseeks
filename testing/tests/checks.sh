#!/bin/bash
# The startup checks: with everything there the window opens as before; with
# something missing, a dialog lists every check and Close ends the app.
# Usage: checks.sh <ok|no-sink|no-portal|no-virtual|no-pipewire|x11>
#
#   ok           nothing is missing: no dialog, the main window
#   no-sink      gtk4paintablesink is not installed; this can only be staged
#                where the sink comes from build/plugins
#   no-portal    nothing answers as xdg-desktop-portal
#   no-virtual   the ScreenCast portal has no virtual source type
#   no-pipewire  the app is pointed at a PipeWire socket that is not there
#   x11          the app runs on sway's Xwayland, with the environment of an
#                X11 session; needs Xwayland
. "$T/lib.sh"
case "${1:-}" in
  ok | no-portal | no-pipewire | x11) ;;
  no-virtual) export MOCK_SOURCE_TYPES=3 ;;
  no-sink)
    if [ -z "${GST_PLUGIN_PATH:-}" ]; then
      echo "the sink is installed system-wide here; it cannot be hidden" >&2
      exit 1
    fi ;;
  *) sed -n '2,13p' "$0" >&2; exit 1 ;;
esac
[ "$1" = no-portal ] || start_portal "checks-$1"
[ "$1" = no-sink ] && unset GST_PLUGIN_PATH
[ "$1" = no-pipewire ] && export PIPEWIRE_REMOTE=pipewire-absent
app_env=()
if [ "$1" = x11 ]; then
  # sway tells the programs it starts where its Xwayland is.
  swaymsg -q exec "echo \$DISPLAY > $OUT/checks-x11-display"; sleep 1
  display=$(cat "$OUT/checks-x11-display")
  if [ -z "$display" ]; then
    echo "sway has no Xwayland here" >&2
    exit 1
  fi
  app_env=(-u WAYLAND_DISPLAY DISPLAY="$display" XDG_SESSION_TYPE=x11 GDK_BACKEND=x11)
fi
env "${app_env[@]}" "$APP" > "$OUT/checks-$1-app.log" 2>&1 &
APP_PID=$!
sleep 3
echo "windows: $(window_titles)"
shot "checks-$1"
if [ "$1" != ok ]; then
  echo "== press Close"
  wtype -s 500 -k space -s 500
else
  close_window
fi
report_exit
echo "--- app log"; applog "$OUT/checks-$1-app.log"
