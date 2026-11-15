#!/usr/bin/env python3
"""A window that fills the virtual monitor and says where the pointer is on it.

It goes fullscreen on the monitor with the highest connector number, which is
the one made last, and prints every pointer event it gets with its position:
"enter", "at", "leave", "press", "release" and "scroll". It also shows
the position, so that it can be read off a picture of the monitor.
"""

import sys

import gi

gi.require_version("Gdk", "4.0")
gi.require_version("Gtk", "4.0")
from gi.repository import Gdk, Gtk  # noqa: E402


def say(*args):
    print("[landing]", *args, flush=True)


def activate(app):
    monitors = list(Gdk.Display.get_default().get_monitors())
    monitor = max(monitors, key=lambda monitor: monitor.get_connector() or "")
    geometry = monitor.get_geometry()
    say(f"on {monitor.get_connector()}, {geometry.width}x{geometry.height} at {geometry.x},{geometry.y}, scale {monitor.get_scale_factor()}")

    label = Gtk.Label(label="the virtual monitor")
    label.add_css_class("title-1")

    def at(what, x, y):
        say(f"{what} {x:.1f},{y:.1f}")
        label.set_label(f"pointer at {x:.0f},{y:.0f}")

    motion = Gtk.EventControllerMotion()
    motion.connect("enter", lambda _c, x, y: at("enter", x, y))
    motion.connect("motion", lambda _c, x, y: at("at", x, y))
    motion.connect("leave", lambda _c: say("leave"))
    label.add_controller(motion)

    click = Gtk.GestureClick(button=0)
    click.connect("pressed", lambda g, _n, x, y: say(f"press {g.get_current_button()} at {x:.1f},{y:.1f}"))
    click.connect("released", lambda g, _n, x, y: say(f"release {g.get_current_button()} at {x:.1f},{y:.1f}"))
    label.add_controller(click)

    scroll = Gtk.EventControllerScroll(flags=Gtk.EventControllerScrollFlags.BOTH_AXES)
    scroll.connect("scroll", lambda _c, dx, dy: say(f"scroll {dx:.2f},{dy:.2f}") or False)
    label.add_controller(scroll)

    window = Gtk.ApplicationWindow(application=app, title="Landing", child=label)
    window.fullscreen_on_monitor(monitor)
    window.present()


app = Gtk.Application(application_id="ch.meeseeks.Landing")
app.connect("activate", activate)
app.run(sys.argv[:1])
