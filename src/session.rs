//! The portal conversation: ask the desktop for a virtual monitor and hand back
//! the PipeWire remote and node it is streamed on. Knows nothing about
//! rendering.

use std::fmt;
use std::os::fd::OwnedFd;

use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};

/// Where the virtual monitor's frames can be picked up.
pub struct Stream {
    /// A connection to the PipeWire remote the stream lives on.
    pub fd: OwnedFd,
    /// The stream's node on that remote.
    pub node_id: u32,
}

/// An open ScreenCast session. The virtual monitor exists for as long as the
/// session does.
pub struct Session {
    session: ashpd::desktop::Session<Screencast>,
}

impl Session {
    /// Opens a session for one virtual source with the cursor embedded in the
    /// frames, and starts it. The portal shows its dialog on the way; this
    /// resolves once the user has answered it.
    pub async fn start() -> Result<(Self, Stream), Error> {
        let proxy = Screencast::new().await?;
        let session = proxy.create_session(Default::default()).await?;

        match start_stream(&proxy, &session).await {
            Ok(stream) => Ok((Self { session }, stream)),
            Err(err) => {
                // Nothing to report if this fails: the portal has already
                // closed the session itself when the dialog was declined.
                let _ = session.close().await;
                Err(err)
            }
        }
    }

    /// Closes the session. The desktop removes the virtual monitor and moves
    /// the windows left on it back to a real one.
    pub async fn close(self) -> Result<(), Error> {
        Ok(self.session.close().await?)
    }
}

async fn start_stream(
    proxy: &Screencast,
    session: &ashpd::desktop::Session<Screencast>,
) -> Result<Stream, Error> {
    proxy
        .select_sources(
            session,
            SelectSourcesOptions::default()
                .set_sources(SourceType::VIRTUAL)
                .set_cursor_mode(CursorMode::EMBEDDED)
                .set_multiple(false),
        )
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

#[derive(Debug)]
pub enum Error {
    /// The portal refused, the user declined, or the bus call failed.
    Portal(ashpd::Error),
    /// The session started but came back without a stream.
    NoStream,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // ashpd's own message drops the D-Bus error underneath; the debug
            // form keeps it, and it is the part that says what went wrong.
            Self::Portal(err) => write!(f, "ScreenCast portal: {err} ({err:?})"),
            Self::NoStream => write!(f, "ScreenCast portal: the session has no stream"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ashpd::Error> for Error {
    fn from(err: ashpd::Error) -> Self {
        Self::Portal(err)
    }
}
