//! Experiment: working on the virtual monitor with the mouse, through the
//! window. Off unless `MEESEEKS_INPUT` names a variant. Then the session is a
//! RemoteDesktop one, see `src/session.rs`, and this module puts the pointer
//! that comes with it to use. It is here to find out how that feels, and goes
//! again if it does not feel good.
//!
//! The portal has no pointer of its own to offer: what it moves is the
//! desktop's one cursor. A position sent to it takes the real cursor off this
//! window and puts it on the virtual monitor, where the window hears no more
//! of it. The two variants differ in what they make of that.
//!
//! - `teleport` counts on it. When the pointer moves onto the picture it is
//!   sent, once, to the matching spot on the monitor. From there the user
//!   works on the monitor itself and leaves it wherever the layout leads.
//! - `forward` passes on what the window gets while the pointer is over the
//!   picture: every position, button and scroll. Where the first position
//!   takes the cursor away, that position is all the window gets to pass on,
//!   and `forward` does what `teleport` does.
//!
//! A button or a scroll has no position of its own: the desktop applies it
//! where its cursor is. As long as the window gets them, the cursor is on the
//! window, so one that is passed on comes straight back to it. `forward`
//! therefore passes them on only after a position the portal took, and takes
//! what arrives right after one it passed on to be that one coming back. A
//! portal that refuses a position is sent no more of them while the pointer
//! stays.
//!
//! Each stay of the pointer over the picture is one line in the log, with
//! what was passed on and how long the portal took. The keyboard is left
//! alone, and nothing brings the pointer back from the monitor.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::session::{self, Pointer};

/// The variable that switches the experiment on by naming a variant.
const VARIABLE: &str = "MEESEEKS_INPUT";

/// The evdev code of the left button; the right and the middle one follow.
const BTN_LEFT: i32 = 0x110;

/// For how long after a button or a scroll has been passed on one that
/// arrives is taken to be the same one coming back. Longer than the desktop
/// takes to bring it round, shorter than a hand takes to click again.
const ECHO: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Teleport,
    Forward,
}

impl Variant {
    /// The variant `MEESEEKS_INPUT` asks for, if it asks for one.
    pub fn from_env() -> Option<Self> {
        let value = std::env::var(VARIABLE).ok()?;
        let variant = match value.trim() {
            "" => return None,
            "teleport" => Self::Teleport,
            "forward" => Self::Forward,
            other => {
                log::warn!(
                    "{VARIABLE}: `{other}` is neither `teleport` nor `forward`; \
                     the input experiment stays off"
                );
                return None;
            }
        };
        log::info!("input experiment: {}", variant.name());
        Some(variant)
    }

    fn name(self) -> &'static str {
        match self {
            Self::Teleport => "teleport",
            Self::Forward => "forward",
        }
    }
}

/// The experiment at work on one picture. Dropping it takes it off the
/// picture again and lets go of any button it holds down.
pub struct Input {
    picture: gtk::Picture,
    controllers: Vec<gtk::EventController>,
    relay: Rc<Relay>,
}

