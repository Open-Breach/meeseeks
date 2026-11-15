#!/bin/bash
# The startup checks on a real Plasma: what the app makes of this desktop
# before anything is asked of the portal. Where a check fails the app shows
# its dialog and ends with Close; where none does, its window.
. "$T/kde/lib.sh"
start_hands kde-checks
hand move 20 20
"$APP" > "$OUT/kde-checks-app.log" 2>&1 &
APP_PID=$!
sleep 5
shot kde-checks
if grep -q "FAIL" "$OUT/kde-checks-app.log"; then
  echo "== a check failed: the dialog (out/kde-checks.png); space on Close, which has the keyboard"
  hand key space; sleep 1
else
  echo "== no check failed: the window (out/kde-checks.png); closing it"
  hand key alt+F4; sleep 1
fi
report_exit
echo "--- app log"; applog "$OUT/kde-checks-app.log" | cut -c1-330
