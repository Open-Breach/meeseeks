//! The GStreamer pipeline that turns a PipeWire video node into something GTK
//! can paint. Knows nothing about portals.

use std::fmt;
use std::os::fd::{AsRawFd, OwnedFd};

use gst::glib;
use gst::prelude::*;
use gtk::gdk;

/// The size asked of the stream. On GNOME the virtual monitor takes whatever
/// size the format negotiation settles on, so this is what makes it 1080p.
const WIDTH: i32 = 1920;
const HEIGHT: i32 = 1080;

/// An empty picture of the size the stream is asked to have, to stand in for
/// it while nothing is running.
pub fn placeholder() -> gdk::Paintable {
    gdk::Paintable::new_empty(WIDTH, HEIGHT)
}

/// A running `pipewiresrc ! gtk4paintablesink` pipeline. Dropping it stops
/// the pipeline.
pub struct Display {
    pipeline: gst::Pipeline,
    paintable: gdk::Paintable,
    _bus_watch: gst::bus::BusWatchGuard,
    /// `pipewiresrc` only duplicates the descriptor once it connects, so ours
    /// has to stay open at least that long.
    _fd: OwnedFd,
}

impl Display {
    /// Starts showing PipeWire node `node_id` of the remote behind `fd`.
    ///
    /// `on_end` is called once, from the main loop, if the pipeline fails or
    /// the stream ends on its own. Must be called on the GTK main thread.
    pub fn new(fd: OwnedFd, node_id: u32, on_end: impl FnOnce() + 'static) -> Result<Self, Error> {
        gst::init()?;

        // `path` is deprecated in favour of `target-object`, but that one
        // wants a node name or serial and the portal hands out the node id.
        let src = make("pipewiresrc")?;
        src.set_property("fd", fd.as_raw_fd());
        src.set_property("path", node_id.to_string());

        // Show every frame as it arrives instead of waiting on the clock for
        // its timestamp. There is no audio to stay in step with, and a frame
        // judged late would otherwise be dropped.
        let sink = make("gtk4paintablesink")?;
        sink.set_property("sync", false);
        let paintable = sink.property::<gdk::Paintable>("paintable");

        let pipeline = gst::Pipeline::new();
        pipeline.add_many([&src, &sink])?;

        // Any caps features, so that the size is requested whether the sink
        // gets DMA-BUFs or plain memory.
        let caps = gst::Caps::builder("video/x-raw")
            .any_features()
            .field("width", WIDTH)
            .field("height", HEIGHT)
            .build();
        src.link_filtered(&sink, &caps)?;

        if let Some(pad) = sink.static_pad("sink") {
            pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, log_buffer_path);
        }

        let mut on_end = Some(on_end);
        let bus = pipeline.bus().expect("a pipeline has a bus");
        let bus_watch = bus.add_watch_local(move |_, message| {
            let ended = match message.view() {
                gst::MessageView::Error(err) => {
                    log::error!(
                        "pipeline error: {} ({})",
                        err.error(),
                        err.debug().unwrap_or_default()
                    );
                    true
                }
                gst::MessageView::Eos(_) => {
                    log::info!("the stream ended");
                    true
                }
                _ => false,
            };
            if ended && let Some(on_end) = on_end.take() {
                on_end();
            }
            glib::ControlFlow::Continue
        })?;

        let display = Self {
            pipeline,
            paintable,
            _bus_watch: bus_watch,
            _fd: fd,
        };
        // On failure `display` is dropped here, which also takes the pipeline
        // back down.
        display.pipeline.set_state(gst::State::Playing)?;
        Ok(display)
    }

    /// The picture. It has the stream's aspect ratio once frames arrive.
    pub fn paintable(&self) -> &gdk::Paintable {
        &self.paintable
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn make(factory: &'static str) -> Result<gst::Element, Error> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|_| Error::MissingElement(factory))
}

/// Logs, once per pipeline, the kind of buffers the sink agreed to take: the
/// first caps to reach it say so. DMA-BUFs are imported as they are; anything
/// else is copied into a texture on the CPU.
fn log_buffer_path(_pad: &gst::Pad, info: &mut gst::PadProbeInfo) -> gst::PadProbeReturn {
    let Some(gst::PadProbeData::Event(event)) = &info.data else {
        return gst::PadProbeReturn::Ok;
    };
    let gst::EventView::Caps(event) = event.view() else {
        return gst::PadProbeReturn::Ok;
    };

    let caps = event.caps();
    let dmabuf = caps
        .features(0)
        .is_some_and(|features| features.contains("memory:DMABuf"));
    let path = if dmabuf { "DMA-BUF" } else { "shared memory" };
    log::info!("buffer path: {path} ({caps})");
    gst::PadProbeReturn::Remove
}

#[derive(Debug)]
pub enum Error {
    /// A GStreamer plugin the pipeline needs is not installed.
    MissingElement(&'static str),
    /// GStreamer itself refused something.
    Gst(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingElement(factory) => write!(
                f,
                "GStreamer element `{factory}` not found; see the README for the package that provides it"
            ),
            Self::Gst(message) => write!(f, "GStreamer: {message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<glib::Error> for Error {
    fn from(err: glib::Error) -> Self {
        Self::Gst(err.to_string())
    }
}

impl From<glib::BoolError> for Error {
    fn from(err: glib::BoolError) -> Self {
        Self::Gst(err.to_string())
    }
}

impl From<gst::StateChangeError> for Error {
    fn from(err: gst::StateChangeError) -> Self {
        Self::Gst(err.to_string())
    }
}
