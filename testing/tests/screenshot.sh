#!/bin/bash
# The screenshot button: a click saves the virtual monitor's frame as a PNG
# in the Pictures folder.
# Usage: screenshot.sh [ok|scaled|away|no-folder]
#
#   ok         a click, then two more within a second: three files in
#              <Pictures>/Meeseeks, named after the date and time, the third
#              with " (2)", each as large as the monitor
#   scaled     the lock off and the window at half its size: the file is as
#              large as the monitor all the same
#   away       the window on its own virtual monitor, where it shows a message
#              in place of the picture: the file is the monitor's frame all
#              the same. An output made after Start stands in for the
#              virtual monitor, as in away.sh
#   no-folder  a file stands where the Meeseeks folder would go, so it cannot
#              be made: the warning mark, a warning in the log, and the app
#              keeps running
#
# The Pictures folder is the XDG user directory, set for the run to a folder
# with another name, as it has in many languages. The frames are the test
# producer's picture: a white edge of 12 pixels, then a gradient that begins
# at R,G,B = 0,0,160.
. "$T/lib.sh"
case=${1:-ok}
case $case in
  ok | scaled | away | no-folder) ;;
  *) sed -n 2,17p "$0" >&2; exit 1 ;;
esac
name=screenshot-$case

PICTURES=$XDG_RUNTIME_DIR/Bilder
mkdir -p "$PICTURES" "$XDG_CONFIG_HOME"
echo "XDG_PICTURES_DIR=\"$PICTURES\"" > "$XDG_CONFIG_HOME/user-dirs.dirs"
write_settings "width = 1280" "height = 720"
[ "$case" = scaled ] && write_settings "width = 1280" "height = 720" "lock_window_size = false"
[ "$case" = no-folder ] && : > "$PICTURES/Meeseeks"

start_portal "$name"
"$APP" > "$OUT/$name-app.log" 2>&1 &
APP_PID=$!
sleep 3
read -r x y w h < <(window_rect)
bar=$((h - 720))
# The title bar, where the button is.
title_bar() { read -r x y w h < <(window_rect); grim -g "$x,$y ${w}x$bar" "$OUT/$1.png"; }
# What the Pictures folder holds, and what is in each picture.
files() {
  if [ ! -d "$PICTURES/Meeseeks" ]; then
    echo "   no folder $PICTURES/Meeseeks"; ls -l "$PICTURES" | sed 's/^/   /'
    return
  fi
  for file in "$PICTURES/Meeseeks"/*; do
    [ -e "$file" ] || { echo "   the folder is empty"; break; }
    echo "   $(basename "$file"): $("$PYTHON" "$T/png.py" "$file" 0,0 11,11 12,12 1279,719 | tr '\n' ' ')"
  done
}

echo "== idle: no button in the title bar ($name-idle.png)"
title_bar "$name-idle"
echo "== press Start: the button is there, after Stop ($name-running.png)"
press_button; sleep 3
title_bar "$name-running"
if [ "$case" = scaled ]; then
  echo "== the window at half its size"
  swaymsg -q "[title=\"Meeseeks\"] resize set 640 $((360 + bar))"; sleep 1
fi
if [ "$case" = away ]; then
  echo "== the window on an output that has appeared since Start: the message ($name-message.png)"
  swaymsg -q create_output; sleep 0.5
  swaymsg -q '[title="Meeseeks"] move container to output HEADLESS-2'; sleep 1
  swaymsg -q '[title="Meeseeks"] focus'; sleep 0.5
  grim -o HEADLESS-2 "$OUT/$name-message.png"
fi
echo "== Tab from Stop to the button, and press it ($name-pressed.png, taken within its second and a half)"
keys -k Tab -k space
title_bar "$name-pressed"
sleep 2.5
echo "== two and a half seconds later: the camera again ($name-later.png)"
title_bar "$name-later"
files
if [ "$case" = ok ]; then
  echo "== two presses within a second"
  keys -k space -k space; sleep 2.5
  files
fi
if kill -0 "$APP_PID" 2>/dev/null; then echo "the app is still running"; fi
echo "== back to Stop, and press it: the button is gone again ($name-stopped.png)"
keys -M shift -k Tab -m shift -k space; sleep 2
title_bar "$name-stopped"
close_window
report_exit
echo "--- app log"; applog "$OUT/$name-app.log"
