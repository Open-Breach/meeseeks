#!/bin/bash
# The window comes up with a Start button. With no portal on the bus, Start
# logs an error and the button comes back.
. "$T/lib.sh"
"$APP" > "$OUT/no-portal-app.log" 2>&1 &
APP_PID=$!
sleep 2
echo "window: $(window_rect)"
shot no-portal-idle
echo "== press Start"
press_button; sleep 2
shot no-portal-after-start
close_window
report_exit
echo "--- app log"; applog "$OUT/no-portal-app.log"
