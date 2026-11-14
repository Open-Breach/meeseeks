# Helpers for the tests; sourced inside env.sh's desktop, where $T is set.

BUILD=$T/build
OUT=$T/out

# The app under test, and a Python that has the distribution's dbus and gi
# bindings. Both can be set from outside.
APP=${APP:-$T/../target/debug/meeseeks}
PYTHON=${PYTHON:-python3}

# The headless compositor has no GPU.
export GSK_RENDERER=${GSK_RENDERER:-cairo}
# A gtk4paintablesink built by hand, where the distribution packages none.
[ -d "$BUILD/plugins" ] && export GST_PLUGIN_PATH=$BUILD/plugins
# What the mock portal starts in place of a virtual monitor.
export MOCK_PRODUCER=${MOCK_PRODUCER:-$BUILD/picture-source}

for needed in "$APP" "$MOCK_PRODUCER"; do
  if [ ! -x "$needed" ]; then
    echo "missing $needed; run cargo build and testing/build.sh first" >&2
    exit 1
  fi
done

# Starts the mock portal; its log goes to out/<name>-portal.log.
start_portal() {
  "$PYTHON" "$T/mock_portal.py" > "$OUT/$1-portal.log" 2>&1 &
  sleep 1
}

# Prints "x y width height" of the app window.
window_rect() {
  swaymsg -t get_tree | "$PYTHON" -c '
import json, sys
def walk(n):
    if n.get("name") == "Meeseeks":
        r = n["rect"]; print(r["x"], r["y"], r["width"], r["height"]); sys.exit(0)
    for c in n.get("nodes", []) + n.get("floating_nodes", []):
        walk(c)
walk(json.load(sys.stdin)); sys.exit(1)'
}

# Presses the Start/Stop button. The headless seat has no devices, so this
# goes through a virtual keyboard: Tab focuses the button, space activates it.
# The pauses let the app bind the keyboard before the keys are sent.
press_button() { wtype -s 500 -k Tab -s 200 -k space -s 200; }

# Closes the window the way a click on its close button would.
close_window() { swaymsg -q '[title="Meeseeks"] kill'; }

# Saves a screenshot as out/<name>.png.
shot() { grim "$OUT/$1.png"; }

# Says how the app, started in the background as $APP_PID, ended.
report_exit() {
  for i in $(seq 50); do kill -0 "$APP_PID" 2>/dev/null || break; sleep 0.1; done
  if kill -0 "$APP_PID" 2>/dev/null; then
    echo "APP STILL RUNNING"; kill "$APP_PID"
  else
    wait "$APP_PID"; echo "app exited with status $?"
  fi
}

# An app log without the noise of a desktop that has no GPU and no
# accessibility bus.
applog() { grep -v -e a11y -e '^$' -e libEGL -e MESA "$1"; }
