#!/bin/bash
# The whole loop on a real Mutter with the real portal, without the input
# experiment: start, stop, start again, close the window while running.
. "$T/gnome/lib.sh"
write_settings "width = 1280" "height = 720"
start_hands gnome-loop
hand move 20 20
"$APP" > "$OUT/gnome-loop-app.log" 2>&1 &
APP_PID=$!
sleep 3

echo "== press Start; the portal's dialog"
press_button; sleep 3
shot gnome-loop-dialog
echo "== Share"
dialog share; sleep 4
shot gnome-loop-running
echo "== press Stop"
press_button; sleep 3
shot gnome-loop-stopped
echo "== press Start again, Share"
press_button; sleep 3
dialog share; sleep 4
echo "== close the window while running"
close_window
report_exit
echo "--- app log"; applog "$OUT/gnome-loop-app.log" | cut -c1-300
echo "--- mutter"; mutterlog | tail -8 | cut -c1-200
