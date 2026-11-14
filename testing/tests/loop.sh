#!/bin/bash
# The whole loop against the mock portal: start, stop, start again, close the
# window while running.
. "$T/lib.sh"
start_portal loop
"$APP" > "$OUT/loop-app.log" 2>&1 &
APP_PID=$!
sleep 2

echo "== press Start"
press_button; sleep 3
shot loop-running
echo "== press Stop"
press_button; sleep 2
shot loop-stopped
echo "== press Start again"
press_button; sleep 3
shot loop-running-again
echo "== close the window while running"
close_window
report_exit
sleep 0.5
echo "--- app log"; applog "$OUT/loop-app.log"
echo "--- portal log"; cat "$OUT/loop-portal.log"
