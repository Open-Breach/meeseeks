#!/bin/bash
# The window on its own virtual monitor: the picture gives way to a message
# and comes back when the window is on a physical screen again.
#
# The mock portal makes no monitor, so a second sway output created after
# Start stands in for the virtual one: to the app, any monitor that turns up
# once it has asked for a virtual monitor is that monitor. An output created
# before Start is a physical screen to it.
. "$T/lib.sh"
move() { swaymsg -q "[title=\"Meeseeks\"] move container to output $1"; sleep 1; }
# Which output the window is on, by sway's reckoning.
where() {
  swaymsg -t get_tree | "$PYTHON" -c '
import json, sys
def walk(n, output):
    if n.get("type") == "output":
        output = n.get("name")
    if n.get("name") == "Meeseeks" and n.get("pid"):
        print(output); sys.exit(0)
    for c in n.get("nodes", []) + n.get("floating_nodes", []):
        walk(c, output)
walk(json.load(sys.stdin), None)'
}
write_settings "width = 1280" "height = 720"
swaymsg -q create_output; sleep 0.5
start_portal away
"$APP" > "$OUT/away-app.log" 2>&1 &
APP_PID=$!
sleep 2
echo "== press Start, then create a third output"
press_button; sleep 3
swaymsg -q create_output; sleep 0.5
echo "window on $(where)"
echo "== move to HEADLESS-2, which was there before Start: the picture stays"
move HEADLESS-2
echo "window on $(where)"; grim -o HEADLESS-2 "$OUT/away-on-2.png"
echo "== move to HEADLESS-3, which has appeared since: the message"
move HEADLESS-3
echo "window on $(where)"; grim -o HEADLESS-3 "$OUT/away-on-3.png"
echo "== move back to HEADLESS-1: the picture again"
move HEADLESS-1
echo "window on $(where)"; grim -o HEADLESS-1 "$OUT/away-back-on-1.png"
echo "== across the edge between HEADLESS-2 and HEADLESS-3: the message"
swaymsg -q '[title="Meeseeks"] move absolute position 2900 100'; sleep 1
grim -g "2900,100 1280x767" "$OUT/away-half-on-3.png"
echo "== press Stop there: the selector"
swaymsg -q '[title="Meeseeks"] focus'
press_button; sleep 2
grim -g "2900,100 1280x767" "$OUT/away-stopped.png"
close_window
report_exit
echo "--- app log"; applog "$OUT/away-app.log"
