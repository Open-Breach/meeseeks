//! The pointer that a RemoteDesktop session grants: the desktop's own cursor,
//! put where the app says. It is what the teleporter in `src/input.rs` moves.
//! Positions are all it sends: no buttons, no scrolling, no keyboard. Knows
//! nothing about GTK.
//!
//! There are two ways to move it, and a session has to settle on one.
//!
//! - libei, over the socket that `ConnectToEIS` hands out. The desktop offers
//!   devices on it, and an absolute pointer comes with regions: the areas it
//!   can reach, each with the mapping id of the stream that shows it. A
//!   position is sent as a spot in the region of this session's stream.
//! - The portal's own call, `NotifyPointerMotionAbsolute`. xdg-desktop-portal
//!   lets a position through only if it lies inside the size it was given
//!   for the stream when the session started. GNOME gives none for a virtual
//!   monitor, so there every position is refused.
//!
//! libei is taken where the portal gave no size but did give a mapping id,
//! which is GNOME. The portal's call is what is left where it did not.

use std::cell::RefCell;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::rc::Rc;

use ashpd::desktop::remote_desktop::RemoteDesktop;
use reis::PendingRequestResult;
use reis::ei;
use reis::event::{Device, DeviceCapability, EiEvent, EiEventConverter, Region};
use reis::handshake::EiHandshaker;

/// What the desktop is told this client is called.
const NAME: &str = "meeseeks";

/// What the portal said of the session's stream when it started.
pub struct Monitor {
    /// The stream's node, which the portal's calls name.
    pub node_id: u32,
    /// The size the portal gave for the stream, if it gave one.
    pub size: Option<(i32, i32)>,
    /// The id by which a libei region is known to show this stream, if the
    /// portal gave one.
    pub mapping_id: Option<String>,
}

pub struct Pointer {
    via: Via,
}

enum Via {
    Portal(Portal),
    Ei(Box<Ei>),
}

impl Pointer {
    /// Settles on a way to move the pointer of `session`, which has started,
    /// and opens it.
    pub async fn open(
        proxy: RemoteDesktop,
        session: Rc<ashpd::desktop::Session<RemoteDesktop>>,
        monitor: Monitor,
    ) -> Self {
        let ei = monitor.size.is_none() && monitor.mapping_id.is_some();
        if ei {
            // Once this call has been made the portal takes none of its own
            // any more, so there is no going back from here.
            match proxy.connect_to_eis(&session, Default::default()).await {
                Ok(fd) => match Ei::new(fd, monitor.mapping_id.clone()) {
                    Ok(ei) => {
                        log::info!(
                            "the pointer is moved over libei; the stream's mapping id is {:?}",
                            monitor.mapping_id
                        );
                        return Self {
                            via: Via::Ei(Box::new(ei)),
                        };
                    }
                    Err(err) => log::warn!("the libei socket cannot be used: {err}"),
                },
                Err(err) => log::warn!("the portal gave no libei socket: {err} ({err:?})"),
            }
        }
        log::info!(
            "the pointer is moved with the portal's own calls; the size it gave for the stream \
             is {:?}",
            monitor.size
        );
        Self {
            via: Via::Portal(Portal {
                proxy,
                session,
                monitor,
            }),
        }
    }

    /// Whether a position can be sent yet. Over libei the desktop offers the
    /// monitor's region only once the monitor is there.
    pub fn ready(&self) -> bool {
        match &self.via {
            Via::Portal(_) => true,
            Via::Ei(ei) => ei.ready(),
        }
    }

    /// What stands in the way of positions on a monitor with frames of
    /// `frame`, if something is known to.
    pub fn caveat(&self, frame: (f64, f64)) -> Option<String> {
        match &self.via {
            Via::Portal(portal) => portal.caveat(frame),
            Via::Ei(_) => None,
        }
    }

    /// Moves the pointer to a spot of the virtual monitor. `along` is how far
    /// along the monitor the spot is, from 0 at its left or top edge to 1 at
    /// the other, and `frame` the size of the monitor's frames.
    pub async fn move_to(&self, along: (f64, f64), frame: (f64, f64)) -> Result<(), Error> {
        match &self.via {
            Via::Portal(portal) => portal.move_to(along.0 * frame.0, along.1 * frame.1).await,
            Via::Ei(ei) => ei.move_to(along),
        }
    }

    /// The socket that has to be watched for the pointer to work, if there is
    /// one, as a descriptor of the caller's own: whenever it can be read
    /// from, [`Pointer::dispatch`] is to be called.
    pub fn watch(&self) -> Option<OwnedFd> {
        match &self.via {
            Via::Portal(_) => None,
            Via::Ei(ei) => ei
                .context
                .as_fd()
                .try_clone_to_owned()
                .inspect_err(|err| log::warn!("the libei socket cannot be watched: {err}"))
                .ok(),
        }
    }

    /// Takes in what the desktop has sent on the socket. Returns whether the
    /// socket is still worth watching.
    pub fn dispatch(&self) -> bool {
        match &self.via {
            Via::Portal(_) => false,
            Via::Ei(ei) => ei.dispatch(),
        }
    }
}

