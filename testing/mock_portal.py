#!/usr/bin/env python3
"""A stand-in for xdg-desktop-portal's ScreenCast interface.

It follows the portal's D-Bus contract (request and session object paths,
Response signals) and, where the real thing would ask the compositor for a
virtual monitor, starts a test producer. Everything it is asked is printed, so
a test can check what the app sent.

MOCK_PRODUCER=<path> is the producer to start; it has to print "node id: <n>".
MOCK_START_RESPONSE=1 makes Start answer "cancelled", as a declined dialog does.
MOCK_START_DELAY=<seconds> holds the answer to Start back, as an open dialog does.
MOCK_BAD_FD=1 hands out a socket nobody answers on instead of the PipeWire
remote; MOCK_BAD_FD=devnull hands out /dev/null.
"""

import os
import socket
import subprocess
import sys

import dbus
import dbus.mainloop.glib
import dbus.service
from gi.repository import GLib

BUS_NAME = "org.freedesktop.portal.Desktop"
OBJECT_PATH = "/org/freedesktop/portal/desktop"
SCREENCAST = "org.freedesktop.portal.ScreenCast"
REQUEST = "org.freedesktop.portal.Request"
SESSION = "org.freedesktop.portal.Session"
PROPERTIES = "org.freedesktop.DBus.Properties"

def say(*args):
    print("[portal]", *args, flush=True)


def sender_part(sender):
    return sender.lstrip(":").replace(".", "_")


class WithProperties(dbus.service.Object):
    """Just enough of org.freedesktop.DBus.Properties for a caching proxy."""

    properties = {}

    @dbus.service.method(PROPERTIES, in_signature="ss", out_signature="v")
    def Get(self, interface, name):
        try:
            return self.properties[str(interface)][str(name)]
        except KeyError:
            raise dbus.exceptions.DBusException(
                f"No such property {interface}.{name}",
                name="org.freedesktop.DBus.Error.InvalidArgs",
            )

    @dbus.service.method(PROPERTIES, in_signature="s", out_signature="a{sv}")
    def GetAll(self, interface):
        return self.properties.get(str(interface), {})


class Request(WithProperties):
    @dbus.service.signal(REQUEST, signature="ua{sv}")
    def Response(self, response, results):
        pass

    @dbus.service.method(REQUEST)
    def Close(self):
        say("Request.Close", self._object_path)

    def respond(self, response, results):
        def emit():
            self.Response(dbus.UInt32(response), dbus.Dictionary(results, signature="sv"))
            self.remove_from_connection()
            return False

        GLib.idle_add(emit)


class Session(WithProperties):
    properties = {SESSION: {"version": dbus.UInt32(1)}}

    def __init__(self, bus, path, owner):
        super().__init__(bus, path)
        self.owner = owner
        self.sources = None
        self.producer = None
        self.closed = False

    @dbus.service.signal(SESSION, signature="a{sv}")
    def Closed(self, details):
        pass

    @dbus.service.method(SESSION)
    def Close(self):
        say("Session.Close", self._object_path)
        self.shut("closed by the app")

    def shut(self, why):
        if self.closed:
            return
        self.closed = True
        if self.producer is not None:
            self.producer.terminate()
            self.producer.wait()
            say(f"virtual monitor removed ({why})")
        self.remove_from_connection()


