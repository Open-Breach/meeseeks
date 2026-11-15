#!/bin/bash
# The teleporter on a real Mutter with the real portal: where the desktop's
# cursor ends up when the picture is clicked.
# Usage: input.sh [granted|denied] [locked|unlocked]
#
# granted  The dialog is answered with Allow Remote Interaction switched on.
#          GNOME gives no size for a virtual monitor's stream, so the app
#          moves the pointer over libei.
# denied   The dialog is answered with the switch left off, as it comes. The
#          monitor is there all the same, and a click sends nothing.
#
# A window on the virtual monitor (landing.py) says where the pointer is on
# it. After each step the test prints what the app says it sent and what that
# window got. With a scaled desktop (SCALE=2 with SCREEN=3200x2000, and
# LAYOUT=logical) the window counts in scaled units, half the monitor's
# pixels.
#
# The mouse here is told where to be on the screen. A real one says how far
# it moved, and would not come back from the monitor between the steps. The
# step called the way out moves it as a real one moves, with the desktop not
# scaled.
. "$T/gnome/lib.sh"
usage="usage: input.sh [granted|denied] [locked|unlocked]"
way=${1:-granted}
lock=${2:-locked}
name=gnome-input-$way-$lock

write_settings "width = 1280" "height = 720"
[ "$lock" = unlocked ] && write_settings "width = 1280" "height = 720" "lock_window_size = false"
case $way in
  granted | denied) ;;
  *) echo "$usage" >&2; exit 1 ;;
esac
start_hands "$name"
hand move 20 20
RUST_LOG=info,meeseeks::input=debug,meeseeks::pointer=debug \
  "$APP" > "$OUT/$name-app.log" 2>&1 &
APP_PID=$!
sleep 3

echo "== press Start; the portal's dialog"
press_button; sleep 3
shot "$name-dialog"
if [ "$way" = denied ]; then
  echo "== Share, with Allow Remote Interaction left off"
  dialog share
else
  echo "== Allow Remote Interaction on, then Share"
  dialog share with interaction
fi
sleep 4
"$PYTHON" "$T/gnome/landing.py" > "$OUT/$name-landing.log" 2>&1 &
LANDING=$!
sleep 3
grep "on Meta" "$OUT/$name-landing.log" | sed 's/^/    /'

