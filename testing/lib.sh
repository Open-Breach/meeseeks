# Helpers for the tests; sourced inside env.sh's desktop, where $T is set.

BUILD=$T/build
OUT=$T/out

# The app under test, and a Python that has the distribution's dbus and gi
# bindings. Both can be set from outside.
APP=${APP:-$T/../target/debug/meeseeks}
PYTHON=${PYTHON:-python3}

# The app's settings file goes where the run's other files are, so that it
# starts from the defaults and leaves the machine's own settings alone.
export XDG_CONFIG_HOME=$XDG_RUNTIME_DIR/config
SETTINGS=$XDG_CONFIG_HOME/meeseeks/settings.toml

# The headless compositor has no GPU.
export GSK_RENDERER=${GSK_RENDERER:-cairo}
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

# Prints the titles of all windows, separated by " | ".
window_titles() {
  swaymsg -t get_tree | "$PYTHON" -c '
import json, sys
def walk(n):
    if n.get("pid") and n.get("name"):
        yield n["name"]
    for c in n.get("nodes", []) + n.get("floating_nodes", []):
        yield from walk(c)
print(" | ".join(walk(json.load(sys.stdin))))'
}

# Presses Start, or Stop while a monitor is running. The headless seat has no
# devices, so this goes through a virtual keyboard: the app keeps the focus on
# the one of the two that can be pressed, and space activates it.
press_button() { keys -k space; }

# Sends keys, given the way wtype takes them. Every call is a new keyboard to
# the app, and a key sent before the app has taken that in is lost.
keys() { wtype -s 500 "$@" -s 200; }

# Types a custom size into the selector's two fields. Start has to have the
# focus, and has it again afterwards.
type_size() { keys -M shift -k Tab -k Tab -m shift -s 200 "$1" -k Tab -s 200 "$2" -k Tab; }

# Gives the seat a mouse, see pointer.py; its log goes to out/<name>-pointer.log.
# `hand` then does with it what its arguments say: move <x> <y> on the output
# layout, press or release <evdev code>, wheel <clicks>, swipe <dy>.
start_pointer() {
  mkfifo "$XDG_RUNTIME_DIR/hand"
  "$PYTHON" "$T/pointer.py" < "$XDG_RUNTIME_DIR/hand" > "$OUT/$1-pointer.log" 2>&1 &
  exec 9> "$XDG_RUNTIME_DIR/hand"
  sleep 0.5
}
hand() { echo "$*" >&9; sleep 0.5; }

# Writes the app's settings file from the lines given, one key = value each.
write_settings() {
  mkdir -p "$(dirname "$SETTINGS")"
  printf '%s\n' "$@" > "$SETTINGS"
}

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