impl Input {
    /// Has `picture`, which shows the virtual monitor, pass what the pointer
    /// does over it on to `pointer`, the way `variant` says.
    pub fn attach(picture: &gtk::Picture, variant: Variant, pointer: Pointer) -> Self {
        let relay = Rc::new(Relay {
            variant,
            picture: picture.clone(),
            pointer,
            queue: RefCell::default(),
            pumping: Cell::new(false),
            visit: RefCell::default(),
            placed: Cell::new(false),
            held: RefCell::default(),
            passed_at: Cell::new(None),
            wheel: Cell::default(),
            complained: Cell::new(false),
            size_checked: Cell::new(false),
        });
        // The handlers hold the relay weakly: the picture keeps them, and the
        // relay keeps the picture.
        let weak = || Rc::downgrade(&relay);
        let mut controllers = Vec::<gtk::EventController>::new();

        // Only a pointer that moves counts. When what lies under a resting
        // pointer changes, as it does when the picture appears, GTK makes up
        // a motion of its own. Those come without a time.
        let motion = gtk::EventControllerMotion::new();
        let relay_weak = weak();
        motion.connect_motion(move |motion, x, y| {
            if motion.current_event_time() != gdk::CURRENT_TIME
                && let Some(relay) = relay_weak.upgrade()
            {
                relay.at(x, y);
            }
        });
        let relay_weak = weak();
        motion.connect_leave(move |_| {
            if let Some(relay) = relay_weak.upgrade() {
                relay.left();
            }
        });
        controllers.push(motion.upcast());

        if variant == Variant::Forward {
            // A click gesture follows one button at a time. This gets them
            // all, as they come.
            let buttons = gtk::EventControllerLegacy::new();
            let relay_weak = weak();
            buttons.connect_event(move |_, event| {
                if let Some(relay) = relay_weak.upgrade()
                    && let Some(button) = event.downcast_ref::<gdk::ButtonEvent>()
                {
                    let pressed = event.event_type() == gdk::EventType::ButtonPress;
                    relay.button(button.button(), pressed);
                }
                glib::Propagation::Proceed
            });
            controllers.push(buttons.upcast());

            let scroll =
                gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
            let relay_weak = weak();
            scroll.connect_scroll(move |scroll, dx, dy| {
                if let Some(relay) = relay_weak.upgrade() {
                    relay.scroll(dx, dy, scroll.unit());
                }
                glib::Propagation::Proceed
            });
            let relay_weak = weak();
            scroll.connect_scroll_end(move |_| {
                if let Some(relay) = relay_weak.upgrade() {
                    relay.scroll_ended();
                }
            });
            controllers.push(scroll.upcast());
        }

        for controller in &controllers {
            picture.add_controller(controller.clone());
        }
        Self {
            picture: picture.clone(),
            controllers,
            relay,
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        for controller in &self.controllers {
            self.picture.remove_controller(controller);
        }
        self.relay.left();
    }
}

/// Something to tell the portal.
#[derive(Clone, Copy, Debug)]
enum Event {
    /// The pointer is at this spot of the monitor.
    Position { x: f64, y: f64 },
    /// A button, by its evdev code.
    Button { code: i32, pressed: bool },
    /// A smooth scroll, as of a touchpad.
    Scroll { dx: f64, dy: f64, finish: bool },
    /// Whole clicks of a wheel.
    Steps { horizontal: i32, vertical: i32 },
    /// The pointer has left the picture: every button that is down goes up.
    LetGo,
}

/// One stay of the pointer over the picture, for the log.
struct Visit {
    since: Instant,
    /// How long it lasted, once it is over. It is kept until the portal has
    /// answered for all of it.
    lasted: Option<Duration>,
    /// Where on the picture widget it began, and where on the monitor that
    /// is.
    began_at: ((f64, f64), (f64, f64)),
    /// What the window got in its course.
    positions: u32,
    buttons: u32,
    scrolls: u32,
    /// What became of it: the calls the portal has answered, how many of
    /// those it refused, and how long it took, counted from the event.
    answered: u32,
    refused: u32,
    waited: Duration,
    slowest: Duration,
    /// The buttons and scrolls the portal was not told of.
    held_back: u32,
    /// Whether the portal has refused a position. It is sent no more of them
    /// for the rest of the stay then; it would refuse those as well.
    turned_down: bool,
}

/// What the picture's event handlers share: the pointer, and what is on its
/// way to it.
struct Relay {
    variant: Variant,
    picture: gtk::Picture,
    pointer: Pointer,
    /// What the portal has yet to be told, oldest first, each with the time
    /// of the event behind it.
    queue: RefCell<VecDeque<(Event, Instant)>>,
    /// Whether the queue is being worked off.
    pumping: Cell<bool>,
    /// The pointer's stay over the picture: the one going on, or the last
    /// one until it has been logged.
    visit: RefCell<Option<Visit>>,
    /// Whether the portal took the last position it was sent.
    placed: Cell<bool>,
    /// The buttons the portal has been told are down.
    held: RefCell<Vec<i32>>,
    /// When a button press or a scroll was last passed on.
    passed_at: Cell<Option<Instant>>,
    /// The parts of a wheel click that have not made a whole one yet.
    wheel: Cell<(f64, f64)>,
    /// Whether a refusal has been logged as a warning already.
    complained: Cell<bool>,
    /// Whether the size the portal gave has been held against the frames.
    size_checked: Cell<bool>,
}

impl Relay {
    /// The pointer is at `(x, y)` of the picture widget.
    fn at(self: &Rc<Self>, x: f64, y: f64) {
        let Some(spot) = self.spot(x, y) else {
            // On the widget, but beside the picture drawn in it.
            self.left();
            return;
        };
        let arrived = !self.over();
        if arrived {
            if let Some((widget, frame)) = self.sizes() {
                log::debug!(
                    "the pointer has come onto the picture: frames of {}×{} drawn in a widget \
                     of {}×{}",
                    frame.0,
                    frame.1,
                    widget.0,
                    widget.1
                );
                self.check_size(frame);
            }
            // The stay before this one may still be waiting for the portal.
            self.report();
            self.visit.replace(Some(Visit {
                since: Instant::now(),
                lasted: None,
                began_at: ((x, y), spot),
                positions: 0,
                buttons: 0,
                scrolls: 0,
                answered: 0,
                refused: 0,
                waited: Duration::ZERO,
                slowest: Duration::ZERO,
                held_back: 0,
                turned_down: false,
            }));
            // A button or a scroll of this stay waits for a position of this
            // stay.
            self.placed.set(false);
        }
        let mut turned_down = false;
        if let Some(visit) = self.visit.borrow_mut().as_mut() {
            visit.positions += 1;
            turned_down = visit.turned_down;
        }
        // `teleport` sends the pointer off once. If it is still here after
        // that, the desktop did not take it, and asking again for every move
        // would not help.
        if arrived || (self.variant == Variant::Forward && !turned_down) {
            self.send(Event::Position {
                x: spot.0,
                y: spot.1,
            });
        }
    }