class ScreenCast(WithProperties):
    properties = {
        SCREENCAST: {
            "version": dbus.UInt32(5),
            "AvailableSourceTypes": dbus.UInt32(1 | 2 | 4),
            "AvailableCursorModes": dbus.UInt32(1 | 2 | 4),
        }
    }

    def __init__(self, bus):
        super().__init__(bus, OBJECT_PATH)
        self.bus = bus
        self.sessions = {}
        bus.add_signal_receiver(
            self.name_owner_changed,
            "NameOwnerChanged",
            "org.freedesktop.DBus",
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
        )

    def name_owner_changed(self, name, old, new):
        if new != "":
            return
        for session in list(self.sessions.values()):
            if session.owner == name and not session.closed:
                say("client", name, "left the bus")
                session.shut("its owner left the bus")

    def request(self, sender, options):
        token = options["handle_token"]
        return Request(self.bus, f"{OBJECT_PATH}/request/{sender_part(sender)}/{token}")

    @dbus.service.method(SCREENCAST, in_signature="a{sv}", out_signature="o", sender_keyword="sender")
    def CreateSession(self, options, sender=None):
        request = self.request(sender, options)
        path = f"{OBJECT_PATH}/session/{sender_part(sender)}/{options['session_handle_token']}"
        self.sessions[path] = Session(self.bus, path, sender)
        say("CreateSession ->", path)
        # A string, not an object path: the real portal's long-standing quirk.
        request.respond(0, {"session_handle": dbus.String(path)})
        return request

    @dbus.service.method(SCREENCAST, in_signature="oa{sv}", out_signature="o", sender_keyword="sender")
    def SelectSources(self, session_handle, options, sender=None):
        request = self.request(sender, options)
        shown = {str(k): (int(v) if not isinstance(v, str) else str(v)) for k, v in options.items() if k != "handle_token"}
        say("SelectSources", shown)
        self.sessions[str(session_handle)].sources = shown
        request.respond(0, {})
        return request

    @dbus.service.method(SCREENCAST, in_signature="osa{sv}", out_signature="o", sender_keyword="sender")
    def Start(self, session_handle, parent_window, options, sender=None):
        request = self.request(sender, options)
        session = self.sessions[str(session_handle)]
        say(f"Start (parent window {str(parent_window)!r})")

        response = int(os.environ.get("MOCK_START_RESPONSE", "0"))
        if response != 0:
            say("answering Start with response", response, "and closing the session, as the real portal does")
            session.shut("dialog declined")
            request.respond(response, {})
            return request

        delay = float(os.environ.get("MOCK_START_DELAY", "0"))
        if delay > 0:
            say(f"dialog open for {delay}s")
            GLib.timeout_add(int(delay * 1000), self.finish_start, session, request)
            return request
        self.finish_start(session, request)
        return request

    def finish_start(self, session, request):
        if session.closed:
            say("dialog dismissed: the session was closed while it was open")
            return False
        session.producer = subprocess.Popen([os.environ["MOCK_PRODUCER"]], stdout=subprocess.PIPE, text=True)
        node_id = None
        for line in session.producer.stdout:
            sys.stdout.write("[producer] " + line)
            sys.stdout.flush()
            if line.startswith("node id:"):
                node_id = int(line.split(":")[1])
                break
        GLib.io_add_watch(session.producer.stdout, GLib.IO_IN | GLib.IO_HUP, self.relay)
        say("virtual monitor created, PipeWire node", node_id)
        stream = dbus.Struct(
            (dbus.UInt32(node_id), dbus.Dictionary({"source_type": dbus.UInt32(4)}, signature="sv")),
            signature="ua{sv}",
        )
        request.respond(0, {"streams": dbus.Array([stream], signature="(ua{sv})")})
        return False

    @staticmethod
    def relay(source, condition):
        line = source.readline()
        if line == "":
            return False
        sys.stdout.write("[producer] " + line)
        sys.stdout.flush()
        return True

    @dbus.service.method(SCREENCAST, in_signature="oa{sv}", out_signature="h")
    def OpenPipeWireRemote(self, session_handle, options):
        if os.environ.get("MOCK_BAD_FD") == "devnull":
            # Not a socket at all.
            say("OpenPipeWireRemote (handing out /dev/null)")
            self.devnull = open("/dev/null", "rb")
            return dbus.types.UnixFd(self.devnull.fileno())
        if os.environ.get("MOCK_BAD_FD"):
            # Not a PipeWire remote at all: a socket nobody answers on.
            say("OpenPipeWireRemote (handing out a dead socket)")
            dead, self.other_end = socket.socketpair()
            self.other_end.close()
            return dbus.types.UnixFd(dead.fileno())
        remote = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        remote.connect(os.path.join(os.environ["XDG_RUNTIME_DIR"], "pipewire-0"))
        say("OpenPipeWireRemote")
        # dbus-python duplicates the descriptor into the reply.
        return dbus.types.UnixFd(remote.fileno())


def main():
    dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    name = dbus.service.BusName(BUS_NAME, bus)  # noqa: F841 (keeps the name)
    ScreenCast(bus)
    say("ready")
    GLib.MainLoop().run()


if __name__ == "__main__":
    main()
