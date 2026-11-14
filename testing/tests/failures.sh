#!/bin/bash
# What the app does when a start goes wrong.
# Usage: failures.sh <declined|close-mid-start|stream-ends|no-sink|bad-fd|not-a-remote>
#
#   declined         the portal dialog is declined
#   close-mid-start  the window is closed while the dialog is still open
#   stream-ends      the compositor's side takes the stream away while running
#   no-sink          gtk4paintablesink is not installed; this can only be
#                    staged where the sink comes from build/plugins
#   bad-fd           the portal hands out a socket nobody answers on
#   not-a-remote     the portal hands out something that is no socket at all
. "$T/lib.sh"
case "${1:-}" in
  declined) export MOCK_START_RESPONSE=1 ;;
  close-mid-start) export MOCK_START_DELAY=6 ;;
  bad-fd) export MOCK_BAD_FD=1 ;;
  not-a-remote) export MOCK_BAD_FD=devnull ;;
  stream-ends) ;;
  no-sink)
    if [ -z "${GST_PLUGIN_PATH:-}" ]; then
      echo "the sink is installed system-wide here; it cannot be hidden" >&2
      exit 1
    fi ;;
  *) sed -n '2,11p' "$0" >&2; exit 1 ;;
esac
start_portal "failures-$1"
[ "$1" = no-sink ] && unset GST_PLUGIN_PATH
"$APP" > "$OUT/failures-$1-app.log" 2>&1 &
APP_PID=$!
sleep 2
echo "== press Start"
press_button; sleep 3
case "$1" in
  stream-ends)
    echo "== the compositor side takes the stream away"
    pkill -f "$MOCK_PRODUCER"; sleep 3 ;;
  close-mid-start)
    echo "== close the window while the dialog is open"
    close_window; sleep 5 ;;
esac
shot "failures-$1"
if kill -0 "$APP_PID" 2>/dev/null; then
  echo "app still running; window: $(window_rect || echo gone)"
  if [ "$1" != close-mid-start ]; then
    echo "== press Start once more"
    press_button; sleep 3
    shot "failures-$1-again"
    close_window
  fi
fi
report_exit
sleep 0.5
echo "--- app log"; applog "$OUT/failures-$1-app.log"
echo "--- portal log"; cat "$OUT/failures-$1-portal.log"
