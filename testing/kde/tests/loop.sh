#!/bin/bash
# The whole loop on a real KWin with the KDE portal: start, the portal's
# dialog, stop, start again, close the window while running.
# Usage: loop.sh [<width> <height>]
. "$T/kde/lib.sh"
write_settings "width = ${1:-1920}" "height = ${2:-1080}"
start_hands kde-loop
hand move 20 20
RUST_LOG=${RUST_LOG:-info} "$APP" > "$OUT/kde-loop-app.log" 2>&1 &
APP_PID=$!
pause 5
shot kde-loop-idle

echo "== press Start; the portal's dialog (out/kde-loop-dialog.png)"
press_button; pause 5
shot kde-loop-dialog
echo "== Return: the one entry there is, the new virtual output"
hand key Return; pause 6
shot kde-loop-running
echo "== press Stop"
press_button; pause 4
shot kde-loop-stopped
echo "== press Start again, Return"
press_button; pause 5
hand key Return; pause 6
echo "== close the window while running"
hand key alt+F4; pause 2
report_exit
echo "--- app log"; applog "$OUT/kde-loop-app.log" | cut -c1-330
echo "--- kwin"; grep -v -e "UTF-8" -e locale -e "^$" -e "qt.qpa" "$OUT/kwin.log" | tail -8 | cut -c1-200
echo "--- portal backend"; grep -v -e "UTF-8" -e locale -e "^$" -e libEGL -e MESA "$OUT/portal-kde.log" | tail -8 | cut -c1-200
