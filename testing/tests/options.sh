#!/bin/bash
# The options dialog: what it saves, the restart that follows, and the
# environment winning over the file. The dialog is worked with the keyboard:
# Alt+O opens it with the focus on the renderer, Tab goes on over the window
# lock to the copy switch, Alt+S saves and Return agrees to the restart.
# Usage: options.sh [plasma]
#
# plasma  The desktop says it is KDE and the portal has the mark of Plasma
#         6.6, so the dialog has one more switch, "Ask for remote control",
#         after the window lock (out/options-plasma-dialog.png). It is
#         switched off and saved; the app is back, and Start asks for the
#         monitor alone: the plain `Start` in the portal log, where the first
#         Start, before the switch, was a `RemoteDesktop.Start`.
. "$T/lib.sh"
# The settings are to decide here, not the environment.
unset GSK_RENDERER MEESEEKS_TRY
log=$OUT/options-app.log

if [ "${1:-}" = plasma ]; then
  export XDG_CURRENT_DESKTOP=KDE MOCK_KDE_MARKER=1 MOCK_STREAM_SIZE=1920x1080 PRODUCER_SIZE=1920x1080
  # Cairo from the file, as there is no GPU here and no GSK_RENDERER to say so.
  write_settings 'renderer = "cairo"'
  start_portal options-plasma
  log=$OUT/options-plasma-app.log
  "$APP" > "$log" 2>&1 &
  APP_PID=$!
  sleep 2
  echo "== Start as the app comes: remote control is asked for; Stop"
  press_button; sleep 3
  press_button; sleep 2
  echo "== open the options; Tab over the window lock to \"Ask for remote control\", off; save and restart"
  keys -M alt -k o -m alt; sleep 0.5
  shot options-plasma-dialog
  keys -k Tab -k Tab -k space
  keys -M alt -k s -m alt; sleep 0.5
  keys -k Return; sleep 3
  echo "--- settings file"; cat "$SETTINGS"
  echo "== the app is back by itself; press Start: the monitor alone"
  press_button; sleep 3
  close_window
  # The app that is running now is not the one that was started above.
  sleep 1
  echo "--- app log"; applog "$log" | grep -e "teleporter" -e "virtual monitor" -e "could not" | cut -c1-200
  echo "--- portal log"; grep -e "Start" -e "SelectDevices" "$OUT/options-plasma-portal.log"
  exit 0
fi
start_portal options

echo "== open the options; third renderer (Cairo), copy on; save and restart"
"$APP" > "$log" 2>&1 &
APP_PID=$!
sleep 2
keys -M alt -k o -m alt; sleep 0.5
keys -k space -s 400 -k Down -k Down -s 300 -k Return
keys -k Tab -k Tab -k space
shot options-dialog
keys -M alt -k s -m alt; sleep 0.5
shot options-question
keys -k Return; sleep 3
echo "--- settings file"; cat "$SETTINGS"
echo "== the app is back by itself; press Start"
press_button; sleep 3
echo "--- app log so far"; applog "$log" | grep -e "GTK draws" -e trying -e renderer -e "buffer path" | cut -c1-150

echo "== options again, with the monitor running: reset to defaults (Alt+R), save and restart; press Start"
seen=$(wc -l < "$log")
keys -M alt -k o -m alt; sleep 0.5
keys -M alt -k r -m alt
keys -M alt -k s -m alt; sleep 0.5
keys -k Return; sleep 3
echo "--- settings file"; cat "$SETTINGS"
press_button; sleep 3
close_window
report_exit
echo "--- app log since"; tail -n "+$((seen + 1))" "$log" | grep -e "GTK draws" -e trying -e renderer -e "buffer path" | cut -c1-150

echo "== Vulkan in the file, GSK_RENDERER=cairo and MEESEEKS_TRY=copy in the environment"
write_settings 'renderer = "vulkan"'
GSK_RENDERER=cairo MEESEEKS_TRY=copy "$APP" > "$log" 2>&1 &
APP_PID=$!
sleep 2
keys -M alt -k o -m alt; sleep 0.5
shot options-overridden
keys -M alt -k c -m alt
press_button; sleep 3
close_window
report_exit
echo "--- app log"; applog "$log" | grep -e "GTK draws" -e trying -e renderer -e "is set" -e "buffer path" | cut -c1-150

echo "== the same file with nothing in the environment"
"$APP" > "$log" 2>&1 &
APP_PID=$!
sleep 2
press_button; sleep 3
close_window
report_exit
echo "--- app log"; applog "$log" | grep -e "GTK draws" -e trying -e renderer -e "is set" -e "buffer path" | cut -c1-150
echo "--- portal log: every session that was started is closed again"
grep -e "virtual monitor created" -e Session.Close "$OUT/options-portal.log"
