#!/bin/bash
# The teleporter against the mock portal's RemoteDesktop: what the app sends
# when the picture is clicked, and that it still gives a monitor where remote
# control cannot be had.
# Usage: input.sh [stay|warp|nosize|denied|absent|kde] [locked|unlocked]
#
# stay    The portal takes every position and only prints it; the cursor stays
#         in the window. No desktop does that, but it shows everything the app
#         sends.
# warp    The portal does what a desktop does with its one cursor: a position
#         takes the cursor out of the window, onto a second sway output that
#         stands in for the virtual monitor.
# nosize  Start gives no size for the stream, as GNOME's portal does for a
#         virtual monitor, and every position is refused for it. On GNOME
#         itself the app moves the pointer over libei instead; the mock has
#         none, see gnome/tests/input.sh for that.
# denied  Start grants no pointer, as GNOME's dialog does with "Allow Remote
#         Interaction" left off. The monitor is there; a click sends nothing.
# absent  The portal has no RemoteDesktop interface. The app opens the plain
#         ScreenCast session instead, with its one Start.
# kde     The desktop says it is KDE. The app does not ask for remote control
#         there and opens the plain ScreenCast session, although this portal
#         would offer it.
#
# In all but `kde` the desktop says it is GNOME, the one the app asks for
# remote control on.
#
# The mouse here is a virtual pointer that is told where to be. A real mouse
# says how far it moved, so on a desktop the cursor would not come back from
# the monitor to a spot in the window the way it does here between the steps.
. "$T/lib.sh"
usage="usage: input.sh [stay|warp|nosize|denied|absent|kde] [locked|unlocked]"
portal=${1:-stay}
lock=${2:-locked}
name=input-$portal-$lock

write_settings "width = 1280" "height = 720"
[ "$lock" = unlocked ] && write_settings "width = 1280" "height = 720" "lock_window_size = false"
export XDG_CURRENT_DESKTOP=GNOME
# What a click on the picture is expected to bring.
expect="a position of that spot"
case $portal in
  stay) export MOCK_STREAM_SIZE=1280x720 ;;
  warp) export MOCK_STREAM_SIZE=1280x720 MOCK_POINTER_OUTPUT=HEADLESS-2 ;;
  nosize) expect="a position, refused" ;;
  denied) export MOCK_STREAM_SIZE=1280x720 MOCK_DEVICES=0; expect="nothing, the pointer was not granted" ;;
  absent) export MOCK_STREAM_SIZE=1280x720 MOCK_REMOTE=0; expect="nothing, there is no pointer" ;;
  kde) export MOCK_STREAM_SIZE=1280x720 XDG_CURRENT_DESKTOP=KDE; expect="nothing, there is no pointer" ;;
  *) echo "$usage" >&2; exit 1 ;;
esac
start_portal "$name"
start_pointer "$name"
RUST_LOG=info,meeseeks::input=debug "$APP" > "$OUT/$name-app.log" 2>&1 &
APP_PID=$!
sleep 2

# Where the picture is: under the title bar. sway centres a floating window
# on half a pixel if it has to, which would be in every position sent.
picture() {
  swaymsg -q '[title="Meeseeks"] move absolute position 160 100'; sleep 0.5
  read -r x y w h < <(window_rect)
  py=$((y + bar))
  echo "   window at $x,$y size ${w}x${h}; the picture begins at $x,$py"
}
# Says what the portal was sent since the last step.
seen=0
sent() {
  sleep 1
  tail -n +$((seen + 1)) "$OUT/$name-portal.log" | grep -e Notify -e "the cursor" -e refused | sed 's/^/    /'
  seen=$(wc -l < "$OUT/$name-portal.log")
}
# Brings the pointer onto a spot of the picture by way of the title bar.
over() { hand move $((x + w / 2)) $((y + bar / 2)); hand move $((x + $1)) $((py + $2)); }
# The same, and a click with the left button there.
click() { over "$1" "$2"; hand press 272; hand release 272; }
read -r x y w h < <(window_rect)
bar=$((h - 720))
picture

echo "== the pointer rests on the middle of the window while Start is pressed: nothing"
hand move $((x + 640)) $((py + 360))
press_button; sleep 3
sent
echo "== the output that stands in for the virtual monitor appears"
swaymsg -q create_output; sleep 0.5
swaymsg -q output HEADLESS-2 resolution 1280x720; sleep 0.5
hand move 5 5; sent > /dev/null

echo "== a click on the picture at 320,180: $expect"
click 320 180; sent
echo "== over the picture to 640,360 and ten quick moves across it, without a click: nothing"
over 640 360; sent
for step in 1 2 3 4 5 6 7 8 9 10; do echo "move $((x + 640 + step * 10)) $((py + 360))" >&9; done; sleep 3; sent
echo "== the right button, the wheel and a swipe on the picture: nothing"
over 640 360; hand press 273; hand release 273; hand wheel 2; hand swipe 30; sent
echo "== a click on the picture at 1279,719, its last pixel: $expect"
click 1279 719; sent
hand move 5 5; sent > /dev/null

if [ "$lock" = unlocked ]; then
  echo "== the window at half the size: 320,180 of it is 640,360 of the monitor"
  swaymsg -q "[title=\"Meeseeks\"] resize set 640 $((360 + bar))"; sleep 1
  picture
  click 320 180; sent
  hand move 5 5; sent > /dev/null
  echo "== the window 800 wide: the picture is 640 wide in the middle, with 80 to either side"
  swaymsg -q "[title=\"Meeseeks\"] resize set 800 $((360 + bar))"; sleep 1
  picture
  shot "$name-bands"
  echo "   40,100 is beside the picture: nothing"
  click 40 100; sent
  echo "   400,180 is its middle: 640,360"
  click 400 180; sent
  hand move 5 5; sent > /dev/null
  echo "== the window 640 by 560 under the title bar: the picture is 360 high in the middle, with 100 above and below"
  swaymsg -q "[title=\"Meeseeks\"] resize set 640 $((560 + bar))"; sleep 1
  picture
  echo "   320,50 is above the picture: nothing"
  click 320 50; sent
  echo "   320,280 is its middle: 640,360"
  click 320 280; sent
  hand move 5 5; sent > /dev/null
fi

echo "== press Stop"
swaymsg -q '[title="Meeseeks"] focus'
press_button; sleep 2
close_window
report_exit
echo "--- app log"; applog "$OUT/$name-app.log"
echo "--- portal log, without the pointer"; grep -v -e Notify -e "the cursor" -e refused -e '^\[producer\] state' "$OUT/$name-portal.log"
