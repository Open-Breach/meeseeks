#!/bin/bash
# The resolution selector: what it offers on an output of the given size, a
# preset and a custom size each started as a virtual monitor, and the last
# size remembered over a restart.
# Usage: resolution.sh <output width>x<output height> [scale]
#
# The presets are reached with the keyboard, backwards from the Start button:
# over the two custom fields, then over the presets that fit, last one first.
# Five steps back is the third preset from the end among those that fit.
. "$T/lib.sh"
size=${1:?usage: resolution.sh <width>x<height> [scale]}
swaymsg -q output HEADLESS-1 resolution "$size" scale "${2:-1}"; sleep 0.5
start_portal resolution
"$APP" > "$OUT/resolution-app.log" 2>&1 &
APP_PID=$!
sleep 2
echo "output $size at scale ${2:-1}; window: $(window_rect)"
shot resolution-idle

echo "== pick a preset: Shift+Tab five times from Start, space"
keys -M shift -k Tab -k Tab -k Tab -k Tab -k Tab -m shift -s 200 -k space; sleep 0.5
echo "window: $(window_rect)"
shot resolution-preset
echo "== back to Start: Tab five times; press it"
keys -k Tab -k Tab -k Tab -k Tab -k Tab
press_button; sleep 3
shot resolution-preset-running
echo "== press Stop"
press_button; sleep 2

echo "== a custom size: Shift+Tab twice from Start, 1500, Tab, 900, Tab"
type_size 1500 900; sleep 0.5
echo "window: $(window_rect)"
shot resolution-custom
echo "== press Start, then Stop"
press_button; sleep 3
shot resolution-custom-running
press_button; sleep 2

echo "== a custom size that does not fit: 5000 by 3000"
type_size 5000 3000; sleep 0.5
echo "window: $(window_rect)"
shot resolution-too-large
echo "== and back to 1500 by 900, started once more so that it is the last size used"
type_size 1500 900
press_button; sleep 3
close_window
report_exit
echo "--- settings file"; cat "$SETTINGS"

echo "== start the app again"
"$APP" >> "$OUT/resolution-app.log" 2>&1 &
APP_PID=$!
sleep 2
echo "window: $(window_rect)"
shot resolution-again
close_window
report_exit
echo "--- app log"; applog "$OUT/resolution-app.log"
echo "--- portal log"; grep -e negotiated -e Session.Close "$OUT/resolution-portal.log"
