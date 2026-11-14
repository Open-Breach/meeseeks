#!/bin/bash
# The area under the title bar is exactly the stream's size, and the picture
# is drawn pixel for pixel. With the window size locked the window cannot be
# resized; with the lock off, resizing scales the picture.
# Usage: window-size.sh <output width>x<output height> [locked|unlocked]
. "$T/lib.sh"
size=${1:?usage: window-size.sh <width>x<height> [locked|unlocked]}
lock=${2:-locked}
swaymsg -q output HEADLESS-1 resolution "$size"; sleep 0.5
[ "$lock" = unlocked ] && write_settings "lock_window_size = false"
start_portal window-size
"$APP" > "$OUT/window-size-app.log" 2>&1 &
APP_PID=$!
sleep 2
read -r x y w h < <(window_rect)
bar=$((h - 1080))
echo "output $size, $lock; window at $x,$y size ${w}x${h}; that leaves ${w}x$((h - bar)) under a ${bar}px title bar"
shot "window-size-$lock-idle"
echo "== press Start"
press_button; sleep 3
grim -t ppm "$OUT/window-size.ppm"; shot "window-size-$lock-running"
py=$((y + bar))
echo "picture corners (the test picture has a 12px white edge, then a gradient from R,G,B = 0,0,160):"
"$PYTHON" "$T/px.py" "$OUT/window-size.ppm" \
  "$x,$py" "$((x + 11)),$((py + 11))" "$((x + 12)),$((py + 12))" \
  "$((x + 1907)),$((py + 1067))" "$((x + 1908)),$((py + 1068))" "$((x + 1919)),$((py + 1079))" \
  "$x,$((py - 1))" "$x,$((py + 1080))"
echo "== ask sway to make the window 700 by 500"
swaymsg -q '[title="Meeseeks"] resize set 700 500'; sleep 1
# sway gives a floating window the frame it is asked for whether or not the
# window agrees, so what tells is the picture. Just right of that frame there
# is still picture if the window kept its size, and desktop if it shrank.
read -r x y w h < <(window_rect)
grim -t ppm "$OUT/window-size.ppm"; shot "window-size-$lock-resized"
py=$((y + bar + 150))
echo "sway's frame is now at $x,$y size ${w}x${h}; pixels just inside its right edge and 50px beyond it:"
"$PYTHON" "$T/px.py" "$OUT/window-size.ppm" "$((x + w - 20)),$py" "$((x + w + 50)),$py"
echo "== press Stop"
press_button; sleep 2
shot "window-size-$lock-stopped"
close_window
report_exit
applog "$OUT/window-size-app.log"
grep -e negotiated -e Session.Close "$OUT/window-size-portal.log"
