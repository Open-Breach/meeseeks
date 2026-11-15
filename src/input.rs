//! The teleporter: a click on the picture puts the desktop's pointer on that
//! spot of the virtual monitor. It works where the session came with a
//! pointer, see `src/session.rs` and `src/pointer.rs`.
//!
//! The desktop has no pointer of its own to offer: what gets moved is its one
//! cursor. A position sent to it takes the real cursor off this window and
//! puts it on the virtual monitor, where the user goes on working with the
//! mouse itself and leaves wherever the layout leads. The window hears no
//! more of the pointer once it is there.
//!
//! That is all of it. The click that sends the pointer is the window's own
//! and is not passed on, so what is clicked on the monitor takes a second
//! click. Moving over the picture does nothing, and neither do the other
//! buttons, the wheel and the keyboard. Nothing brings the pointer back.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::pointer::Pointer;

/// The teleporter at work on one picture. Dropping it takes it off the
/// picture again.
///
/// The pointer's connection lasts until the drop. It is the desktop's to
/// close, with the session: a desktop that finds it closed first while it
/// takes the monitor down complains in its log. So to stop, [`Input::detach`]
/// first, close the session, and drop this last.
pub struct Input {
    picture: gtk::Picture,
    click: Option<gtk::GestureClick>,
    /// The watch on the pointer's socket, if it has one.
    watch: Option<glib::Source>,
    /// Keeps the pointer, which the handlers hold weakly.
    _relay: Rc<Relay>,
}

impl Input {
    /// Has a click on `picture`, which shows the virtual monitor, send
    /// `pointer` to the spot that was clicked.
    pub fn attach(picture: &gtk::Picture, pointer: Pointer) -> Self {
        let relay = Rc::new(Relay {
            picture: picture.clone(),
            pointer,
            complained: Cell::new(false),
            cautioned: Cell::new(false),
        });

        // A click with the left button, down and up on the picture. The
        // handlers hold the relay weakly: the picture keeps them, and the
        // relay keeps the picture.
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();
        let weak = Rc::downgrade(&relay);
        click.connect_released(move |_, _, x, y| {
            if let Some(relay) = weak.upgrade() {
                relay.clicked(x, y);
            }
        });
        picture.add_controller(click.clone());

        // What the desktop says on the pointer's socket has to be taken in
        // for the pointer to work, and for the socket not to fill up.
        let watch = relay
            .pointer
            .watch()
            .and_then(|fd| {
                gio::Socket::from_fd(fd)
                    .inspect_err(|err| log::warn!("the pointer's socket cannot be watched: {err}"))
                    .ok()
            })
            .map(|socket| {
                let weak = Rc::downgrade(&relay);
                let source = gio::prelude::SocketExtManual::create_source(
                    &socket,
                    glib::IOCondition::IN,
                    gio::Cancellable::NONE,
                    None,
                    glib::Priority::DEFAULT,
                    move |_, _| match weak.upgrade() {
                        Some(relay) if relay.pointer.dispatch() => glib::ControlFlow::Continue,
                        _ => glib::ControlFlow::Break,
                    },
                );
                source.attach(None);
                source
            });
        log::info!("teleporter: a click on the picture places the pointer on the monitor");
        Self {
            picture: picture.clone(),
            click: Some(click),
            watch,
            _relay: relay,
        }
    }

    /// Takes the teleporter off the picture and stops listening to the
    /// desktop. Nothing is sent from here on.
    pub fn detach(&mut self) {
        if let Some(click) = self.click.take() {
            self.picture.remove_controller(&click);
        }
        if let Some(watch) = self.watch.take() {
            watch.destroy();
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        self.detach();
    }
}

/// What the picture's click handler works with.
struct Relay {
    picture: gtk::Picture,
    pointer: Pointer,
    /// Whether a refusal has been logged as a warning already.
    complained: Cell<bool>,
    /// Whether the pointer has been asked what stands in its way.
    cautioned: Cell<bool>,
}

impl Relay {
    /// The picture widget was clicked at `(x, y)`.
    fn clicked(self: &Rc<Self>, x: f64, y: f64) {
        if !self.pointer.ready() {
            log::debug!("teleporter: the desktop has not offered the monitor to the pointer yet");
            return;
        }
        // Not if the click was on the widget but beside the picture drawn in
        // it, nor before the first frame has arrived.
        let Some((along, frame)) = self.spot(x, y) else {
            return;
        };
        if !self.cautioned.replace(true)
            && let Some(caveat) = self.pointer.caveat(frame)
        {
            log::warn!("teleporter: {caveat}");
        }
        let (monitor_x, monitor_y) = (along.0 * frame.0, along.1 * frame.1);
        let (width, height) = (self.picture.width(), self.picture.height());
        let since = Instant::now();
        let relay = self.clone();
        glib::spawn_future_local(async move {
            let result = relay.pointer.move_to(along, frame).await;
            let took = since.elapsed().as_secs_f64() * 1000.0;
            match result {
                Ok(()) => log::debug!(
                    "teleporter: a click at {x:.0},{y:.0} of the picture sent the pointer to \
                     {monitor_x:.0},{monitor_y:.0} of the monitor, {took:.1} ms after the click; \
                     frames of {}×{} are drawn in an area of {width}×{height}",
                    frame.0,
                    frame.1
                ),
                Err(err) if !relay.complained.replace(true) => log::warn!(
                    "teleporter: the desktop did not take the pointer at \
                     {monitor_x:.0},{monitor_y:.0} of the monitor: {err}. More of these go to \
                     the debug log"
                ),
                Err(err) => log::debug!(
                    "teleporter: the desktop did not take the pointer at \
                     {monitor_x:.0},{monitor_y:.0} of the monitor: {err}"
                ),
            }
        });
    }

