#!/bin/bash
# The whole loop on a real KWin with the KDE portal: start, the portal's
# dialog, stop, start again, close the window while running.
# Usage: loop.sh [<width> <height> [off]]
#
# The size is the one chosen in the app. Plasma makes the monitor at a size
# of its own, 1920x1080 up to 6.7, and the window follows: look for `the
# desktop made the monitor` in the log where another size was chosen.
#
# `off` has "Ask for remote control" off in the settings. Without it the app
# asks a Plasma of 6.6 or later for remote control, and the dialog is
# "Remote Control" with Approve and Deny; with it, or on an older Plasma,
# the dialog is the one that lists what can be shared. Return answers
# either.
#
# A backend of another release, given as KDE_BACKEND, draws its chooser with
# this Plasma's libraries, and the one of 6.5 then has no buttons and takes
# no Return. A double click on its one entry answers it; the place clicked
# is right for the 1100x700 screen in-vm.sh gives.
. "$T/kde/lib.sh"
started() { grep -c "the portal started a virtual monitor" "$OUT/kde-loop-app.log"; }
answer() {
  local before
  before=$(started)
  hand key Return; pause 6
  if [ "$(started)" = "$before" ]; then
    echo "   Return did not answer it: a double click on the dialog's one entry"
    hand move 550 270; hand press 272; hand release 272; hand press 272; hand release 272; pause 6
    hand move 20 20
  fi
}
settings=("width = ${1:-1920}" "height = ${2:-1080}")
[ "${3:-}" = off ] && settings+=("ask_remote_control = false")
write_settings "${settings[@]}"
start_hands kde-loop
hand move 20 20
RUST_LOG=${RUST_LOG:-info} "$APP" > "$OUT/kde-loop-app.log" 2>&1 &
APP_PID=$!
pause 5
shot kde-loop-idle

echo "== press Start; the portal's dialog (out/kde-loop-dialog.png)"
press_button; pause 5
shot kde-loop-dialog
echo "== Return: the dialog's default answer"
answer
shot kde-loop-running
echo "== press Stop"
press_button; pause 4
shot kde-loop-stopped
echo "== press Start again, Return"
press_button; pause 5
answer
echo "== close the window while running"
hand key alt+F4; pause 2
report_exit
echo "--- app log"; applog "$OUT/kde-loop-app.log" | cut -c1-330
echo "--- kwin"; grep -v -e "UTF-8" -e locale -e "^$" -e "qt.qpa" "$OUT/kwin.log" | tail -8 | cut -c1-200
echo "--- portal backend"; grep -v -e "UTF-8" -e locale -e "^$" -e libEGL -e MESA "$OUT/portal-kde.log" | tail -8 | cut -c1-200
