#!/bin/bash
# A producer set up like Mutter's virtual monitor and any consumer of it,
# inside a root with a newer PipeWire. No app and no compositor: this is the
# reproduction behind the upstream report
# https://gitlab.freedesktop.org/pipewire/pipewire/-/work_items/5586
#
# Usage: consumer.sh <root> <renegotiate after ms | none> <seconds> <command>
#
# ID in the command is replaced by the producer's node id. For example:
#
#   consumer.sh root 400 11 'gst-launch-1.0 pipewiresrc path=ID ! video/x-raw,width=1920,height=1080 ! fakesink'
#   consumer.sh root 400 11 'python3 system-clock-consumer.py ID system-clock'
#   consumer.sh root 400 4  'python3 clockwatch.py ID'
#   consumer.sh root 400 11 './plain-consumer ID'
#
# Prints the producer's count of frames queued and of ticks on which it had no
# free buffer; the consumer's own output is in out/consumer.log. Needs root.
set -u
T=$(cd "$(dirname "$0")/.." && pwd)
usage='usage: consumer.sh <root> <renegotiate after ms | none> <seconds> <command>'
ROOT=${1:?$usage}
after=${2:?$usage}; secs=${3:?$usage}; command=${4:?$usage}

. "$T/newer-pipewire/root.sh"

renegotiate=()
[ "$after" = none ] || renegotiate=("RENEGOTIATE_MS=$after")
rm -f "$ROOT$IN_ROOT/done"
start_in_root SECS="$secs" COMMAND="$command" "${renegotiate[@]}" '
    ./renegotiating-source > producer.log 2>&1 &
    producer=$!
    for i in $(seq 30); do grep -q "node id" producer.log && break; sleep 0.1; done
    id=$(sed -n "s/^node id: //p" producer.log | head -1)
    timeout "$SECS" bash -c "${COMMAND//ID/$id}" > consumer.log 2>&1
    kill -INT $producer 2>/dev/null; sleep 0.5
    touch done
  '
for i in $(seq $((secs * 10 + 100))); do [ -e "$ROOT$IN_ROOT/done" ] && break; sleep 0.1; done

cp "$ROOT$IN_ROOT/producer.log" "$T/out/producer.log"
cp "$ROOT$IN_ROOT/consumer.log" "$T/out/consumer.log"
grep -e renegotiating -e "last 5 s" -e "driving: no" "$T/out/producer.log"