    /// The pointer is no longer over the picture.
    fn left(self: &Rc<Self>) {
        if !self.over() {
            return;
        }
        // A button that goes up elsewhere would stay down on the monitor.
        if self.variant == Variant::Forward {
            self.send(Event::LetGo);
        }
        self.wheel.take();
        if let Some(visit) = self.visit.borrow_mut().as_mut() {
            visit.lasted = Some(visit.since.elapsed());
        }
        if !self.pumping.get() {
            self.report();
        }
    }

    /// GDK's button `button` went down or up.
    fn button(self: &Rc<Self>, button: u32, pressed: bool) {
        if !self.over() {
            return;
        }
        let Some(code) = evdev_button(button) else {
            return;
        };
        if let Some(visit) = self.visit.borrow_mut().as_mut() {
            visit.buttons += 1;
        }
        self.send(Event::Button { code, pressed });
    }

    /// The wheel or the touchpad scrolled by `(dx, dy)`, in `unit`.
    fn scroll(self: &Rc<Self>, dx: f64, dy: f64, unit: gdk::ScrollUnit) {
        if !self.over() {
            return;
        }
        if let Some(visit) = self.visit.borrow_mut().as_mut() {
            visit.scrolls += 1;
        }
        if unit == gdk::ScrollUnit::Wheel {
            // Clicks of a wheel, or parts of one from a wheel that tells of
            // finer steps. The portal takes whole clicks.
            let (pending_x, pending_y) = self.wheel.get();
            let (x, y) = (pending_x + dx, pending_y + dy);
            let (horizontal, vertical) = (x.trunc(), y.trunc());
            self.wheel.set((x - horizontal, y - vertical));
            if horizontal != 0.0 || vertical != 0.0 {
                self.send(Event::Steps {
                    horizontal: horizontal as i32,
                    vertical: vertical as i32,
                });
            }
        } else {
            self.send(Event::Scroll {
                dx,
                dy,
                finish: false,
            });
        }
    }

    /// The fingers have left the touchpad.
    fn scroll_ended(self: &Rc<Self>) {
        if self.over() {
            self.send(Event::Scroll {
                dx: 0.0,
                dy: 0.0,
                finish: true,
            });
        }
    }

    /// Whether the pointer is over the picture, as far as the window has
    /// been told.
    fn over(&self) -> bool {
        self.visit
            .borrow()
            .as_ref()
            .is_some_and(|visit| visit.lasted.is_none())
    }

    /// Where on the monitor the spot `(x, y)` of the picture widget is, if
    /// the picture is drawn there. There is no telling before the first
    /// frame has arrived.
    ///
    /// The spot is counted in the frame's pixels from the monitor's top left
    /// corner. By their source, that is how GNOME and KDE count on a monitor
    /// that is not scaled, and a virtual one is not unless it has been set
    /// otherwise.
    fn spot(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (widget, frame) = self.sizes()?;
        to_monitor(widget, frame, (x, y))
    }

    /// The sizes of the picture widget and of the frames drawn in it, each
    /// as width and height.
    fn sizes(&self) -> Option<((f64, f64), (f64, f64))> {
        let paintable = self.picture.paintable()?;
        let frame = (
            f64::from(paintable.intrinsic_width()),
            f64::from(paintable.intrinsic_height()),
        );
        let widget = (
            f64::from(self.picture.width()),
            f64::from(self.picture.height()),
        );
        Some((widget, frame))
    }

