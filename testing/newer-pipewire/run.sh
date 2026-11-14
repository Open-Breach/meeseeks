#!/bin/bash
# The app inside a root with a newer PipeWire, against a producer set up like
# Mutter's virtual monitor. sway runs on this machine; the bus, PipeWire, the
# mock portal, the producer and the app run inside the root.
#
# Usage: run.sh <root> <renegotiate after ms | none> <seconds> [VAR=value ...]
#
# VAR=value pairs go into the app's environment; WRAP="gdb -q -batch -ex run
# -ex bt --args" runs it under a debugger, if the root has one. Needs root.
# The app binary is the one built on this machine, which works as long as the
# root's libraries are the newer ones.
set -u
T=$(cd "$(dirname "$0")/.." && pwd)
ROOT=${1:?usage: run.sh <root> <renegotiate after ms | none> <seconds> [VAR=value ...]}
after=${2:?}; secs=${3:?}; shift 3
APP=${APP:-$T/../target/debug/meeseeks}
[ -x "$APP" ] || { echo "missing $APP; run cargo build first" >&2; exit 1; }

. "$T/newer-pipewire/root.sh"
cp "$APP" "$ROOT$IN_ROOT/meeseeks"

WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
  sway -c "$T/sway.conf" > "$T/out/sway.log" 2>&1 &
SWAY=$!
for i in $(seq 50); do
  socket=$(ls "$XDG_RUNTIME_DIR" | grep -m1 "^wayland-[0-9]*$") && break
  sleep 0.1
done
[ -n "${socket:-}" ] || { echo "sway did not come up; see $T/out/sway.log" >&2; exit 1; }
export WAYLAND_DISPLAY=$socket
export SWAYSOCK=$(ls "$XDG_RUNTIME_DIR"/sway-ipc.*.sock 2>/dev/null | head -1)
swaymsg -q output HEADLESS-1 resolution 2560x1440; sleep 0.3

renegotiate=()
[ "$after" = none ] || renegotiate=("RENEGOTIATE_MS=$after")
start_in_root WAYLAND_DISPLAY="$socket" GSK_RENDERER=cairo RUST_LOG=meeseeks=debug \
  MOCK_PRODUCER=$IN_ROOT/renegotiating-source "${renegotiate[@]}" "$@" '
    python3 mock_portal.py > portal.log 2>&1 &
    sleep 1
    ${WRAP:-} ./meeseeks > app.log 2>&1
    echo "app exited with status $?" >> app.log
  '

sleep 6
wtype -s 500 -k Tab -s 200 -k space -s 200
sleep "$secs"
grim "$T/out/newer-pipewire.png" 2>/dev/null
swaymsg -q '[title="Meeseeks"] kill' 2>/dev/null
for i in $(seq 50); do grep -q "app exited" "$ROOT$IN_ROOT/app.log" 2>/dev/null && break; sleep 0.1; done
swaymsg exit > /dev/null 2>&1

cp "$ROOT$IN_ROOT/app.log" "$T/out/newer-pipewire-app.log"
cp "$ROOT$IN_ROOT/portal.log" "$T/out/newer-pipewire-portal.log"
echo "--- producer"
grep -e renegotiating -e "last 5 s" "$T/out/newer-pipewire-portal.log" | cut -c1-160
echo "--- app"
grep -v -e a11y -e '^$' -e libEGL -e MESA "$T/out/newer-pipewire-app.log" | cut -c1-230
