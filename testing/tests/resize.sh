#!/bin/bash
# Resizing the window before Start: the custom fields follow it, a size too
# large for the screen brings the warning, and Start takes the size the
# window has. From Start on the lock decides; after Stop the window can be
# resized again.
# Usage: resize.sh [locked|unlocked]
#
# sway resizes the window on request here. Nothing drags it with a pointer;
# the headless seat has none.
. "$T/lib.sh"
lock=${1:-locked}
swaymsg -q output HEADLESS-1 resolution 3440x1440; sleep 0.5
[ "$lock" = unlocked ] && write_settings "lock_window_size = false"
resize() { swaymsg -q "[title=\"Meeseeks\"] resize set $1 $2"; sleep 1; }
start_portal resize
"$APP" > "$OUT/resize-app.log" 2>&1 &
APP_PID=$!
sleep 2
read -r x y w h < <(window_rect)
bar=$((h - 1080))
echo "$lock; window: $x $y $w $h, with a ${bar}px title bar"

echo "== resize to 1000 by $((653 + bar)) before Start: the fields should read 1000 by 653, with no preset chosen"
resize 1000 $((653 + bar))
echo "window: $(window_rect)"
shot "resize-$lock-idle"
echo "== resize to 3000 by 1420: too large, the warning should show"
resize 3000 1420
echo "window: $(window_rect)"
shot "resize-$lock-too-large"
echo "== resize to 1100 by $((750 + bar)), then press Start"
resize 1100 $((750 + bar))
read -r x y w h < <(window_rect)
press_button; sleep 3
grim -t ppm "$OUT/resize.ppm"; shot "resize-$lock-running"
py=$((y + bar))
echo "window: $x $y $w $h; picture corners (white edge, gradient, white edge; then just outside):"
"$PYTHON" "$T/px.py" "$OUT/resize.ppm" \
  "$x,$py" "$((x + 12)),$((py + 12))" "$((x + 1099)),$((py + 749))" \
  "$((x + 1100)),$((py + 100))"

echo "== ask sway to make the window 700 by 500 while it runs"
resize 700 500
# sway draws the frame it is asked for either way; just right of it there is
# still picture if the window kept its size, and desktop if it shrank.
read -r x y w h < <(window_rect)
grim -t ppm "$OUT/resize.ppm"; shot "resize-$lock-running-resized"
py=$((y + bar + 150))
echo "sway's frame: $x $y $w $h; pixels just inside its right edge and 50px beyond it:"
"$PYTHON" "$T/px.py" "$OUT/resize.ppm" "$((x + w - 20)),$py" "$((x + w + 50)),$py"

echo "== press Stop; the window should be back at 1100 by $((750 + bar)) and the fields at 1100 by 750"
press_button; sleep 2
shot "resize-$lock-stopped"
echo "== resize to 900 by 650 again"
resize 900 650
echo "window: $(window_rect)"
shot "resize-$lock-after"
close_window
report_exit
echo "--- settings file"; cat "$SETTINGS"
echo "--- app log"; applog "$OUT/resize-app.log"
echo "--- portal log"; grep -e negotiated -e Session.Close "$OUT/resize-portal.log"