    /// Says once, now that the size of the frames is known, if the portal is
    /// likely to refuse positions. xdg-desktop-portal lets a position through
    /// only if it lies inside the size it was given for the stream when the
    /// session started, and takes a size it was not given to be nothing. By
    /// its source, GNOME's portal gives none for a virtual monitor.
    fn check_size(&self, frame: (f64, f64)) {
        if self.size_checked.replace(true) {
            return;
        }
        match self.pointer.reported_size() {
            None => log::warn!(
                "the portal gave no size for the monitor's stream. It checks positions against \
                 that size, so it is likely to refuse every one sent to it"
            ),
            Some((width, height)) if (f64::from(width), f64::from(height)) != frame => log::warn!(
                "the portal gave {width}×{height} for the monitor's stream, but its frames are \
                 {}×{}. It checks positions against the first, so it is likely to refuse those \
                 outside it",
                frame.0,
                frame.1
            ),
            Some(_) => {}
        }
    }

    /// Queues `event` for the portal and sees to it that the queue is being
    /// worked off. The calls go out one after the other, each once the one
    /// before has been answered, so they arrive in order.
    fn send(self: &Rc<Self>, event: Event) {
        let entry = (event, Instant::now());
        {
            let mut queue = self.queue.borrow_mut();
            match queue.back_mut() {
                // The portal has not been told of that position yet, and
                // only the newest matters to it.
                Some(waiting @ (Event::Position { .. }, _))
                    if matches!(event, Event::Position { .. }) =>
                {
                    *waiting = entry;
                }
                _ => queue.push_back(entry),
            }
        }
        if self.pumping.replace(true) {
            return;
        }
        let relay = self.clone();
        glib::spawn_future_local(async move {
            loop {
                let next = relay.queue.borrow_mut().pop_front();
                let Some((event, since)) = next else {
                    break;
                };
                relay.pass_on(event, since).await;
            }
            relay.pumping.set(false);
            if !relay.over() {
                relay.report();
            }
        });
    }

    /// Tells the portal of `event`, which the window got at `since`, unless
    /// it is a button or a scroll that has to be held back.
    async fn pass_on(&self, event: Event, since: Instant) {
        if let Err(why) = self.may_pass(event, since) {
            return self.hold_back(event, why);
        }
        let pointer = &self.pointer;
        let result = match event {
            Event::Position { x, y } => {
                let result = pointer.move_to(x, y).await;
                self.placed.set(result.is_ok());
                result
            }
            Event::Button { code, pressed } => pointer.button(code, pressed).await,
            Event::Scroll { dx, dy, finish } => pointer.scroll(dx, dy, finish).await,
            Event::Steps {
                horizontal,
                vertical,
            } => pointer.scroll_steps(horizontal, vertical).await,
            Event::LetGo => {
                let held = self.held.take();
                if held.is_empty() {
                    return;
                }
                let mut result = Ok(());
                for code in held {
                    result = result.and(pointer.button(code, false).await);
                }
                result
            }
        };
        self.answered(event, since.elapsed(), result);
    }

    /// Whether the portal is to be told of `event`, which the window got at
    /// `since`, and if not, why not. Keeps track of the buttons that are
    /// down.
    fn may_pass(&self, event: Event, since: Instant) -> Result<(), &'static str> {
        match event {
            Event::Position { .. } | Event::LetGo => Ok(()),
            // A button the portal knows down always goes up again. One it
            // does not know down was held back when it went down, or this is
            // a release that was passed on, coming back.
            Event::Button {
                code,
                pressed: false,
            } => {
                let mut held = self.held.borrow_mut();
                let down = held
                    .iter()
                    .position(|other| *other == code)
                    .ok_or("the portal does not know the button down")?;
                held.swap_remove(down);
                Ok(())
            }
            Event::Button { .. } | Event::Scroll { .. } | Event::Steps { .. } => {
                // Without the position, it would act where the cursor is:
                // on this window.
                if !self.placed.get() {
                    return Err("the portal did not take the position before it");
                }
                if self
                    .passed_at
                    .get()
                    .is_some_and(|passed| passed <= since && since < passed + ECHO)
                {
                    return Err("it came right after one that was passed on");
                }
                if let Event::Button { code, .. } = event {
                    let mut held = self.held.borrow_mut();
                    if held.contains(&code) {
                        return Err("the portal knows the button down");
                    }
                    held.push(code);
                }
                self.passed_at.set(Some(Instant::now()));
                Ok(())
            }
        }
    }

    /// Notes that the portal is not told of `event`.
    fn hold_back(&self, event: Event, why: &str) {
        if let Some(visit) = self.visit.borrow_mut().as_mut() {
            visit.held_back += 1;
        }
        log::debug!("{event:?} is not passed on: {why}");
    }

