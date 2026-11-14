#!/bin/bash
# The window opens with the picture area at exactly the stream's size, and the
# picture is drawn pixel for pixel.
# Usage: window-size.sh <output width>x<output height>, for example 2560x1440
. "$T/lib.sh"
size=${1:?usage: window-size.sh <width>x<height>}
swaymsg -q output HEADLESS-1 resolution "$size"; sleep 0.5
start_portal window-size
"$APP" > "$OUT/window-size-app.log" 2>&1 &
APP_PID=$!
sleep 2
read -r x y w h < <(window_rect)
row=$((h - 1080))
echo "output $size; window at $x,$y size ${w}x${h}; that leaves ${w}x$((h - row)) under a ${row}px button row"
shot "window-size-$size-idle"
echo "== press Start"
press_button; sleep 3
grim -t ppm "$OUT/window-size-$size.ppm"; shot "window-size-$size-running"
py=$((y + row))
echo "picture corners (the test picture has a 12px white edge, then a gradient from R,G,B = 0,0,160):"
"$PYTHON" "$T/px.py" "$OUT/window-size-$size.ppm" \
  "$x,$py" "$((x + 11)),$((py + 11))" "$((x + 12)),$((py + 12))" \
  "$((x + 1907)),$((py + 1067))" "$((x + 1908)),$((py + 1068))" "$((x + 1919)),$((py + 1079))" \
  "$x,$((py - 1))" "$x,$((py + 1080))"
echo "== press Stop"
press_button; sleep 2
echo "window after Stop: $(window_rect)"
echo "== shrink"
swaymsg -q '[title="Meeseeks"] resize set 700 500'; sleep 1
echo "window after resize: $(window_rect)"
close_window
report_exit
applog "$OUT/window-size-app.log"
grep -e negotiated -e Session.Close "$OUT/window-size-portal.log"
