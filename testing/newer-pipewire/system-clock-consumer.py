#!/usr/bin/python3
"""pipewiresrc ! fakesink, optionally with the pipeline forced onto the system
clock, the way the app does it.
Usage: system-clock-consumer.py <node id> [system-clock]"""
import sys
import gi
gi.require_version("Gst", "1.0")
from gi.repository import Gst, GLib

Gst.init(None)
pipeline = Gst.parse_launch(
    f"pipewiresrc path={sys.argv[1]} ! video/x-raw,width=1920,height=1080 ! fakesink sync=false")
if len(sys.argv) > 2 and sys.argv[2] == "system-clock":
    pipeline.use_clock(Gst.SystemClock.obtain())
pipeline.set_state(Gst.State.PLAYING)
GLib.timeout_add_seconds(1, lambda: print("clock:", pipeline.get_clock().get_name(), flush=True) and False)
GLib.MainLoop().run()
