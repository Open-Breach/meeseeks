#!/bin/bash
# A desktop that decides the size of the virtual monitor itself, as KDE
# Plasma does: the monitor is made before the stream is asked for, the portal
# says how large it is, and the stream comes in no other size.
# Usage: given-size.sh [<width>x<height>]
#
# The size given is the monitor's; the default is 1920x1080, which is what
# Plasma makes up to 6.7. The app is started with 1280x720 chosen, the
# desktop says it is KDE, and the output is large enough for either size.
# Look for a Start that works, the `buffer path` line with the monitor's
# size, a window whose picture is that size, and that size in the settings
# and in the selector after Stop.
. "$T/lib.sh"
given=${1:-1920x1080}
export XDG_CURRENT_DESKTOP=KDE MOCK_STREAM_SIZE=$given PRODUCER_SIZE=$given
swaymsg -q output HEADLESS-1 resolution 2560x1440; sleep 0.5
write_settings "width = 1280" "height = 720"
start_portal given-size
"$APP" > "$OUT/given-size-app.log" 2>&1 &
APP_PID=$!
sleep 2
read -r x y w h < <(window_rect)
bar=$((h - 720))
echo "before Start: window ${w}x${h}, of which $bar are the title bar"
shot given-size-idle

echo "== press Start, with 1280x720 chosen and a monitor of $given"
press_button; sleep 3
if read -r x y w h < <(window_rect); then
  echo "running: window ${w}x${h}, the picture ${w}x$((h - bar))"
fi
shot given-size-running
echo "== press Stop"
press_button; sleep 2
shot given-size-stopped
echo "--- settings file"; cat "$SETTINGS"
close_window
report_exit
echo "--- app log"; applog "$OUT/given-size-app.log" | cut -c1-330
echo "--- portal log"; grep -e "SelectSources" -e "negotiated" -e "Start" "$OUT/given-size-portal.log"
