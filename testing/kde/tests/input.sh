#!/bin/bash
# The teleporter on a real KWin with the KDE portal: where the desktop's
# cursor ends up when the picture is clicked.
#
# The app does not ask Plasma for remote control. This test is for a build
# that does, which is not in the repository: it needs the one line in
# src/desktop.rs changed so that Plasma has the teleporter, and APP pointing
# at that build. With the app as it is, the test shows the plain session and
# clicks that send nothing.
#
# A window on the virtual monitor (gnome/landing.py) says where the pointer
# is on it. After each step the test prints what the app says it sent and
# what that window got.
. "$T/kde/lib.sh"
name=kde-input
write_settings "width = 1920" "height = 1080"
start_hands "$name"
hand move 20 20
RUST_LOG=info,meeseeks::input=debug,meeseeks::pointer=debug \
  "$APP" > "$OUT/$name-app.log" 2>&1 &
APP_PID=$!
pause 5

echo "== press Start; the portal's dialog (out/$name-dialog.png)"
press_button; pause 5
shot "$name-dialog"
echo "== Return: the dialog's default answer"
hand key Return; pause 7
# The KDE portal gives KWin three seconds to make the stream. A machine that
# is slow enough misses that the first time and makes it the second.
if grep -q "could not start" "$OUT/$name-app.log"; then
  echo "== Start failed: $(grep "could not start" "$OUT/$name-app.log" | tail -1 | sed 's/^[^]]*] //')"
  echo "== Start once more, and Return"
  press_button; pause 5
  hand key Return; pause 7
fi
"$PYTHON" "$T/gnome/landing.py" > "$OUT/$name-landing.log" 2>&1 &
LANDING=$!
pause 5
grep " on " "$OUT/$name-landing.log" | head -1 | sed 's/^/    /'
shot "$name-running"

# What the app and the window on the monitor have said since the last step.
app_seen=0; landing_seen=0
said() {
  pause 2
  tail -n +$((app_seen + 1)) "$OUT/$name-app.log" \
    | grep -e "teleporter: a click at" -e "WARN" \
    | sed -e 's/^[^ ]* //' -e 's/; frames of.*//' -e 's/^/    app: /' | cut -c1-330
  tail -n +$((landing_seen + 1)) "$OUT/$name-landing.log" | grep -v " on " | sed 's/^\[landing\]/    monitor:/'
  app_seen=$(wc -l < "$OUT/$name-app.log"); landing_seen=$(wc -l < "$OUT/$name-landing.log")
}
click() { hand move "$1" "$2"; hand press 272; hand release 272; }

# Presses the screenshot button, which has the keyboard, and says what the
# file has around the spot the pointer was last sent to.
frame() {
  hand key space; pause 3
  file=$(grep "screenshot saved: " "$OUT/$name-app.log" | tail -1 | sed 's/.*screenshot saved: //')
  if [ -z "$file" ] || [ "$file" = "$last_file" ]; then echo "   no file"; return; fi
  last_file=$file
  cp "$file" "$OUT/$name-$1.png"
  spot=$(grep "teleporter: a click at" "$OUT/$name-app.log" | tail -1 | sed 's/.*sent the pointer to \([0-9]*\),\([0-9]*\) of the monitor.*/\1 \2/')
  if [ -z "$spot" ]; then echo "   out/$name-$1.png, and no click has sent the pointer anywhere"; return; fi
  sx=${spot% *}; sy=${spot#* }
  echo "   out/$name-$1.png: $("$PYTHON" "$T/png.py" "$file" $((sx + 3)),$((sy + 6)) $((sx + 6)),$((sy + 12)) $((sx + 200)),$((sy + 100)) | tr '\n' ' ')"
}

echo "== over the picture without a click: nothing"
hand move 400 300; hand move 500 350; hand move 20 20; said
echo "== a click on the picture, at 400,300 of the screen"
click 400 300; said
echo "== the screenshot button, by Tab from Stop: the frame with the cursor on the monitor"
hand key Tab; frame with-cursor
echo "== a click and a turn of the wheel, wherever the cursor now is"
hand press 272; hand release 272; hand wheel 1; said
echo "== the mouse back onto the screen, and a click at 700,450"
hand move 20 20; click 700 450; said
hand move 20 20; said > /dev/null
echo "== the screenshot button again: the frame with the cursor back on the screen"
frame without-cursor
echo "== close the window"
kill $LANDING; pause 1
hand key alt+F4; pause 2
report_exit
echo "--- app log, without the clicks"; applog "$OUT/$name-app.log" | grep -v -e "DEBUG" | cut -c1-400
echo "--- what libei offered"; grep "libei:" "$OUT/$name-app.log" | sed 's/^[^ ]* //' | sort | uniq -c | cut -c1-330
echo "--- portal backend"; grep -v -e "UTF-8" -e locale -e "^$" -e libEGL -e MESA -e "qt.qpa" -e statusnotifier "$OUT/portal-kde.log" | tail -6 | cut -c1-200
echo "--- kwin"; grep -v -e "UTF-8" -e locale -e "^$" -e "qt.qpa" -e applications.menu "$OUT/kwin.log" | tail -6 | cut -c1-200