# What the app and the window on the monitor have said since the last step.
app_seen=0; landing_seen=0
said() {
  sleep 1
  tail -n +$((app_seen + 1)) "$OUT/$name-app.log" \
    | grep -e "teleporter: a click at" -e "WARN" \
    | sed -e 's/^[^ ]* //' -e 's/; frames of.*//' -e 's/^/    app: /' | cut -c1-330
  tail -n +$((landing_seen + 1)) "$OUT/$name-landing.log" | grep -v "on Meta" | sed 's/^\[landing\]/    monitor:/'
  app_seen=$(wc -l < "$OUT/$name-app.log"); landing_seen=$(wc -l < "$OUT/$name-landing.log")
}
# Clicks a spot of the screen with the left button.
click() { hand move "$1" "$2"; hand press 272; hand release 272; }
# Where on the picture the last click landed, by the app's reckoning.
clicked() { grep "teleporter: a click at" "$OUT/$name-app.log" | tail -1 | sed 's/.*a click at \([-0-9.]*\),\([-0-9.]*\) of the picture.*/\1 \2/'; }
# Finds out where the picture is on the screen, and how many of the screen's
# units one of the picture's is, from two spots that are clicked.
find_picture() {
  hand move 20 20; click "$1" "$2"; hand move 20 20; read -r ax ay < <(clicked)
  hand move 20 20; click "$3" "$4"; hand move 20 20; read -r bx by < <(clicked)
  read -r px py kx ky < <("$PYTHON" -c "
import sys
hx1, hy1, hx2, hy2, ax, ay, bx, by = map(float, sys.argv[1:])
kx, ky = (hx2 - hx1) / (bx - ax), (hy2 - hy1) / (by - ay)
print(hx1 - ax * kx, hy1 - ay * ky, kx, ky)" "$1" "$2" "$3" "$4" "$ax" "$ay" "$bx" "$by")
  echo "   the picture begins at $px,$py of the screen; one of its units is $kx by $ky of the screen's"
}
# Where on the screen a spot of the picture is.
screen() { "$PYTHON" -c "import sys; px, py, kx, ky, x, y = map(float, sys.argv[1:]); print(px + x * kx, py + y * ky)" "$px" "$py" "$kx" "$ky" "$1" "$2"; }
# Clicks a spot of the picture, coming from off the window.
onto() {
  hand move 20 20
  click $(screen "$1" "$2")
}

if [ "$way" = denied ]; then
  echo "== two clicks on the picture: nothing, the pointer was not granted"
  click 700 450; click 900 550; hand move 20 20; said
else
  echo "== finding the picture on the screen"
  find_picture 700 450 900 550; said > /dev/null
  echo "== over the picture at 640,360 and across it, without a click: nothing"
  hand move 20 20; hand move $(screen 640 360); hand move $(screen 700 400); hand move 20 20; said
  echo "== the right button and the wheel on the picture: nothing"
  hand move $(screen 640 360); hand press 273; hand release 273; hand wheel 2; hand move 20 20; said
  echo "== a click on the picture at 320,180"
  onto 320 180; said
  echo "== a click and two clicks of the wheel, wherever the cursor now is: the monitor gets them, from the mouse itself"
  hand press 272; hand release 272; hand wheel 2; said
  echo "== the mouse back onto the screen, off the window"
  hand move 20 20; said
  echo "== a click on the picture at 1279,719, its last pixel"
  onto 1279 719; said
  hand move 20 20; said > /dev/null

  if [ "${SCALE:-0}" = 0 ]; then
    # The virtual monitor lies to the right of the screen, their tops level,
    # so a pointer that leaves it to the left at the height of the picture's
    # middle is 300 further on over the picture.
    my=$("$PYTHON" -c "print(round($py + 360 * $ky))")
    echo "== a click on the picture at 300,$my, then the way out as a real mouse goes it: 400 to the left, over the monitor's left edge onto the screen"
    onto 300 "$my"; hand nudge -400 0; said
    echo "== 300 further to the left, across the picture: nothing, the pointer stays on the screen"
    hand nudge -300 0; said
    hand move 20 20; said > /dev/null
  fi

  if [ "$lock" = unlocked ]; then
    echo "== the window tiled to the left half of the screen: the picture is scaled, with bands above and below"
    # A click on the title bar gives the window the keyboard back.
    hand move "$("$PYTHON" -c "print($px + 640 * $kx)")" "$("$PYTHON" -c "print($py - 20 * $ky)")"
    hand press 272; hand release 272; hand key super+Left; sleep 2
    shot "$name-tiled"
    find_picture 300 450 500 550; said > /dev/null
    # From here on the spots are those of the area the picture is drawn in.
    read -r ww wh < <(grep "drawn in an area of" "$OUT/$name-app.log" | tail -1 | sed 's/.*area of \([0-9]*\)×\([0-9]*\).*/\1 \2/')
    echo "   the picture is drawn in an area of ${ww}x${wh}"
    echo "   a click on the middle of that area: 640,360 of the monitor"
    onto $((ww / 2)) $((wh / 2)); said
    echo "   a click 40 below its top edge, in the band above the picture: nothing"
    onto $((ww / 2)) 40; said
    hand move 20 20; said > /dev/null
  fi
fi

echo "== press Stop"
kill $LANDING; sleep 1
press_button; sleep 3
echo "== close the window"
close_window
report_exit
echo "--- app log, without the clicks"; applog "$OUT/$name-app.log" | grep -v -e "DEBUG" | cut -c1-400
echo "--- what libei offered"; grep "libei:" "$OUT/$name-app.log" | sed 's/^[^ ]* //' | sort | uniq -c | cut -c1-330
echo "--- mutter"; mutterlog | grep -v "stack_position" | tail -8 | cut -c1-200
