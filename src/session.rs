//! The portal conversation: ask the desktop for a virtual monitor and hand back
//! the PipeWire remote and node it is streamed on. Knows nothing about
//! rendering.
//!
//! There are two ways to ask. The plain one is a ScreenCast session. The
//! other is for the experiment in `src/input.rs`: a RemoteDesktop session
//! with the same screen cast in it, which also lets the app move the pointer.

use std::fmt;
use std::os::fd::OwnedFd;
use std::rc::Rc;

use ashpd::desktop::remote_desktop::{
    Axis, DeviceType, KeyState, NotifyPointerAxisOptions, RemoteDesktop, SelectDevicesOptions,
};
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};

/// Where the virtual monitor's frames can be picked up.
pub struct Stream {
    /// A connection to the PipeWire remote the stream lives on.
    pub fd: OwnedFd,
    /// The stream's node on that remote.
    pub node_id: u32,
}

/// An open portal session. The virtual monitor exists for as long as the
/// session does.
pub struct Session {
    session: Handle,
}

/// The session, by the portal it was opened through.
enum Handle {
    Cast(ashpd::desktop::Session<Screencast>),
    /// Shared with the [`Pointer`], which names the session in every call.
    Remote(Rc<ashpd::desktop::Session<RemoteDesktop>>),
}

impl Session {
    /// Opens a session for one virtual source with the cursor embedded in the
    /// frames, and starts it. The portal shows its dialog on the way; this
    /// resolves once the user has answered it.
    pub async fn start() -> Result<(Self, Stream), Error> {
        let proxy = Screencast::new().await?;
        let session = proxy.create_session(Default::default()).await?;

        match start_stream(&proxy, &session).await {
            Ok(stream) => {
                let session = Handle::Cast(session);
                Ok((Self { session }, stream))
            }
            Err(err) => {
                // Nothing to report if this fails: the portal has already
                // closed the session itself when the dialog was declined.
                let _ = session.close().await;
                Err(err)
            }
        }
    }

    /// The same monitor from a RemoteDesktop session, which asks for the
    /// pointer along with the screen cast. It is still one session and one
    /// dialog. The pointer is `None` if the user did not grant it; the
    /// monitor is there all the same.
    pub async fn start_with_pointer() -> Result<(Self, Stream, Option<Pointer>), Error> {
        let proxy = RemoteDesktop::new().await.map_err(Error::Remote)?;
        let cast = Screencast::new().await?;
        let session = proxy
            .create_session(Default::default())
            .await
            .map_err(Error::Remote)?;

        match start_remote_stream(&proxy, &cast, &session).await {
            Ok((stream, granted, size)) => {
                let session = Rc::new(session);
                let pointer = granted.then(|| Pointer {
                    proxy,
                    session: session.clone(),
                    node_id: stream.node_id,
                    size,
                });
                let session = Handle::Remote(session);
                Ok((Self { session }, stream, pointer))
            }
            Err(err) => {
                // As in `start`.
                let _ = session.close().await;
                Err(err)
            }
        }
    }

    /// Closes the session. The desktop removes the virtual monitor and moves
    /// the windows left on it back to a real one.
    pub async fn close(self) -> Result<(), Error> {
        match self.session {
            Handle::Cast(session) => Ok(session.close().await?),
            Handle::Remote(session) => session.close().await.map_err(Error::Remote),
        }
    }
}

/// What is asked of the screen cast: one virtual source with the cursor
/// embedded in the frames.
fn virtual_source() -> SelectSourcesOptions {
    SelectSourcesOptions::default()
        .set_sources(SourceType::VIRTUAL)
        .set_cursor_mode(CursorMode::EMBEDDED)
        .set_multiple(false)
}

async fn start_stream(
    proxy: &Screencast,
    session: &ashpd::desktop::Session<Screencast>,
) -> Result<Stream, Error> {
    proxy
        .select_sources(session, virtual_source())
        .await?
        .response()?;

    let streams = proxy
        .start(session, None, Default::default())
        .await?
        .response()?;
    let stream = streams.streams().first().ok_or(Error::NoStream)?;
    let node_id = stream.pipe_wire_node_id();
    log::info!(
        "the portal started a virtual monitor on PipeWire node {node_id}, reported size: {:?}",
        stream.size()
    );

    let fd = proxy
        .open_pipe_wire_remote(session, Default::default())
        .await?;

    Ok(Stream { fd, node_id })
}