    /// Where on the monitor the spot `(x, y)` of the picture widget is, if
    /// the picture is drawn there: how far along the monitor, from 0 to 1
    /// each way, and the size of the monitor's frames.
    fn spot(&self, x: f64, y: f64) -> Option<((f64, f64), (f64, f64))> {
        let paintable = self.picture.paintable()?;
        let frame = (
            f64::from(paintable.intrinsic_width()),
            f64::from(paintable.intrinsic_height()),
        );
        let widget = (
            f64::from(self.picture.width()),
            f64::from(self.picture.height()),
        );
        Some((along(widget, frame, (x, y))?, frame))
    }
}

/// How far along the monitor the spot `at` of the picture widget is, from 0
/// at the monitor's left or top edge to 1 at the other, or `None` if the
/// picture is not drawn there.
///
/// The widget draws the frame as large as fits it with the frame's aspect
/// ratio kept, in the middle; that is what `ContentFit::Contain` does. With
/// the window's size locked the frame fills the widget pixel for pixel. With
/// the lock off it is scaled, and has bands beside it or above and below.
fn along(widget: (f64, f64), frame: (f64, f64), at: (f64, f64)) -> Option<(f64, f64)> {
    if widget.0 <= 0.0 || widget.1 <= 0.0 || frame.0 <= 0.0 || frame.1 <= 0.0 {
        return None;
    }
    let scale = (widget.0 / frame.0).min(widget.1 / frame.1);
    let drawn = (frame.0 * scale, frame.1 * scale);
    let origin = ((widget.0 - drawn.0) / 2.0, (widget.1 - drawn.1) / 2.0);
    let along = ((at.0 - origin.0) / drawn.0, (at.1 - origin.1) / drawn.1);
    let within = 0.0..1.0;
    (within.contains(&along.0) && within.contains(&along.1)).then_some(along)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_HD: (f64, f64) = (1920.0, 1080.0);

    /// The spot on the monitor in the frame's pixels, to the thousandth, so
    /// that the last bit of the arithmetic does not count.
    fn spot(widget: (f64, f64), frame: (f64, f64), at: (f64, f64)) -> Option<(f64, f64)> {
        let round = |value: f64| (value * 1000.0).round() / 1000.0;
        along(widget, frame, at).map(|(x, y)| (round(x * frame.0), round(y * frame.1)))
    }

    #[test]
    fn pixel_for_pixel_when_the_window_is_locked() {
        assert_eq!(spot(FULL_HD, FULL_HD, (0.0, 0.0)), Some((0.0, 0.0)));
        assert_eq!(
            spot(FULL_HD, FULL_HD, (640.5, 360.25)),
            Some((640.5, 360.25))
        );
        assert_eq!(
            spot(FULL_HD, FULL_HD, (1919.0, 1079.0)),
            Some((1919.0, 1079.0))
        );
    }

    #[test]
    fn the_far_edges_are_outside() {
        assert_eq!(spot(FULL_HD, FULL_HD, (1920.0, 500.0)), None);
        assert_eq!(spot(FULL_HD, FULL_HD, (500.0, 1080.0)), None);
        assert_eq!(spot(FULL_HD, FULL_HD, (-0.5, 500.0)), None);
    }

    #[test]
    fn scaled_down_with_the_aspect_ratio_kept() {
        let widget = (960.0, 540.0);
        assert_eq!(spot(widget, FULL_HD, (480.0, 270.0)), Some((960.0, 540.0)));
        assert_eq!(
            spot(widget, FULL_HD, (959.0, 539.0)),
            Some((1918.0, 1078.0))
        );
    }

    #[test]
    fn scaled_up() {
        let widget = (3840.0, 2160.0);
        assert_eq!(spot(widget, FULL_HD, (1000.0, 500.0)), Some((500.0, 250.0)));
    }

    #[test]
    fn bands_beside_the_picture_are_outside() {
        // A widget wider than the picture: 960 by 540 drawn in the middle of
        // 1160, with 100 to either side.
        let widget = (1160.0, 540.0);
        assert_eq!(spot(widget, FULL_HD, (99.0, 100.0)), None);
        assert_eq!(spot(widget, FULL_HD, (100.0, 0.0)), Some((0.0, 0.0)));
        assert_eq!(spot(widget, FULL_HD, (580.0, 270.0)), Some((960.0, 540.0)));
        assert_eq!(spot(widget, FULL_HD, (1060.0, 100.0)), None);
    }

    #[test]
    fn bands_above_and_below_the_picture_are_outside() {
        // A widget taller than the picture: 960 by 540 drawn in the middle of
        // 740, with 100 above and below.
        let widget = (960.0, 740.0);
        assert_eq!(spot(widget, FULL_HD, (100.0, 99.0)), None);
        assert_eq!(spot(widget, FULL_HD, (480.0, 370.0)), Some((960.0, 540.0)));
        assert_eq!(spot(widget, FULL_HD, (100.0, 640.0)), None);
    }

    #[test]
    fn nothing_before_the_first_frame() {
        assert_eq!(spot(FULL_HD, (0.0, 0.0), (10.0, 10.0)), None);
    }
}
