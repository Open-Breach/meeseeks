//! Tells when the window sits on the virtual monitor it is showing. The
//! stream then shows the window, which shows the stream, and so on: a hall
//! of mirrors.
//!
//! Nothing says which of GDK's monitors is the virtual one. It is taken to
//! be any monitor that was not there yet when the virtual monitor was asked
//! for, which a real monitor plugged in afterwards is as well.

use std::cell::RefCell;

use gtk::gdk;
use gtk::prelude::*;

#[derive(Default)]
pub struct Mirror {
    /// The monitors there were before the virtual monitor was asked for.
    before: RefCell<Vec<gdk::Monitor>>,
    /// The monitors the window is on, as its surface has entered and left
    /// them.
    under: RefCell<Vec<gdk::Monitor>>,
}

impl Mirror {
    /// Takes note of the monitors there are. To be called right before a
    /// virtual monitor is asked for.
    pub fn mark(&self, display: &gdk::Display) {
        let monitors = display.monitors();
        self.before
            .replace(monitors.iter::<gdk::Monitor>().flatten().collect());
    }

    /// The window's surface has entered `monitor`.
    pub fn entered(&self, monitor: &gdk::Monitor) {
        let mut under = self.under.borrow_mut();
        if !under.contains(monitor) {
            under.push(monitor.clone());
        }
    }

    /// The window's surface has left `monitor`.
    pub fn left(&self, monitor: &gdk::Monitor) {
        self.under.borrow_mut().retain(|other| other != monitor);
    }

    /// A monitor the window is on that has appeared since [`Mirror::mark`],
    /// if there is one. A monitor that has gone since counts for nothing,
    /// whether or not the surface was told it left.
    pub fn virtual_monitor(&self) -> Option<gdk::Monitor> {
        let before = self.before.borrow();
        self.under
            .borrow()
            .iter()
            .find(|monitor| monitor.is_valid() && !before.contains(monitor))
            .cloned()
    }
}
