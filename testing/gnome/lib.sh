# Helpers for the tests on the headless GNOME; sourced inside gnome/env.sh's
# desktop, where $T is set.

OUT=$T/out
APP=${APP:-$T/../target/debug/meeseeks}
PYTHON=${PYTHON:-python3}
SETTINGS=$XDG_CONFIG_HOME/meeseeks/settings.toml
# A gtk4paintablesink built by hand, where the distribution packages none.
if ! gst-inspect-1.0 gtk4paintablesink > /dev/null 2>&1 && [ -d "$T/build/plugins" ]; then
  export GST_PLUGIN_PATH=$T/build/plugins
fi

if [ ! -x "$APP" ]; then
  echo "missing $APP; run cargo build first" >&2
  exit 1
fi

# Writes the app's settings file from the lines given, one key = value each.
write_settings() {
  mkdir -p "$(dirname "$SETTINGS")"
  printf '%s\n' "$@" > "$SETTINGS"
}

# Gives the desktop a keyboard, a mouse and a camera, see hands.py; its log
# goes to out/<name>-hands.log. `hand` then does with them what its arguments
# say: move <x> <y> on the screen, nudge <dx> <dy> from where the pointer is,
# press or release <evdev code>, wheel <clicks>, key <name>[+<name>...]. It
# has to be there before the app, which would otherwise find a seat with no
# devices.
start_hands() {
  mkfifo "$XDG_RUNTIME_DIR/hands"
  "$PYTHON" "$T/gnome/hands.py" < "$XDG_RUNTIME_DIR/hands" > "$OUT/$1-hands.log" 2>&1 &
  exec 9> "$XDG_RUNTIME_DIR/hands"
  sleep 1.5
}
hand() { echo "$*" >&9; sleep 0.5; }

# Saves a picture of the screen as out/<name>.png.
shot() { hand shot "$OUT/$1.png"; sleep 1; }

# Presses Start, or Stop while a monitor is running: the app keeps the focus
# on the one of the two that can be pressed, and space activates it.
press_button() { hand key space; }

# Answers the portal's dialog, which has the focus when it opens. `share`
# presses Share; `share with interaction` switches Allow Remote Interaction
# on first; `cancel` presses Cancel. The keys are the dialog's own mnemonics.
dialog() {
  case "$*" in
    "share with interaction") hand key alt+i; sleep 0.5; hand key alt+s ;;
    share) hand key alt+s ;;
    cancel) hand key alt+c ;;
  esac
}

# Closes the window the way its close button would.
close_window() { hand key alt+F4; }

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

# What Mutter had to say, without what a bare Mutter always says.
mutterlog() { grep -v -e colord -e locale1 -e gnome-session -e RTKit -e '^$' "$OUT/mutter.log"; }
