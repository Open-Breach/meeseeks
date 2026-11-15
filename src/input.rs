//! Experiment: working on the virtual monitor with the mouse, through the
//! window. Off unless `MEESEEKS_INPUT` names a variant. Then the session is a
//! RemoteDesktop one, see `src/session.rs`, and this module puts the pointer
//! that comes with it, see `src/pointer.rs`, to use. It is here to find out
//! how that feels, and goes again if it does not feel good.
//!
//! The desktop has no pointer of its own to offer: what gets moved is its
//! one cursor. A position sent to it takes the real cursor off this window
//! and puts it on the virtual monitor, where the window hears no more of it.
//! The variants differ in what they make of that.
//!
//! - `teleport` counts on it. When the pointer moves onto the picture it is
//!   sent, once, to the matching spot on the monitor. From there the user
//!   works on the monitor itself and leaves it wherever the layout leads.
//!   Getting in is easy and getting out is not: on the way back, any
//!   crossing of the picture sends the pointer to the monitor again.
//! - `click` is `teleport` on a click instead: moving over the picture does
//!   nothing, and a click on it sends the pointer to that spot.
//! - `forward` passes on what the window gets while the pointer is over the
//!   picture: every position, button and scroll. Where the first position
//!   takes the cursor away, that position is all the window gets to pass on,
//!   and `forward` does what `teleport` does.
//!
//! A button or a scroll has no position of its own: the desktop applies it
//! where its cursor is. As long as the window gets them, the cursor is on the
//! window, so one that is passed on comes straight back to it, and passing
//! that one on again would go round for ever. `forward` therefore passes
//! them on only after a position the desktop took, and only one press and
//! one scroll for each spot the pointer is at: one that comes back finds the
//! pointer where it was. Once a position has been refused, no more are sent
//! while the pointer stays.
//!
//! Each stay of the pointer over the picture is one line in the log, with
//! what was passed on and how long that took. The keyboard is left alone,
//! and nothing brings the pointer back from the monitor.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::pointer::{self, Pointer};

/// The variable that switches the experiment on by naming a variant.
const VARIABLE: &str = "MEESEEKS_INPUT";

/// The evdev code of the left button; the right and the middle one follow.
const BTN_LEFT: i32 = 0x110;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Teleport,
    Click,
    Forward,
}

impl Variant {
    /// The variant `MEESEEKS_INPUT` asks for, if it asks for one.
    pub fn from_env() -> Option<Self> {
        let value = std::env::var(VARIABLE).ok()?;
        let variant = match value.trim() {
            "" => return None,
            "teleport" => Self::Teleport,
            "click" => Self::Click,
            "forward" => Self::Forward,
            other => {
                log::warn!(
                    "{VARIABLE}: `{other}` is none of `teleport`, `click` and `forward`; \
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
            Self::Click => "click",
            Self::Forward => "forward",
        }
    }

    /// How a stay of the pointer begins, for the log.
    fn begins(self) -> &'static str {
        match self {
            Self::Teleport | Self::Forward => "the pointer came onto the picture at",
            Self::Click => "the picture was clicked at",
        }
    }
}

/// The experiment at work on one picture. Dropping it takes it off the
/// picture again and lets go of any button it holds down.
///
/// The pointer's connection lasts until the drop. It is the desktop's to
/// close, with the session: a desktop that finds it closed first while it
/// takes the monitor down complains in its log. So to stop, [`Input::detach`]
/// first, close the session, and drop this last.
pub struct Input {
    picture: gtk::Picture,
    controllers: Vec<gtk::EventController>,
    /// The watch on the pointer's socket, if it has one.
    watch: Option<glib::Source>,
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
            spot: Cell::new(0),
            last_at: Cell::new(None),
            pressed_at: Cell::new(None),
            scrolled_at: Cell::new(None),
            swiping: Cell::new(false),
            wheel: Cell::default(),
            complained: Cell::new(false),
            cautioned: Cell::new(false),
        });
        // The handlers hold the relay weakly: the picture keeps them, and the
        // relay keeps the picture.
        let weak = || Rc::downgrade(&relay);
        let mut controllers = Vec::<gtk::EventController>::new();

        let motion = gtk::EventControllerMotion::new();
        if variant == Variant::Click {
            // A click with the left button, down and up on the picture.
            let click = gtk::GestureClick::builder()
                .button(gdk::BUTTON_PRIMARY)
                .build();
            let relay_weak = weak();
            click.connect_released(move |_, _, x, y| {
                if let Some(relay) = relay_weak.upgrade() {
                    relay.at(x, y);
                }
            });
            controllers.push(click.upcast());
        } else {
            // Only a pointer that moves counts. When what lies under a
            // resting pointer changes, as it does when the picture appears,
            // GTK makes up a motion of its own. Those come without a time.
            let relay_weak = weak();
            motion.connect_motion(move |motion, x, y| {
                if motion.current_event_time() != gdk::CURRENT_TIME
                    && let Some(relay) = relay_weak.upgrade()
                {
                    relay.at(x, y);
                }
            });
        }
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
                let relay_weak = weak();
                let source = gio::prelude::SocketExtManual::create_source(
                    &socket,
                    glib::IOCondition::IN,
                    gio::Cancellable::NONE,
                    None,
                    glib::Priority::DEFAULT,
                    move |_, _| match relay_weak.upgrade() {
                        Some(relay) if relay.pointer.dispatch() => glib::ControlFlow::Continue,
                        _ => glib::ControlFlow::Break,
                    },
                );
                source.attach(None);
                source
            });
        Self {
            picture: picture.clone(),
            controllers,
            watch,
            relay,
        }
    }
}

