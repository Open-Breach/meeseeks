#!/usr/bin/env python3
"""A keyboard, a mouse and a camera for the headless Plasma desktop.

KWin takes input from a program that speaks its fake input protocol
(org_kde_kwin_fake_input), and hands out pictures of the screen over D-Bus
(org.kde.KWin.ScreenShot2). It keeps both for programs it knows, unless it
runs with KWIN_WAYLAND_NO_PERMISSION_CHECKS and
KWIN_SCREENSHOT_NO_PERMISSION_CHECKS set, as kde/env.sh starts it.

This holds a fake input device for as long as it runs and does what the lines
on its standard input say:

  move <x> <y>        the pointer to that spot of the screen layout
  nudge <dx> <dy>     the pointer that far from where it is, as a real mouse
                      moves it; this is the way onto another screen
  press <button>      by evdev code; 272 is the left button
  release <button>
  wheel <clicks>      the wheel, down if positive
  key <name>[+<name>...]   keys down in that order and up again, such as
                      space, Return, Tab, alt+F4
  shot <file.png>     a picture of the screen the pointer is on

It speaks the Wayland wire protocol itself, so it needs no bindings.
"""

import os
import select
import socket
import struct
import sys
import time
import zlib

FAKE_INPUT = "org_kde_kwin_fake_input"
# The requests of org_kde_kwin_fake_input, by their place in the protocol.
AUTHENTICATE, MOTION, BUTTON, AXIS, MOTION_ABSOLUTE, KEY = 0, 1, 2, 3, 9, 10
VERTICAL = 0
# What one click of a wheel is worth to KWin.
CLICK = 15
# Keys by their Linux evdev codes.
KEYS = {
    "Escape": 1, "Tab": 15, "Return": 28, "ctrl": 29, "shift": 42, "alt": 56, "space": 57,
    "F4": 62, "Up": 103, "Left": 105, "Right": 106, "Down": 108, "super": 125,
    "a": 30, "c": 46, "d": 32, "s": 31,
}


def say(*args):
    print("[hands]", *args, flush=True)


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


def png(width, height, stride, kind, raw):
    """A PNG file of a picture as KWin hands it out: rows of 32 bit pixels,
    blue first (QImage's ARGB32 and RGB32 formats) or red first."""
    def chunk(name, body):
        return struct.pack(">I", len(body)) + name + body + struct.pack(">I", zlib.crc32(name + body))

    red_first = kind in (16, 17, 18)  # QImage::Format_RGBX8888 and its kin
    rows = bytearray()
    for y in range(height):
        row = raw[y * stride:y * stride + width * 4]
        rows.append(0)
        if red_first:
            rows += bytes(b for i in range(0, len(row), 4) for b in row[i:i + 3])
        else:
            rows += bytes(b for i in range(0, len(row), 4) for b in (row[i + 2], row[i + 1], row[i]))
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(bytes(rows), 6)) + chunk(b"IEND", b"")


def shot(path):
    """Asks KWin for a picture of the screen the pointer is on."""
    import dbus

    bus = dbus.SessionBus()
    camera = dbus.Interface(
        bus.get_object("org.kde.KWin.ScreenShot2", "/org/kde/KWin/ScreenShot2"), "org.kde.KWin.ScreenShot2"
    )
    reader, writer = os.pipe()
    results = camera.CaptureActiveScreen({"include-cursor": dbus.Boolean(True)}, dbus.types.UnixFd(writer))
    os.close(writer)
    raw = b""
    while True:
        part = os.read(reader, 1 << 20)
        if not part:
            break
        raw += part
    os.close(reader)
    width, height, stride = int(results["width"]), int(results["height"]), int(results["stride"])
    with open(path, "wb") as file:
        file.write(png(width, height, stride, int(results["format"]), raw))


def main():
    wayland = Wayland()
    registry = wayland.new_id()
    wayland.request(1, 1, struct.pack("<I", registry))  # wl_display.get_registry
    found = None
    for target, opcode, arguments in wayland.roundtrip():
        if target == registry and opcode == 0:  # wl_registry.global
            name, length = struct.unpack_from("<II", arguments)
            interface = arguments[8:8 + length - 1].decode()
            version = struct.unpack_from("<I", arguments, 8 + length + (-length % 4))[0]
            if interface == FAKE_INPUT:
                found = (name, version)
    if found is None:
        sys.exit(f"the compositor has no {FAKE_INPUT}")
    name, version = found
    if version < 4:
        sys.exit(f"the compositor's {FAKE_INPUT} is version {version}, without the keyboard")

    device = wayland.new_id()
    wayland.request(registry, 0, struct.pack("<I", name) + string(FAKE_INPUT) + struct.pack("<II", 4, device))
    wayland.request(device, AUTHENTICATE, string("meeseeks test") + string("works the desktop for a test"))
    wayland.roundtrip()
    say("ready")

    def act(words):
        match words:
            case ["move", x, y]:
                wayland.request(device, MOTION_ABSOLUTE, fixed(float(x)) + fixed(float(y)))
            case ["nudge", dx, dy]:
                wayland.request(device, MOTION, fixed(float(dx)) + fixed(float(dy)))
            case ["press" | "release" as action, button]:
                wayland.request(device, BUTTON, struct.pack("<II", int(button), action == "press"))
            case ["wheel", clicks]:
                wayland.request(device, AXIS, struct.pack("<I", VERTICAL) + fixed(CLICK * int(clicks)))
            case ["key", combination]:
                codes = [KEYS[name] for name in combination.split("+")]
                for code in codes:
                    wayland.request(device, KEY, struct.pack("<II", code, 1))
                    wayland.roundtrip()
                    time.sleep(0.05)
                for code in reversed(codes):
                    wayland.request(device, KEY, struct.pack("<II", code, 0))
                    wayland.roundtrip()
                    time.sleep(0.05)
            case ["shot", path]:
                shot(path)
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
                try:
                    act(line.split())
                except Exception as error:  # noqa: BLE001 (a test goes on and says so)
                    say("failed:", line.strip(), "-", error)
                wayland.roundtrip()


if __name__ == "__main__":
    main()
