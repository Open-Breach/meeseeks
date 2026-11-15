//! The portal conversation: ask the desktop for a virtual monitor and hand back
//! the PipeWire remote and node it is streamed on. Knows nothing about
//! rendering.
//!
//! There are two ways to ask. A RemoteDesktop session with the screen cast in
//! it also grants the pointer that `src/pointer.rs` moves, which is what the
//! teleporter in `src/input.rs` needs. It is asked for where the desktop is
//! known to give a virtual monitor that way, see
//! [`Desktop::has_teleporter`] and, for KDE Plasma, `src/plasma.rs`.
//! Everywhere else, and where the RemoteDesktop portal cannot be had, the
//! session is a plain ScreenCast one.
//!
//! Either way the desktop shows one dialog for each Start. On GNOME the
//! monitor is there whether or not the pointer was granted in it. Plasma's
//! dialog allows both or neither: denied, there is no monitor, and no second
//! session is opened to get one, which would be a second dialog.

use std::fmt;
use std::os::fd::OwnedFd;
use std::rc::Rc;

use ashpd::desktop::ResponseError;
use ashpd::desktop::remote_desktop::{DeviceType, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
use ashpd::zbus;

use crate::desktop::{self, Desktop};
use crate::plasma::{self, NotAsked};
use crate::pointer::{Monitor, Pointer};

/// Where the virtual monitor's frames can be picked up.
pub struct Stream {
    /// A connection to the PipeWire remote the stream lives on.
    pub fd: OwnedFd,
    /// The stream's node on that remote.
    pub node_id: u32,
    /// How large the monitor is, where the portal says. KDE Plasma's does:
    /// there the desktop has made the monitor by now, at a size of its own
    /// choosing, and the stream comes in no other. GNOME's says nothing for
    /// a virtual monitor, which takes the size the stream is asked for.
    pub size: Option<(i32, i32)>,
}

/// An open portal session. The virtual monitor exists for as long as the
/// session does.
pub struct Session {
    session: Handle,
}

/// The session, by the portal it was opened through.
enum Handle {
    Cast(ashpd::desktop::Session<Screencast>),
    /// Shared with the [`Pointer`], which names the session in its calls.
    Remote(Rc<ashpd::desktop::Session<RemoteDesktop>>),
}

/// A RemoteDesktop session that has been told what is wanted of it and has
/// not been started yet, so the desktop has not shown its dialog.
struct Prepared {
    proxy: RemoteDesktop,
    cast: Screencast,
    session: ashpd::desktop::Session<RemoteDesktop>,
}

impl Session {
    /// Opens a session for one virtual source with the cursor embedded in the
    /// frames, and starts it. The portal shows its dialog on the way; this
    /// resolves once the user has answered it.
    ///
    /// The pointer comes with it where the desktop grants it. It is `None`
    /// where remote control is not asked for or cannot be had, and where the
    /// user did not allow it in GNOME's dialog; the monitor is there all the
    /// same.
    ///
    /// `ask_remote_control` is the option of that name, which counts on KDE
    /// Plasma only: off, Plasma is asked for the monitor alone.
    pub async fn start(ask_remote_control: bool) -> Result<(Self, Stream, Option<Pointer>), Error> {
        if let Some(prepared) = prepare_remote(ask_remote_control).await {
            return Self::start_remote(prepared).await;
        }

        let proxy = Screencast::new().await?;
        let session = proxy.create_session(Default::default()).await?;
        match start_stream(&proxy, &session).await {
            Ok(stream) => {
                let session = Handle::Cast(session);
                Ok((Self { session }, stream, None))
            }
            Err(err) => {
                // Nothing to report if this fails: the portal has already
                // closed the session itself when the dialog was declined.
                let _ = session.close().await;
                Err(err)
            }
        }
    }

    /// Starts the RemoteDesktop session. From here on the desktop's dialog
    /// may have been shown, so whatever goes wrong is final: asking again
    /// another way would show a second one.
    async fn start_remote(prepared: Prepared) -> Result<(Self, Stream, Option<Pointer>), Error> {
        let Prepared {
            proxy,
            cast,
            session,
        } = prepared;
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

/// Gets a RemoteDesktop session ready to be started, if this is a desktop to
/// ask one of and its portal can be had: the pointer and the screen cast are
/// both chosen before the one Start, so the desktop asks once for both.
///
/// Nothing here makes the desktop show a dialog. So when a step fails, the
/// plain ScreenCast session can still be opened instead, and the user is
/// asked once all the same. The reason goes to the log.
async fn prepare_remote(ask_remote_control: bool) -> Option<Prepared> {
    let (desktop, name) = desktop::current();
    if !desktop.has_teleporter() {
        log::info!(
            "no teleporter: remote control is not asked for on this desktop ({name:?}); it \
             is used on GNOME and on KDE Plasma 6.6 or later"
        );
        return None;
    }
    if desktop == Desktop::Kde {
        let asked = match zbus::Connection::session().await {
            Ok(bus) => plasma::asks_remote_control(&bus, ask_remote_control).await,
            Err(err) => {
                log::warn!("the session bus cannot be reached to look at Plasma: {err}");
                Err(NotAsked::Older)
            }
        };
        match asked {
            Ok(()) => {}
            Err(NotAsked::SwitchedOff) => {
                log::info!(
                    "no teleporter: \"Ask for remote control\" is off in the options, so \
                     Plasma is asked for the monitor alone"
                );
                return None;
            }
            Err(NotAsked::Older) => {
                log::info!(
                    "no teleporter: remote control is not asked of a Plasma older than 6.6, \
                     which this one seems to be"
                );
                return None;
            }
        }
    }
    match try_prepare_remote().await {
        Ok(prepared) => Some(prepared),
        Err(err) => {
            log::warn!(
                "no teleporter: remote control cannot be asked for, so the monitor starts \
                 without it. {err}"
            );
            None
        }
    }
}

async fn try_prepare_remote() -> Result<Prepared, Error> {
    let remote = Error::Remote;
    let proxy = RemoteDesktop::new().await.map_err(remote)?;
    let offered = proxy.available_device_types().await.map_err(remote)?;
    if !offered.contains(DeviceType::POINTER) {
        return Err(Error::NoPointer(offered));
    }
    let cast = Screencast::new().await?;
    let session = proxy
        .create_session(Default::default())
        .await
        .map_err(remote)?;

    let chosen = async {
        proxy
            .select_devices(
                &session,
                SelectDevicesOptions::default().set_devices(DeviceType::POINTER),
            )
            .await
            .map_err(remote)?
            .response()
            .map_err(remote)?;
        cast.select_sources(&session, virtual_source())
            .await?
            .response()?;
        Ok(())
    };
    match chosen.await {
        Ok(()) => Ok(Prepared {
            proxy,
            cast,
            session,
        }),
        Err(err) => {
            let _ = session.close().await;
            Err(err)
        }
    }
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
    let size = stream.size();
    log::info!(
        "the portal started a virtual monitor on PipeWire node {node_id}, reported size: {size:?}"
    );

    let fd = proxy
        .open_pipe_wire_remote(session, Default::default())
        .await?;

    Ok(Stream { fd, node_id, size })
}

/// Starts the prepared RemoteDesktop session. Hands back, with the stream,
/// what a pointer has to know of the monitor, if the pointer was granted.
async fn start_remote_stream(
    proxy: &RemoteDesktop,
    cast: &Screencast,
    session: &ashpd::desktop::Session<RemoteDesktop>,
) -> Result<(Stream, Option<Monitor>), Error> {
    let started = proxy
        .start(session, None, Default::default())
        .await
        .map_err(Error::Remote)?
        .response()
        .map_err(|err| match err {
            ashpd::Error::Response(ResponseError::Cancelled) => Error::RemoteDeclined,
            err => Error::Remote(err),
        })?;
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
            "no teleporter: the pointer was not granted, so a click on the picture will not \
             place it. GNOME's dialog grants it only with \"Allow Remote Interaction\" \
             switched on"
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
    Ok((Stream { fd, node_id, size }, monitor))
}

#[derive(Debug)]
pub enum Error {
    /// The portal refused, the user declined, or the bus call failed.
    Portal(ashpd::Error),
    /// The same from the RemoteDesktop portal.
    Remote(ashpd::Error),
    /// The user said no in the dialog of a RemoteDesktop session, or closed
    /// it. On KDE Plasma that is how remote control is denied, and the
    /// monitor with it.
    RemoteDeclined,
    /// The RemoteDesktop portal has no pointer among the devices it offers.
    NoPointer(DeviceType),
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
            Self::RemoteDeclined => write!(
                f,
                "RemoteDesktop portal: the desktop's dialog was declined, so there is no monitor"
            ),
            Self::NoPointer(offered) => write!(
                f,
                "RemoteDesktop portal: it offers no pointer, only {offered:?}"
            ),
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
