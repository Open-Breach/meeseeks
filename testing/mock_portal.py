#!/usr/bin/env python3
"""A stand-in for xdg-desktop-portal's ScreenCast interface, and for the part
of its RemoteDesktop interface that moves the pointer.

It follows the portal's D-Bus contract (request and session object paths,
Response signals) and, where the real thing would ask the compositor for a
virtual monitor, starts a test producer. Everything it is asked is printed, so
a test can check what the app sent.

MOCK_PRODUCER=<path> is the producer to start; it has to print "node id: <n>".
MOCK_SOURCE_TYPES=<bits> is what AvailableSourceTypes says; the default is 7,
which is monitor, window and virtual.
MOCK_START_RESPONSE=1 makes Start answer "cancelled", as a declined dialog does.
MOCK_START_DELAY=<seconds> holds the answer to Start back, as an open dialog does.
MOCK_BAD_FD=1 hands out a socket nobody answers on instead of the PipeWire
remote; MOCK_BAD_FD=devnull hands out /dev/null.
MOCK_STREAM_SIZE=<width>x<height> has Start give that size for the stream,
as KDE's portal does; the default is to give none, as GNOME's does for a
virtual monitor. There is no ConnectToEIS and Start gives no mapping id, so
the app never takes this for a portal to speak libei with.

For RemoteDesktop sessions:
MOCK_REMOTE=0 leaves the RemoteDesktop interface out, as a desktop whose
portal has none does.
MOCK_DEVICES=<bits> is what Start grants of the devices asked for; the default
is all of them. 0 is a user who leaves remote interaction switched off.
MOCK_POINTER_OUTPUT=<sway output> makes the mock do what a desktop does with
its one cursor: a position sent for the stream puts sway's cursor at that spot
of that output. Without it the call is only printed and the cursor stays
where it is.
"""

import json
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
REMOTE_DESKTOP = "org.freedesktop.portal.RemoteDesktop"
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
        # What a RemoteDesktop session has been asked for and has been given.
        self.devices_asked = 0
        self.devices = 0
        self.node_id = None
        # The size Start gave for the stream, which is none unless asked to.
        self.size = (0, 0)

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
            "AvailableSourceTypes": dbus.UInt32(int(os.environ.get("MOCK_SOURCE_TYPES", 1 | 2 | 4))),
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
        say(f"Start (parent window {str(parent_window)!r})")
        return self.start(session_handle, options, sender, {})

    def start(self, session_handle, options, sender, more_results):
        """The dialog and the virtual monitor, for either kind of session.
        more_results is what the answer says besides the streams."""
        request = self.request(sender, options)
        session = self.sessions[str(session_handle)]

        response = int(os.environ.get("MOCK_START_RESPONSE", "0"))
        if response != 0:
            say("answering Start with response", response, "and closing the session, as the real portal does")
            session.shut("dialog declined")
            request.respond(response, {})
            return request

        delay = float(os.environ.get("MOCK_START_DELAY", "0"))
        if delay > 0:
            say(f"dialog open for {delay}s")
            GLib.timeout_add(int(delay * 1000), self.finish_start, session, request, more_results)
            return request
        self.finish_start(session, request, more_results)
        return request

    def finish_start(self, session, request, more_results):
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
        session.node_id = node_id
        properties = {"source_type": dbus.UInt32(4)}
        size = os.environ.get("MOCK_STREAM_SIZE")
        if size:
            session.size = tuple(int(side) for side in size.split("x"))
            properties["size"] = dbus.Struct([dbus.Int32(side) for side in session.size], signature="ii")
        stream = dbus.Struct(
            (dbus.UInt32(node_id), dbus.Dictionary(properties, signature="sv")),
            signature="ua{sv}",
        )
        request.respond(0, dict(more_results, streams=dbus.Array([stream], signature="(ua{sv})")))
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


