# Helpers for the tests on the headless Plasma; sourced inside kde/env.sh's
# desktop, where $T is set.

OUT=$T/out
APP=${APP:-$T/../target/debug/meeseeks}
PYTHON=${PYTHON:-python3}
SETTINGS=$XDG_CONFIG_HOME/meeseeks/settings.toml

if [ ! -x "$APP" ]; then
  echo "missing $APP; run cargo build first" >&2
  exit 1
fi

# Waits that many seconds, or SLOW times as many where the desktop runs in an
# emulated machine and everything takes its time.
pause() { sleep "$(( $1 * ${SLOW:-1} ))"; }

# Writes the app's settings file from the lines given, one key = value each.
write_settings() {
  mkdir -p "$(dirname "$SETTINGS")"
  printf '%s\n' "$@" > "$SETTINGS"
}

# Gives the desktop a keyboard, a mouse and a camera, see hands.py; its log
# goes to out/<name>-hands.log. `hand` then does with them what its arguments
# say: move <x> <y> on the screen, nudge <dx> <dy> from where the pointer is,
# press or release <evdev code>, wheel <clicks>, key <name>[+<name>...].
start_hands() {
  mkfifo "$XDG_RUNTIME_DIR/hands"
  "$PYTHON" "$T/kde/hands.py" < "$XDG_RUNTIME_DIR/hands" > "$OUT/$1-hands.log" 2>&1 &
  exec 9> "$XDG_RUNTIME_DIR/hands"
  sleep 1.5
}
hand() { echo "$*" >&9; sleep "$(( ${SLOW:-1} > 1 ? 2 : 1 ))"; }

# Saves a picture of the screen the pointer is on as out/<name>.png.
shot() { hand shot "$OUT/$1.png"; sleep 1; }

# Presses Start, or Stop while a monitor is running: the app keeps the focus
# on the one of the two that can be pressed, and space activates it.
press_button() { hand key space; }

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
applog() { grep -v -e a11y -e '^$' -e libEGL -e MESA -e "UTF-8" -e "locale" "$1"; }

# What the names on the session bus are, without the numbered ones.
bus_names() {
  gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.ListNames | tr ',' '\n' | tr -d "[]()' " | grep -v '^:' | sort
}
