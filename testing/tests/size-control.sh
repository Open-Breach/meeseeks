#!/bin/bash
# The control for the size check: the same producer with a consumer that asks
# for no particular size. It should settle on the producer's own 1280x720; if
# it came out at 1920x1080 as well, the app's result would prove nothing.
. "$T/lib.sh"
"$MOCK_PRODUCER" > "$OUT/size-control-producer.log" 2>&1 &
sleep 1
id=$(sed -n 's/^node id: //p' "$OUT/size-control-producer.log" | head -1)
timeout 4 gst-launch-1.0 -q pipewiresrc path="$id" num-buffers=5 ! videoconvert ! fakesink \
  > "$OUT/size-control-gst.log" 2>&1
kill %1 2>/dev/null; wait %1 2>/dev/null
grep -e "negotiated" "$OUT/size-control-producer.log"
