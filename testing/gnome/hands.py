#!/usr/bin/env python3
"""A keyboard, a mouse and a camera for the headless GNOME desktop.

Mutter lets a program on the session bus work its keyboard and pointer and
watch its monitors. This holds such a session for as long as it runs and does
what the lines on its standard input say:

  move <x> <y>        the pointer to that spot of the monitor
  nudge <dx> <dy>     the pointer that far from where it is, as a real mouse
                      moves it; this is the way onto another monitor
  press <button>      by evdev code; 272 is the left button
  release <button>
  wheel <clicks>      the wheel, down if positive
  key <name>[+<name>...]   keys down in that order and up again, such as
                      space, Return, Tab, alt+s
  shot <file.png>     a picture of the monitor, cursor included
"""

import os
import subprocess
import sys
import time

import dbus
import dbus.mainloop.glib
from gi.repository import GLib

REMOTE_DESKTOP = "org.gnome.Mutter.RemoteDesktop"
SCREEN_CAST = "org.gnome.Mutter.ScreenCast"
KEYS = {
    "alt": 0xFFE9, "ctrl": 0xFFE3, "shift": 0xFFE1, "super": 0xFFEB,
    "space": 0x20, "Return": 0xFF0D, "Tab": 0xFF09, "Escape": 0xFF1B,
    "Up": 0xFF52, "F4": 0xFFC1, "Down": 0xFF54, "Left": 0xFF51, "Right": 0xFF53,
}


def say(*args):
    print("[hands]", *args, flush=True)


class Hands:
    def __init__(self, connector):
        self.bus = dbus.SessionBus()
        remote = self.bus.get_object(REMOTE_DESKTOP, "/org/gnome/Mutter/RemoteDesktop")
        path = remote.CreateSession(dbus_interface=REMOTE_DESKTOP)
        session = self.bus.get_object(REMOTE_DESKTOP, path)
        self.session = dbus.Interface(session, REMOTE_DESKTOP + ".Session")
        session_id = session.Get(REMOTE_DESKTOP + ".Session", "SessionId", dbus_interface=dbus.PROPERTIES_IFACE)

        cast = self.bus.get_object(SCREEN_CAST, "/org/gnome/Mutter/ScreenCast")
        cast_path = cast.CreateSession({"remote-desktop-session-id": session_id}, dbus_interface=SCREEN_CAST)
        cast_session = dbus.Interface(self.bus.get_object(SCREEN_CAST, cast_path), SCREEN_CAST + ".Session")
        # Cursor mode 1 has the cursor drawn into the frames.
        self.stream = cast_session.RecordMonitor(connector, {"cursor-mode": dbus.UInt32(1)})
        self.node = None
        stream = self.bus.get_object(SCREEN_CAST, self.stream)
        stream.connect_to_signal("PipeWireStreamAdded", self.stream_added, dbus_interface=SCREEN_CAST + ".Stream")
        self.session.Start()
        # A headless Mutter has no devices, and gives its seat a pointer and a
        # keyboard only once this session has used them. A window that is
        # there by then misses that first event, so they are used up here.
        self.session.NotifyPointerMotionRelative(1.0, 1.0)
        self.session.NotifyKeyboardKeysym(dbus.UInt32(KEYS["shift"]), True)
        self.session.NotifyKeyboardKeysym(dbus.UInt32(KEYS["shift"]), False)

    def stream_added(self, node):
        self.node = int(node)
        say(f"watching the monitor on PipeWire node {self.node}")

    def act(self, words):
        match words:
            case ["move", x, y]:
                self.session.NotifyPointerMotionAbsolute(self.stream, float(x), float(y))
            case ["nudge", dx, dy]:
                self.session.NotifyPointerMotionRelative(float(dx), float(dy))
            case ["press" | "release" as action, button]:
                self.session.NotifyPointerButton(int(button), action == "press")
            case ["wheel", clicks]:
                self.session.NotifyPointerAxisDiscrete(dbus.UInt32(0), int(clicks))
            case ["key", combination]:
                keys = [KEYS.get(name) or ord(name) for name in combination.split("+")]
                for key in keys:
                    self.session.NotifyKeyboardKeysym(dbus.UInt32(key), True)
                    time.sleep(0.05)
                for key in reversed(keys):
                    self.session.NotifyKeyboardKeysym(dbus.UInt32(key), False)
                    time.sleep(0.05)
            case ["shot", path]:
                self.shot(path)
            case _:
                say("not understood:", " ".join(words))
                return
        say(" ".join(words))

    def shot(self, path):
        if self.node is None:
            say("no stream yet")
            return
        camera = subprocess.Popen(
            ["gst-launch-1.0", "-q", "pipewiresrc", f"path={self.node}", "num-buffers=1",
             "!", "videoconvert", "!", "pngenc", "!", "filesink", f"location={path}"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        # A monitor is only sent when something on it changes.
        for _ in range(40):
            if camera.poll() is not None:
                break
            self.session.NotifyPointerMotionRelative(1.0, 0.0)
            self.session.NotifyPointerMotionRelative(-1.0, 0.0)
            time.sleep(0.1)
        else:
            camera.kill()
            say("no frame came")


def main():
    dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
    hands = Hands(sys.argv[1] if len(sys.argv) > 1 else "Meta-0")
    loop = GLib.MainLoop()

    def line(source, condition):
        text = source.readline()
        if text == "":
            loop.quit()
            return False
        if text.strip():
            try:
                hands.act(text.split())
            except dbus.exceptions.DBusException as error:
                say("failed:", text.strip(), "-", error.get_dbus_message())
        return True

    GLib.io_add_watch(sys.stdin, GLib.IO_IN | GLib.IO_HUP, line)
    say("ready")
    loop.run()


if __name__ == "__main__":
    main()
