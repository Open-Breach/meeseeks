#!/bin/bash
# Frame rates with a producer set up like Mutter's virtual monitor: 60 frames
# a second from a pool of 8 buffers, and, if asked, a renegotiation some time
# after the start. The producer counts the ticks on which it had no free
# buffer; the app logs what reached it.
# Usage: frame-rate.sh <renegotiate after ms | none> <seconds> [VAR=value ...]
. "$T/lib.sh"
after=${1:?usage: frame-rate.sh <renegotiate after ms | none> <seconds> [VAR=value ...]}
secs=${2:?}; shift 2
export MOCK_PRODUCER=$BUILD/renegotiating-source
[ "$after" = none ] || export RENEGOTIATE_MS=$after
swaymsg -q output HEADLESS-1 resolution 2560x1440; sleep 0.5
start_portal frame-rate
env RUST_LOG=meeseeks=debug "$@" "$APP" > "$OUT/frame-rate-app.log" 2>&1 &
APP_PID=$!
sleep 2
press_button; sleep "$secs"
close_window
report_exit
echo "--- app"
applog "$OUT/frame-rate-app.log" | grep -e "buffer path" -e "last 5 s" -e ERROR -e trying | cut -c1-210
echo "--- producer"
grep -e "last 5 s" -e renegotiating "$OUT/frame-rate-portal.log"
