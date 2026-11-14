#!/usr/bin/python3
"""pipewiresrc ! fakesink on the pipeline's own choice of clock; prints, every
50 ms, how far that clock has advanced compared with CLOCK_MONOTONIC.
Usage: clockwatch.py <node id>"""
import sys, time
import gi
gi.require_version("Gst", "1.0")
from gi.repository import Gst, GLib

Gst.init(None)
pipeline = Gst.parse_launch(
    f"pipewiresrc path={sys.argv[1]} ! video/x-raw,width=1920,height=1080 ! fakesink sync=false")
pipeline.set_state(Gst.State.PLAYING)
first = {}
def sample():
    clock = pipeline.get_clock()
    if clock is None:
        return True
    mono = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
    now = clock.get_time()
    if not first:
        first["mono"], first["clock"] = mono, now
        print("clock:", clock.get_name(), flush=True)
    m = (mono - first["mono"]) / 1e6
    c = (now - first["clock"]) / 1e6
    print(f"monotonic +{m:8.1f} ms   pipeline clock +{c:8.1f} ms   behind by {m - c:7.1f} ms", flush=True)
    return True
GLib.timeout_add(50, sample)
GLib.MainLoop().run()
