#!/bin/bash
# The screenshot button on a real Mutter with the real portal: the file is
# the virtual monitor's frame, with the cursor where the frame has it.
# Usage: screenshot.sh
#
# A window fills the virtual monitor (landing.py). A click on the picture
# sends the pointer to 320,180 of the monitor, and the button is pressed with
# the keyboard: that file should show the cursor there. The mouse is then
# brought back to the screen and the button pressed again: no cursor in that
# file. The test prints both files' sizes and the pixels around the spot;
# what the pictures show is for the eye.
#
# GSK_RENDERER=gl has GTK draw with GL, where Mutter and Mesa let it without
# a graphics card; the default here is Cairo.
. "$T/gnome/lib.sh"
name=gnome-screenshot
PICTURES=$XDG_RUNTIME_DIR/Bilder
mkdir -p "$PICTURES"
echo "XDG_PICTURES_DIR=\"$PICTURES\"" > "$XDG_CONFIG_HOME/user-dirs.dirs"
write_settings "width = 1280" "height = 720"
start_hands "$name"
hand move 20 20
RUST_LOG=info,meeseeks=debug "$APP" > "$OUT/$name-app.log" 2>&1 &
APP_PID=$!
sleep 3

echo "== press Start; Allow Remote Interaction on, then Share"
press_button; sleep 3
dialog share with interaction; sleep 4
"$PYTHON" "$T/gnome/landing.py" > "$OUT/$name-landing.log" 2>&1 &
LANDING=$!
sleep 3

# The newest file, and what is in it around the spot the cursor is sent to.
newest() {
  file=$(ls -t "$PICTURES/Meeseeks"/* 2>/dev/null | head -1)
  if [ -z "$file" ]; then echo "   no file"; return; fi
  cp "$file" "$OUT/$name-$1.png"
  echo "   $(basename "$file") (kept as out/$name-$1.png): $("$PYTHON" "$T/png.py" "$file" 320,180 322,184 326,190 600,600 | tr '\n' ' ')"
}

echo "== two clicks on the picture to find it, the second at 320,180 of the monitor"
hand move 700 450; hand press 272; hand release 272; hand move 20 20
read -r ax ay < <(grep "teleporter: a click at" "$OUT/$name-app.log" | tail -1 | sed 's/.*a click at \([-0-9.]*\),\([-0-9.]*\) of the picture.*/\1 \2/')
hand move $((700 - ax + 320)) $((450 - ay + 180)); hand press 272; hand release 272; sleep 1
tail -1 "$OUT/$name-landing.log" | sed 's/^\[landing\]/   monitor:/'
echo "== Tab from Stop to the button, and press it: the cursor is in the frame"
hand key Tab; hand key space; sleep 2
newest with-cursor
echo "== the mouse back onto the screen, and the button pressed again: no cursor"
hand move 20 20; sleep 0.5
hand key space; sleep 2
newest without-cursor

echo "== close the window"
kill $LANDING; sleep 1
close_window
report_exit
echo "--- app log"; applog "$OUT/$name-app.log" | grep -e "buffer path" -e "screenshot" -e "GTK draws" -e WARN -e ERROR | cut -c1-260
