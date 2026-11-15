//! The portal conversation: ask the desktop for a virtual monitor and hand back
//! the PipeWire remote and node it is streamed on. Knows nothing about
//! rendering.
//!
//! There are two ways to ask. The plain one is a ScreenCast session. The
//! other is for the experiment in `src/input.rs`: a RemoteDesktop session
//! with the same screen cast in it, which also grants the pointer that
//! `src/pointer.rs` moves.

use std::fmt;
use std::os::fd::OwnedFd;
use std::rc::Rc;

use ashpd::desktop::remote_desktop::{DeviceType, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};

use crate::pointer::{Monitor, Pointer};

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
            Ok((stream, monitor)) => {
                let session = Rc::new(session);
                let pointer = match monitor {
                    Some(monitor) => Some(Pointer::open(proxy, session.clone(), monitor).await),
                    None => None,
                };
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
/// Hands back, with the stream, what a pointer has to know of the monitor,
/// if the pointer was granted.
async fn start_remote_stream(
    proxy: &RemoteDesktop,
    cast: &Screencast,
    session: &ashpd::desktop::Session<RemoteDesktop>,
) -> Result<(Stream, Option<Monitor>), Error> {
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
    let mapping_id = stream.mapping_id().map(str::to_owned);
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

    let monitor = granted.then_some(Monitor {
        node_id,
        size,
        mapping_id,
    });
    Ok((Stream { fd, node_id }, monitor))
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
