#!/bin/bash
# The options dialog: what it saves, the restart that follows, and the
# environment winning over the file. The dialog is worked with the keyboard:
# Alt+O opens it with the focus on the renderer, Tab goes on over the window
# lock to the copy switch, Alt+S saves and Return agrees to the restart.
. "$T/lib.sh"
# The settings are to decide here, not the environment.
unset GSK_RENDERER MEESEEKS_TRY
start_portal options
log=$OUT/options-app.log

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