/// The portal's own calls.
struct Portal {
    proxy: RemoteDesktop,
    session: Rc<ashpd::desktop::Session<RemoteDesktop>>,
    monitor: Monitor,
}

impl Portal {
    /// xdg-desktop-portal lets a position through only if it lies inside the
    /// size it was given for the stream, and takes a size it was not given to
    /// be nothing.
    fn caveat(&self, frame: (f64, f64)) -> Option<String> {
        match self.monitor.size {
            None => Some(
                "the portal gave no size for the monitor's stream. It checks positions against \
                 that size, so it is likely to refuse every one sent to it"
                    .to_owned(),
            ),
            Some((width, height)) if (f64::from(width), f64::from(height)) != frame => {
                Some(format!(
                    "the portal gave {width}×{height} for the monitor's stream, but its frames \
                     are {}×{}. It checks positions against the first, so it is likely to refuse \
                     those outside it",
                    frame.0, frame.1
                ))
            }
            Some(_) => None,
        }
    }

    /// The spot is counted in the frame's pixels from the monitor's top left
    /// corner. By their source, that is how GNOME and KDE count on a monitor
    /// that is not scaled.
    async fn move_to(&self, x: f64, y: f64) -> Result<(), Error> {
        self.proxy
            .notify_pointer_motion_absolute(
                &self.session,
                self.monitor.node_id,
                x,
                y,
                Default::default(),
            )
            .await
            .map_err(Error::Portal)
    }
}

/// A libei connection as a sender of events.
struct Ei {
    context: ei::Context,
    /// The mapping id of the session's stream.
    mapping_id: Option<String>,
    state: RefCell<State>,
}

struct State {
    stage: Stage,
    /// The devices the desktop has offered.
    devices: Vec<Offered>,
    /// Counts the times a device was taken into use.
    sequence: u32,
}

enum Stage {
    /// The two sides are still telling each other what they speak.
    Greeting(EiHandshaker<'static>),
    Running(Box<EiEventConverter>),
    /// The connection is no more.
    Gone,
}

struct Offered {
    device: Device,
    /// Whether the desktop takes events from it at the moment.
    resumed: bool,
    /// Whether the desktop has been told that events are coming from it.
    emulating: bool,
}

impl Ei {
    fn new(fd: OwnedFd, mapping_id: Option<String>) -> io::Result<Self> {
        Ok(Self {
            context: ei::Context::new(UnixStream::from(fd))?,
            mapping_id,
            state: RefCell::new(State {
                stage: Stage::Greeting(EiHandshaker::new(NAME, ei::handshake::ContextType::Sender)),
                devices: Vec::new(),
                sequence: 0,
            }),
        })
    }

    fn dispatch(&self) -> bool {
        let mut state = self.state.borrow_mut();
        if matches!(state.stage, Stage::Gone) {
            return false;
        }
        if let Err(err) = self.context.read() {
            return state.end(&format!("the socket cannot be read from: {err}"));
        }
        while let Some(pending) = self.context.pending_event() {
            let event = match pending {
                PendingRequestResult::Request(event) => event,
                PendingRequestResult::ParseError(err) => {
                    return state.end(&format!("the desktop sent what cannot be read: {err}"));
                }
                // Something for an object this side has let go of already.
                PendingRequestResult::InvalidObject(_) => continue,
            };
            let mut events = Vec::new();
            match &mut state.stage {
                Stage::Greeting(greeting) => match greeting.handle_event(event) {
                    Ok(Some(greeted)) => {
                        state.stage =
                            Stage::Running(Box::new(EiEventConverter::new(&self.context, greeted)));
                    }
                    Ok(None) => {}
                    Err(err) => return state.end(&format!("the greeting failed: {err}")),
                },
                Stage::Running(converter) => {
                    if let Err(err) = converter.handle_event(event) {
                        return state.end(&format!("the desktop broke the protocol: {err}"));
                    }
                    while let Some(event) = converter.next_event() {
                        events.push(event);
                    }
                }
                Stage::Gone => return false,
            }
            for event in events {
                if !state.take_in(event, self.mapping_id.as_deref()) {
                    return false;
                }
            }
        }
        if let Err(err) = self.context.flush() {
            return state.end(&format!("the socket cannot be written to: {err}"));
        }
        true
    }

    fn ready(&self) -> bool {
        let state = self.state.borrow();
        matches!(state.stage, Stage::Running(_))
            && state.devices.iter().any(|offered| {
                offered.resumed && region_of(&offered.device, self.mapping_id.as_deref()).is_some()
            })
    }

    fn move_to(&self, along: (f64, f64)) -> Result<(), Error> {
        let mapping_id = self.mapping_id.as_deref();
        self.send(
            |device| region_of(device, mapping_id).is_some(),
            |device| {
                let (Some(region), Some(pointer)) = (
                    region_of(device, mapping_id),
                    device.interface::<ei::PointerAbsolute>(),
                ) else {
                    return;
                };
                // A region is laid out in the desktop's own units, which a
                // scaled monitor has fewer of than its frames have pixels.
                // The spot goes by how far along the monitor it is, so the
                // scale does not come into it.
                let x = f64::from(region.x) + along.0 * f64::from(region.width);
                let y = f64::from(region.y) + along.1 * f64::from(region.height);
                pointer.motion_absolute(x as f32, y as f32);
            },
        )
    }