    /// The portal has answered the call for `event`, `took` after the event.
    fn answered(&self, event: Event, took: Duration, result: Result<(), session::Error>) {
        if let Some(visit) = self.visit.borrow_mut().as_mut() {
            visit.answered += 1;
            visit.waited += took;
            visit.slowest = visit.slowest.max(took);
            if result.is_err() {
                visit.refused += 1;
                visit.turned_down |= matches!(event, Event::Position { .. });
            }
        }
        match result {
            Ok(()) => log::debug!(
                "{event:?}: the portal answered {:.1} ms after the event",
                ms(took)
            ),
            Err(err) if !self.complained.replace(true) => log::warn!(
                "the portal did not take {event:?}: {err}. More of these go to the debug log"
            ),
            Err(err) => log::debug!("the portal did not take {event:?}: {err}"),
        }
    }

    /// Logs the stay that is over, if there is one that has not been logged.
    fn report(&self) {
        let mut visit = self.visit.borrow_mut();
        let Some(lasted) = visit.as_ref().and_then(|visit| visit.lasted) else {
            return;
        };
        let Some(visit) = visit.take() else {
            return;
        };
        let ((x, y), (monitor_x, monitor_y)) = visit.began_at;
        let portal = match visit.answered {
            0 => "none answered yet".to_owned(),
            answered => format!(
                "{answered} answered, {:.1} ms after the event on average and {:.1} ms at most, \
                 {} refused",
                ms(visit.waited) / f64::from(answered),
                ms(visit.slowest),
                visit.refused
            ),
        };
        let held_back = match visit.held_back {
            0 => String::new(),
            held_back => format!("; {held_back} buttons and scrolls not passed on"),
        };
        log::info!(
            "{}: the pointer came onto the picture at {x:.0},{y:.0}, which is \
             {monitor_x:.0},{monitor_y:.0} on the monitor, and was gone from it {:.1} ms later. \
             From the window in that time: positions {}, button events {}, scroll events {}. \
             Calls to the portal: {portal}{held_back}",
            self.variant.name(),
            ms(lasted),
            visit.positions,
            visit.buttons,
            visit.scrolls,
        );
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// The evdev code of the button GDK numbers `button`. GDK counts the left,
/// middle and right button as 1, 2 and 3 and, with 4 to 7 left out for the
/// wheel of old, goes on at 8.
fn evdev_button(button: u32) -> Option<i32> {
    match button {
        1 => Some(BTN_LEFT),
        2 => Some(BTN_LEFT + 2),
        3 => Some(BTN_LEFT + 1),
        8.. => i32::try_from(button - 8)
            .ok()
            .map(|more| BTN_LEFT + 3 + more),
        _ => None,
    }
}

/// Where on the monitor the spot `at` of the picture widget is, in the
/// frame's pixels, or `None` if the picture is not drawn there.
///
/// The widget draws the frame as large as fits it with the frame's aspect
/// ratio kept, in the middle; that is what `ContentFit::Contain` does. With
/// the window's size locked the frame fills the widget pixel for pixel. With
/// the lock off it is scaled, and has bands beside it or above and below.
fn to_monitor(widget: (f64, f64), frame: (f64, f64), at: (f64, f64)) -> Option<(f64, f64)> {
    if widget.0 <= 0.0 || widget.1 <= 0.0 || frame.0 <= 0.0 || frame.1 <= 0.0 {
        return None;
    }
    let scale = (widget.0 / frame.0).min(widget.1 / frame.1);
    let drawn = (frame.0 * scale, frame.1 * scale);
    let origin = ((widget.0 - drawn.0) / 2.0, (widget.1 - drawn.1) / 2.0);
    // How far along the picture, from 0 at its left or top edge to 1 at the
    // other.
    let along = ((at.0 - origin.0) / drawn.0, (at.1 - origin.1) / drawn.1);
    let within = 0.0..1.0;
    (within.contains(&along.0) && within.contains(&along.1))
        .then_some((along.0 * frame.0, along.1 * frame.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_HD: (f64, f64) = (1920.0, 1080.0);

    /// The spot on the monitor, to the thousandth, so that the last bit of
    /// the arithmetic does not count.
    fn spot(widget: (f64, f64), frame: (f64, f64), at: (f64, f64)) -> Option<(f64, f64)> {
        let round = |value: f64| (value * 1000.0).round() / 1000.0;
        to_monitor(widget, frame, at).map(|(x, y)| (round(x), round(y)))
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

    #[test]
    fn buttons_by_their_evdev_codes() {
        assert_eq!(evdev_button(1), Some(0x110));
        assert_eq!(evdev_button(2), Some(0x112));
        assert_eq!(evdev_button(3), Some(0x111));
        assert_eq!(evdev_button(8), Some(0x113));
        assert_eq!(evdev_button(9), Some(0x114));
        assert_eq!(evdev_button(4), None);
    }
}
