#!/bin/bash
# The README's screenshot: the window before Start with 1280×720 chosen, on a
# screen of 3440×1440, which is the kind of screen the app is for. Taken on
# the release build, once per release:
#
#   cargo build --release
#   APP=target/release/meeseeks testing/env.sh testing/tests/readme-shot.sh
#
# It writes docs/screenshots/readme.png, the window alone: 1280×767, the
# picture's 1280×720 under the title bar. The headless desktop draws with
# Cairo and has the fonts of the machine it runs on, so look at the result
# before committing it.
. "$T/lib.sh"
into=$T/../docs/screenshots
swaymsg -q output HEADLESS-1 resolution 3440x1440
# White behind the window's round corners, as on the README's page.
swaymsg -q output HEADLESS-1 bg '#ffffff' solid_color
sleep 0.5
write_settings "width = 1280" "height = 720"
start_portal readme-shot
"$APP" > "$OUT/readme-shot-app.log" 2>&1 &
APP_PID=$!
sleep 3
read -r x y w h < <(window_rect)
mkdir -p "$into"
grim -g "$x,$y ${w}x${h}" "$into/readme.png"
echo "docs/screenshots/readme.png: $("$PYTHON" "$T/png.py" "$into/readme.png" | head -1)"
close_window
report_exit
echo "--- app log"; applog "$OUT/readme-shot-app.log"