    /// Has `events` send what it has to on a device that `fits` and that the
    /// desktop takes events from, and closes the frame they make up.
    fn send(
        &self,
        fits: impl Fn(&Device) -> bool,
        events: impl FnOnce(&Device),
    ) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        let Stage::Running(converter) = &state.stage else {
            return Err(Error::NotReady);
        };
        let serial = converter.connection().serial();
        let Some(offered) = state
            .devices
            .iter_mut()
            .find(|offered| offered.resumed && fits(&offered.device))
        else {
            return Err(Error::NotReady);
        };
        let device = offered.device.device();
        if !offered.emulating {
            state.sequence += 1;
            device.start_emulating(serial, state.sequence);
            offered.emulating = true;
        }
        events(&offered.device);
        device.frame(serial, now_micros());
        self.context
            .flush()
            .map_err(|err| Error::Ei(err.to_string()))
    }
}

impl State {
    /// Takes note of what the desktop has said. Returns whether the
    /// connection is still there.
    fn take_in(&mut self, event: EiEvent, mapping_id: Option<&str>) -> bool {
        match event {
            EiEvent::SeatAdded(added) => {
                // Positions are all this sends: no relative pointer, no
                // buttons, no scrolling and no keyboard are asked for.
                added
                    .seat
                    .bind_capabilities(DeviceCapability::PointerAbsolute.into());
            }
            EiEvent::DeviceAdded(added) => {
                let device = added.device;
                log::debug!(
                    "libei: the desktop offers the device {:?} with the regions {:?}{}",
                    device.name().unwrap_or_default(),
                    device.regions(),
                    if region_of(&device, mapping_id).is_some() {
                        "; one of them is the monitor"
                    } else {
                        ""
                    }
                );
                self.devices.push(Offered {
                    device,
                    resumed: false,
                    emulating: false,
                });
            }
            EiEvent::DeviceRemoved(removed) => {
                log::debug!(
                    "libei: the desktop takes the device {:?} back",
                    removed.device.name().unwrap_or_default()
                );
                self.devices
                    .retain(|offered| offered.device != removed.device);
            }
            EiEvent::DeviceResumed(resumed) => {
                if let Some(offered) = self.offered(&resumed.device) {
                    offered.resumed = true;
                    offered.emulating = false;
                }
            }
            EiEvent::DevicePaused(paused) => {
                log::debug!(
                    "libei: the desktop takes no events from the device {:?} for now",
                    paused.device.name().unwrap_or_default()
                );
                if let Some(offered) = self.offered(&paused.device) {
                    offered.resumed = false;
                    offered.emulating = false;
                }
            }
            EiEvent::Disconnected(disconnected) => {
                return self.end(&format!(
                    "the desktop closed it: {:?} {}",
                    disconnected.reason,
                    disconnected.explanation.unwrap_or_default()
                ));
            }
            _ => {}
        }
        true
    }

    fn offered(&mut self, device: &Device) -> Option<&mut Offered> {
        self.devices
            .iter_mut()
            .find(|offered| offered.device == *device)
    }

    /// Notes that the connection is over, and why. Returns false, for the
    /// callers that have to say whether it is still there.
    fn end(&mut self, why: &str) -> bool {
        log::warn!("the libei connection is over: {why}. The pointer can no longer be moved");
        self.stage = Stage::Gone;
        self.devices.clear();
        false
    }
}

/// The region of `device` that shows the stream with `mapping_id`, if it is
/// an absolute pointer and has one.
fn region_of<'a>(device: &'a Device, mapping_id: Option<&str>) -> Option<&'a Region> {
    let mapping_id = mapping_id?;
    if !device.has_capability(DeviceCapability::PointerAbsolute) {
        return None;
    }
    device
        .regions()
        .iter()
        .find(|region| region.mapping_id.as_deref() == Some(mapping_id))
}

/// The time libei stamps events with: microseconds of the monotonic clock.
fn now_micros() -> u64 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    u64::try_from(now.tv_sec).unwrap_or_default() * 1_000_000
        + u64::try_from(now.tv_nsec).unwrap_or_default() / 1_000
}

#[derive(Debug)]
pub enum Error {
    /// The portal refused the call, or the bus call failed.
    Portal(ashpd::Error),
    /// Something could not be written to the libei socket.
    Ei(String),
    /// The desktop offers no device for it, or not yet.
    NotReady,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // As in `session::Error`: the debug form keeps the D-Bus error.
            Self::Portal(err) => write!(f, "RemoteDesktop portal: {err} ({err:?})"),
            Self::Ei(err) => write!(f, "libei: {err}"),
            Self::NotReady => write!(f, "libei: the desktop offers no device for it"),
        }
    }
}

impl std::error::Error for Error {}
