#!/usr/bin/env python3
"""A mouse for the headless seat, which has none. Without a pointer device
sway gives the seat no pointer, and a window never hears of one.

This holds a virtual pointer (wlr-virtual-pointer-unstable-v1) for as long as
it runs, and does with it what the lines on its standard input say:

  move <x> <y>       to that spot of the output layout
  press <button>     by evdev code; 272 is the left button
  release <button>
  wheel <clicks>     the wheel, down if positive
  swipe <dy>         two fingers on a touchpad, down if positive, then lifted

It speaks the Wayland wire protocol itself, so it needs no bindings. The
moves ask swaymsg for the size of the layout.
"""

import json
import os
import select
import socket
import struct
import subprocess
import sys
import time

MANAGER = "zwlr_virtual_pointer_manager_v1"
# The requests of zwlr_virtual_pointer_v1, by their place in the protocol.
MOTION_ABSOLUTE, BUTTON, AXIS, FRAME, AXIS_SOURCE, AXIS_STOP, AXIS_DISCRETE = 1, 2, 3, 4, 5, 6, 7
VERTICAL = 0
WHEEL, FINGER = 0, 1
# What one click of a wheel is worth in the units of a smooth scroll.
CLICK = 15


def say(*args):
    print("[pointer]", *args, flush=True)


def string(text):
    data = text.encode() + b"\0"
    return struct.pack("<I", len(data)) + data + b"\0" * (-len(data) % 4)


def fixed(value):
    return struct.pack("<i", round(value * 256))


class Wayland:
    def __init__(self):
        display = os.environ.get("WAYLAND_DISPLAY", "wayland-0")
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.connect(os.path.join(os.environ["XDG_RUNTIME_DIR"], display))
        self.pending = b""
        self.last_id = 1  # wl_display

    def new_id(self):
        self.last_id += 1
        return self.last_id

    def request(self, target, opcode, arguments=b""):
        self.socket.sendall(struct.pack("<IHH", target, opcode, 8 + len(arguments)) + arguments)

    def events(self):
        """Reads what has arrived and yields it as (object, opcode, arguments)."""
        data = self.socket.recv(65536)
        if not data:
            sys.exit("the compositor has gone")
        self.pending += data
        while len(self.pending) >= 8:
            target, opcode, size = struct.unpack_from("<IHH", self.pending)
            if len(self.pending) < size:
                break
            arguments, self.pending = self.pending[8:size], self.pending[size:]
            if target == 1 and opcode == 0:  # wl_display.error
                length = struct.unpack_from("<I", arguments, 8)[0]
                sys.exit("the compositor says: " + arguments[12:12 + length - 1].decode())
            yield target, opcode, arguments

    def roundtrip(self):
        """Waits until the compositor has seen everything sent so far, and
        returns the events that arrived in the meantime."""
        callback = self.new_id()
        self.request(1, 0, struct.pack("<I", callback))  # wl_display.sync
        seen = []
        while True:
            for event in self.events():
                if event[0] == callback:
                    return seen
                seen.append(event)


def layout_size():
    outputs = json.loads(subprocess.check_output(["swaymsg", "-t", "get_outputs"]))
    rects = [output["rect"] for output in outputs if output.get("active")]
    return (
        max(rect["x"] + rect["width"] for rect in rects),
        max(rect["y"] + rect["height"] for rect in rects),
    )


def main():
    wayland = Wayland()
    registry = wayland.new_id()
    wayland.request(1, 1, struct.pack("<I", registry))  # wl_display.get_registry
    manager_name = None
    for target, opcode, arguments in wayland.roundtrip():
        if target == registry and opcode == 0:  # wl_registry.global
            name, length = struct.unpack_from("<II", arguments)
            if arguments[8:8 + length - 1].decode() == MANAGER:
                manager_name = name
    if manager_name is None:
        sys.exit(f"the compositor has no {MANAGER}")

    manager = wayland.new_id()
    wayland.request(registry, 0, struct.pack("<I", manager_name) + string(MANAGER) + struct.pack("<II", 1, manager))
    pointer = wayland.new_id()
    wayland.request(manager, 0, struct.pack("<II", 0, pointer))  # create_virtual_pointer, on any seat
    wayland.roundtrip()
    say("the seat has a pointer now")

    def now():
        return struct.pack("<I", int(time.monotonic() * 1000) & 0xFFFFFFFF)

    def frame():
        wayland.request(pointer, FRAME)

    def act(words):
        match words:
            case ["move", x, y]:
                width, height = layout_size()
                wayland.request(pointer, MOTION_ABSOLUTE, now() + struct.pack("<IIII", int(x), int(y), width, height))
                frame()
            case ["press" | "release" as action, button]:
                wayland.request(pointer, BUTTON, now() + struct.pack("<II", int(button), action == "press"))
                frame()
            case ["wheel", clicks]:
                wayland.request(pointer, AXIS_SOURCE, struct.pack("<I", WHEEL))
                wayland.request(pointer, AXIS_DISCRETE, now() + struct.pack("<I", VERTICAL) + fixed(CLICK * int(clicks)) + struct.pack("<i", int(clicks)))
                frame()
            case ["swipe", dy]:
                wayland.request(pointer, AXIS_SOURCE, struct.pack("<I", FINGER))
                wayland.request(pointer, AXIS, now() + struct.pack("<I", VERTICAL) + fixed(float(dy)))
                frame()
                wayland.request(pointer, AXIS_SOURCE, struct.pack("<I", FINGER))
                wayland.request(pointer, AXIS_STOP, now() + struct.pack("<I", VERTICAL))
                frame()
            case _:
                say("not understood:", " ".join(words))
                return
        say(" ".join(words))

    while True:
        readable, _, _ = select.select([sys.stdin, wayland.socket], [], [])
        if wayland.socket in readable:
            for _ in wayland.events():
                pass
        if sys.stdin in readable:
            line = sys.stdin.readline()
            if line == "":
                break
            if line.strip():
                act(line.split())
                wayland.roundtrip()


if __name__ == "__main__":
    main()