class Portal(ScreenCast):
    """The same object, answering as RemoteDesktop too. A RemoteDesktop
    session takes ScreenCast's SelectSources and OpenPipeWireRemote, as the
    real portal's does, and its own Start gives the streams with the devices."""

    properties = dict(
        ScreenCast.properties,
        **{
            REMOTE_DESKTOP: {
                "version": dbus.UInt32(2),
                "AvailableDeviceTypes": dbus.UInt32(1 | 2 | 4),
            }
        },
    )

    @dbus.service.method(REMOTE_DESKTOP, in_signature="a{sv}", out_signature="o", sender_keyword="sender")
    def CreateSession(self, options, sender=None):
        request = self.request(sender, options)
        path = f"{OBJECT_PATH}/session/{sender_part(sender)}/{options['session_handle_token']}"
        self.sessions[path] = Session(self.bus, path, sender)
        say("RemoteDesktop.CreateSession ->", path)
        request.respond(0, {"session_handle": dbus.String(path)})
        return request

    @dbus.service.method(REMOTE_DESKTOP, in_signature="oa{sv}", out_signature="o", sender_keyword="sender")
    def SelectDevices(self, session_handle, options, sender=None):
        request = self.request(sender, options)
        shown = {str(k): int(v) for k, v in options.items() if k != "handle_token"}
        say("RemoteDesktop.SelectDevices", shown)
        self.sessions[str(session_handle)].devices_asked = shown.get("types", 0)
        request.respond(0, {})
        return request

    @dbus.service.method(REMOTE_DESKTOP, in_signature="osa{sv}", out_signature="o", sender_keyword="sender")
    def Start(self, session_handle, parent_window, options, sender=None):
        session = self.sessions[str(session_handle)]
        session.devices = session.devices_asked & int(os.environ.get("MOCK_DEVICES", session.devices_asked))
        say(f"RemoteDesktop.Start (parent window {str(parent_window)!r}), granting devices {session.devices}")
        return self.start(session_handle, options, sender, {"devices": dbus.UInt32(session.devices)})

    def pointer_session(self, session_handle):
        """The session, if it may move the pointer."""
        session = self.sessions.get(str(session_handle))
        if session is None or session.closed or session.node_id is None:
            raise dbus.exceptions.DBusException(
                "Session is not started", name="org.freedesktop.DBus.Error.Failed"
            )
        if not session.devices & 2:
            raise dbus.exceptions.DBusException(
                "Session is not allowed to call NotifyPointer methods",
                name="org.freedesktop.DBus.Error.Failed",
            )
        return session

    @dbus.service.method(REMOTE_DESKTOP, in_signature="oa{sv}udd", out_signature="")
    def NotifyPointerMotionAbsolute(self, session_handle, options, stream, x, y):
        session = self.pointer_session(session_handle)
        x, y = float(x), float(y)
        say(f"NotifyPointerMotionAbsolute stream {int(stream)} at {x:.1f},{y:.1f}")
        # xdg-desktop-portal's own check, and its only one: the position has
        # to lie inside the size Start gave for the stream, and a size that
        # was not given counts as 0 by 0. It does not look at the stream.
        width, height = session.size
        if not (0 <= x < width and 0 <= y < height):
            say(f"refused: not inside the {width}x{height} that Start gave for the stream")
            raise dbus.exceptions.DBusException(
                "Invalid position", name="org.freedesktop.DBus.Error.Failed"
            )
        # Answer first, then act, as the real portal does: it hands the event
        # on to the desktop without waiting for it.
        GLib.idle_add(self.put_cursor, x, y)

    @staticmethod
    def put_cursor(x, y):
        output = os.environ.get("MOCK_POINTER_OUTPUT")
        if not output:
            return False
        outputs = json.loads(subprocess.check_output(["swaymsg", "-t", "get_outputs"]))
        rect = next((o["rect"] for o in outputs if o["name"] == output), None)
        if rect is None:
            say(f"there is no output {output}; the cursor stays where it is")
            return False
        subprocess.run(["swaymsg", "-q", "seat", "seat0", "cursor", "set", str(rect["x"] + x), str(rect["y"] + y)])
        say(f"the cursor is now at {x:.1f},{y:.1f} of {output}")
        return False


def main():
    dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    name = dbus.service.BusName(BUS_NAME, bus)  # noqa: F841 (keeps the name)
    if os.environ.get("MOCK_REMOTE", "1") == "0":
        ScreenCast(bus)
    else:
        Portal(bus)
    say("ready")
    GLib.MainLoop().run()


if __name__ == "__main__":
    main()