impl Input {
    /// Takes the experiment off the picture and stops listening to the
    /// desktop. Nothing is passed on from here on.
    pub fn detach(&mut self) {
        for controller in self.controllers.drain(..) {
            self.picture.remove_controller(&controller);
        }
        self.relay.left();
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

/// Something to tell the portal.
#[derive(Clone, Copy, Debug)]
enum Event {
    /// The pointer is at this spot of the monitor: how far along it, from 0
    /// to 1 each way, and the size of the monitor's frames.
    Position {
        along: (f64, f64),
        frame: (f64, f64),
    },
    /// A button, by its evdev code.
    Button { code: i32, pressed: bool },
    /// A smooth scroll, as of a touchpad.
    Scroll { dx: f64, dy: f64, finish: bool },
    /// Whole clicks of a wheel.
    Steps { horizontal: i32, vertical: i32 },
    /// The pointer has left the picture: every button that is down goes up.
    LetGo,
}

/// Something that waits to be passed on.
#[derive(Clone, Copy)]
struct Queued {
    event: Event,
    /// When the window got it.
    since: Instant,
    /// The spot the pointer was at then, by its number.
    spot: u64,
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
    /// What became of it: the events the desktop has taken or refused, how
    /// many of those it refused, and how long that took, counted from the
    /// event.
    answered: u32,
    refused: u32,
    waited: Duration,
    slowest: Duration,
    /// The buttons and scrolls the desktop was not told of.
    held_back: u32,
    /// Whether a position has been refused. No more are sent for the rest of
    /// the stay then; they would be refused as well.
    turned_down: bool,
}

/// What the picture's event handlers share: the pointer, and what is on its
/// way to it.
struct Relay {
    variant: Variant,
    picture: gtk::Picture,
    pointer: Pointer,
    /// What the desktop has yet to be told, oldest first, each with the time
    /// of the event behind it.
    queue: RefCell<VecDeque<Queued>>,
    /// Whether the queue is being worked off.
    pumping: Cell<bool>,
    /// The pointer's stay over the picture: the one going on, or the last
    /// one until it has been logged.
    visit: RefCell<Option<Visit>>,
    /// Whether the desktop took the last position it was sent.
    placed: Cell<bool>,
    /// The buttons the desktop has been told are down.
    held: RefCell<Vec<i32>>,
    /// Counts the spots of the picture the pointer has been at, so that each
    /// has a number.
    spot: Cell<u64>,
    /// Where on the picture widget the pointer last was.
    last_at: Cell<Option<(f64, f64)>>,
    /// The spots at which a button press and a scroll were last passed on.
    pressed_at: Cell<Option<u64>>,
    scrolled_at: Cell<Option<u64>>,
    /// Whether a smooth scroll has been passed on that has not ended yet.
    swiping: Cell<bool>,
    /// The parts of a wheel click that have not made a whole one yet.
    wheel: Cell<(f64, f64)>,
    /// Whether a refusal has been logged as a warning already.
    complained: Cell<bool>,
    /// Whether the pointer has been asked what stands in its way.
    cautioned: Cell<bool>,
}

impl Relay {
    /// The pointer is at `(x, y)` of the picture widget, or, with `click`,
    /// was clicked there.
    fn at(self: &Rc<Self>, x: f64, y: f64) {
        let Some((along, frame)) = self.spot(x, y) else {
            // On the widget, but beside the picture drawn in it.
            self.left();
            return;
        };
        if self.last_at.replace(Some((x, y))) != Some((x, y)) {
            self.spot.set(self.spot.get() + 1);
        }
        let arrived = !self.over();
        if arrived {
            log::debug!(
                "{} {x},{y}: frames of {}×{} drawn in a widget of {}×{}",
                self.variant.begins(),
                frame.0,
                frame.1,
                self.picture.width(),
                self.picture.height()
            );
            if !self.cautioned.replace(true)
                && let Some(caveat) = self.pointer.caveat(frame)
            {
                log::warn!("{caveat}");
            }
            // The stay before this one may still be waiting to be passed on.
            self.report();
            self.visit.replace(Some(Visit {
                since: Instant::now(),
                lasted: None,
                began_at: ((x, y), (along.0 * frame.0, along.1 * frame.1)),
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
        // would not help. A click asks each time.
        let again = match self.variant {
            Variant::Teleport => false,
            Variant::Click => true,
            Variant::Forward => !turned_down,
        };
        if arrived || again {
            self.send(Event::Position { along, frame });
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
        self.last_at.take();
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
    /// the picture is drawn there: how far along the monitor, from 0 to 1
    /// each way, and the size of the monitor's frames. There is no telling
    /// before the first frame has arrived, nor before the pointer can be
    /// moved.
    fn spot(&self, x: f64, y: f64) -> Option<((f64, f64), (f64, f64))> {
        if !self.pointer.ready() {
            return None;
        }
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

    /// Queues `event` for the desktop and sees to it that the queue is being
    /// worked off. The events go out one after the other, each once the one
    /// before has been taken or refused, so they arrive in order.
    fn send(self: &Rc<Self>, event: Event) {
        let entry = Queued {
            event,
            since: Instant::now(),
            spot: self.spot.get(),
        };
        {
            let mut queue = self.queue.borrow_mut();
            match queue.back_mut() {
                // The desktop has not been told of that position yet, and
                // only the newest matters to it.
                Some(waiting)
                    if matches!(waiting.event, Event::Position { .. })
                        && matches!(event, Event::Position { .. }) =>
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
                let Some(next) = next else {
                    break;
                };
                relay.pass_on(next).await;
            }
            relay.pumping.set(false);
            if !relay.over() {
                relay.report();
            }
        });
    }

    /// Tells the desktop of an event the window got, unless it is a button
    /// or a scroll that has to be held back.
    async fn pass_on(&self, Queued { event, since, spot }: Queued) {
        if let Err(why) = self.may_pass(event, spot) {
            return self.hold_back(event, why);
        }
        let pointer = &self.pointer;
        let result = match event {
            Event::Position { along, frame } => {
                let result = pointer.move_to(along, frame).await;
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

    /// Whether the desktop is to be told of `event`, which the window got
    /// while the pointer was at `spot`, and if not, why not. Keeps track of
    /// the buttons that are down.
    fn may_pass(&self, event: Event, spot: u64) -> Result<(), &'static str> {
        // Without the position, a press or a scroll would act where the
        // cursor is: on this window.
        const NOT_PLACED: &str = "the desktop did not take the position before it";
        match event {
            Event::Position { .. } | Event::LetGo => Ok(()),
            // A button the desktop was told is down always goes up again.
            // One it was not told of was held back when it went down, or
            // this is a release that was passed on, coming back.
            Event::Button {
                code,
                pressed: false,
            } => {
                let mut held = self.held.borrow_mut();
                let down = held
                    .iter()
                    .position(|other| *other == code)
                    .ok_or("the desktop was not told the button is down")?;
                held.swap_remove(down);
                Ok(())
            }
            Event::Button { code, .. } => {
                if !self.placed.get() {
                    return Err(NOT_PLACED);
                }
                // One that was passed on and comes back finds the pointer
                // where it was.
                if self.pressed_at.get() == Some(spot) {
                    return Err("a press was passed on at this spot already");
                }
                let mut held = self.held.borrow_mut();
                if held.contains(&code) {
                    return Err("the desktop was told the button is down already");
                }
                held.push(code);
                self.pressed_at.set(Some(spot));
                Ok(())
            }
            // The end of a smooth scroll goes with its beginning.
            Event::Scroll { finish: true, .. } => {
                if self.swiping.replace(false) {
                    Ok(())
                } else {
                    Err("no scroll that it ends was passed on")
                }
            }
            Event::Scroll { .. } | Event::Steps { .. } => {
                if !self.placed.get() {
                    return Err(NOT_PLACED);
                }
                if self.scrolled_at.get() == Some(spot) {
                    return Err("a scroll was passed on at this spot already");
                }
                self.scrolled_at.set(Some(spot));
                if matches!(event, Event::Scroll { .. }) {
                    self.swiping.set(true);
                }
                Ok(())
            }
        }
    }

    /// Notes that the desktop is not told of `event`.
    fn hold_back(&self, event: Event, why: &str) {
        if let Some(visit) = self.visit.borrow_mut().as_mut() {
            visit.held_back += 1;
        }
        log::debug!("{event:?} is not passed on: {why}");
    }

    /// `event` has been passed on, or refused, `took` after the window got
    /// it.
    fn answered(&self, event: Event, took: Duration, result: Result<(), pointer::Error>) {
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
            Ok(()) => log::debug!("{event:?}: passed on {:.1} ms after the event", ms(took)),
            Err(err) if !self.complained.replace(true) => log::warn!(
                "the desktop did not take {event:?}: {err}. More of these go to the debug log"
            ),
            Err(err) => log::debug!("the desktop did not take {event:?}: {err}"),
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
        let passed_on = match visit.answered {
            0 => "nothing yet".to_owned(),
            answered => format!(
                "{}, {:.1} ms after the window got them on average and {:.1} ms at most; \
                 refused: {}",
                answered - visit.refused,
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
            "{}: {} {x:.0},{y:.0}, which is {monitor_x:.0},{monitor_y:.0} on the monitor. The \
             pointer was off the picture {:.1} ms later. From the window in that time: positions \
             {}, button events {}, scroll events {}. Passed on to the desktop: \
             {passed_on}{held_back}",
            self.variant.name(),
            self.variant.begins(),
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