/// The RemoteDesktop way to the same stream: the pointer and the screen cast
/// are both chosen before the one Start, so the portal asks once for both.
/// Hands back, with the stream, whether the pointer was granted and the size
/// the portal gave for the stream, if it gave one.
async fn start_remote_stream(
    proxy: &RemoteDesktop,
    cast: &Screencast,
    session: &ashpd::desktop::Session<RemoteDesktop>,
) -> Result<(Stream, bool, Option<(i32, i32)>), Error> {
    proxy
        .select_devices(
            session,
            SelectDevicesOptions::default().set_devices(DeviceType::POINTER),
        )
        .await
        .map_err(Error::Remote)?
        .response()
        .map_err(Error::Remote)?;
    cast.select_sources(session, virtual_source())
        .await?
        .response()?;

    let started = proxy
        .start(session, None, Default::default())
        .await
        .map_err(Error::Remote)?
        .response()
        .map_err(Error::Remote)?;
    let stream = started.streams().first().ok_or(Error::NoStream)?;
    // A portal that takes no virtual source in a RemoteDesktop session may
    // cast the real screens instead, and this window would show itself in
    // them.
    if let Some(kind) = stream.source_type()
        && kind != SourceType::VIRTUAL
    {
        return Err(Error::NotVirtual(kind));
    }
    let node_id = stream.pipe_wire_node_id();
    let size = stream.size();
    let granted = started.devices().contains(DeviceType::POINTER);
    log::info!(
        "the portal started a virtual monitor on PipeWire node {node_id} in a RemoteDesktop \
         session, reported size: {size:?}, devices granted: {:?}",
        started.devices()
    );
    if !granted {
        log::warn!(
            "the pointer was not granted, so the monitor runs without the input experiment. \
             GNOME's dialog grants it only with \"Allow Remote Interaction\" switched on"
        );
    }

    let fd = cast
        .open_pipe_wire_remote(session, Default::default())
        .await?;

    Ok((Stream { fd, node_id }, granted, size))
}

/// The pointer of a RemoteDesktop session: the desktop's own, moved on the
/// app's word. Every position is one on the session's virtual monitor.
pub struct Pointer {
    proxy: RemoteDesktop,
    session: Rc<ashpd::desktop::Session<RemoteDesktop>>,
    node_id: u32,
    size: Option<(i32, i32)>,
}

impl Pointer {
    /// The size the portal gave for the monitor's stream when the session
    /// started, in the units it takes positions in. Not every portal gives
    /// one.
    pub fn reported_size(&self) -> Option<(i32, i32)> {
        self.size
    }

    /// Moves the pointer to a spot on the virtual monitor, counted from the
    /// monitor's top left corner.
    pub async fn move_to(&self, x: f64, y: f64) -> Result<(), Error> {
        self.proxy
            .notify_pointer_motion_absolute(&self.session, self.node_id, x, y, Default::default())
            .await
            .map_err(Error::Remote)
    }

    /// Presses or releases a button, named by its Linux evdev code.
    pub async fn button(&self, code: i32, pressed: bool) -> Result<(), Error> {
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        self.proxy
            .notify_pointer_button(&self.session, code, state, Default::default())
            .await
            .map_err(Error::Remote)
    }

    /// Scrolls smoothly, the way a touchpad does. `finish` ends the gesture.
    pub async fn scroll(&self, dx: f64, dy: f64, finish: bool) -> Result<(), Error> {
        let options = NotifyPointerAxisOptions::default().set_finish(finish);
        self.proxy
            .notify_pointer_axis(&self.session, dx, dy, options)
            .await
            .map_err(Error::Remote)
    }

    /// Scrolls by whole clicks of a wheel, to the right and down if positive.
    pub async fn scroll_steps(&self, horizontal: i32, vertical: i32) -> Result<(), Error> {
        for (axis, steps) in [(Axis::Horizontal, horizontal), (Axis::Vertical, vertical)] {
            if steps != 0 {
                self.proxy
                    .notify_pointer_axis_discrete(&self.session, axis, steps, Default::default())
                    .await
                    .map_err(Error::Remote)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    /// The portal refused, the user declined, or the bus call failed.
    Portal(ashpd::Error),
    /// The same from the RemoteDesktop portal.
    Remote(ashpd::Error),
    /// The session started but came back without a stream.
    NoStream,
    /// The RemoteDesktop session came back with a stream of something else
    /// than a virtual monitor.
    NotVirtual(SourceType),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // ashpd's own message drops the D-Bus error underneath; the debug
            // form keeps it, and it is the part that says what went wrong.
            Self::Portal(err) => write!(f, "ScreenCast portal: {err} ({err:?})"),
            Self::Remote(err) => write!(f, "RemoteDesktop portal: {err} ({err:?})"),
            Self::NoStream => write!(f, "ScreenCast portal: the session has no stream"),
            Self::NotVirtual(kind) => write!(
                f,
                "RemoteDesktop portal: the stream is not of a virtual monitor but of {kind:?}"
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<ashpd::Error> for Error {
    fn from(err: ashpd::Error) -> Self {
        Self::Portal(err)
    }
}
