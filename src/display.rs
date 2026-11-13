//! The GStreamer pipeline that turns a PipeWire video node into something GTK
//! can paint. Knows nothing about portals.

use std::cell::Cell;
use std::fmt;
use std::os::fd::{AsRawFd, OwnedFd};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gst::glib;
use gst::prelude::*;
use gtk::gdk;
use gtk::gdk::prelude::PaintableExt;

/// The size asked of the stream. On GNOME the virtual monitor takes whatever
/// size the format negotiation settles on, so this is what makes it 1080p.
const WIDTH: i32 = 1920;
const HEIGHT: i32 = 1080;

/// An empty picture of the size the stream is asked to have, to stand in for
/// it while nothing is running.
pub fn placeholder() -> gdk::Paintable {
    gdk::Paintable::new_empty(WIDTH, HEIGHT)
}

/// How often the frame rates go to the debug log, in seconds.
const FRAME_LOG_INTERVAL: u32 = 5;

/// Variations to compare while the smoothest configuration is being looked
/// for. They are switched on by listing them in `MEESEEKS_TRY`, separated by
/// commas. Temporary: the one that wins becomes the only path and this goes.
#[derive(Default)]
struct Tries {
    /// `srgb`: tell the sink the frames are plain sRGB, whatever the stream
    /// says. GTK only hands a frame to the compositor untouched if its colours
    /// are in GTK's default state, and Mutter tags its stream as BT.709 with a
    /// gamma 2.2 curve, which is not.
    srgb: bool,
    /// `copy`: have `pipewiresrc` copy every frame and give the compositor's
    /// buffer straight back, instead of lending it out until GTK is done
    /// with it. Mutter has 8 buffers and stops sending when none is free; this
    /// takes the timing of GTK's renderer out of when they return. It costs
    /// the DMA-BUF path: a copy only makes sense from shared memory, so only
    /// that is asked for.
    copy: bool,
    /// `reconnect`: take the stream as shared memory for a moment, then
    /// disconnect and connect again asking for what is really wanted. Every
    /// stuttering session seen in Mutter's log had its stream renegotiated
    /// after it had started, which Mutter does once per stream; a connection
    /// made after that should be spared it. The first connection asks for
    /// shared memory because that has drawn the renegotiation every time.
    reconnect: bool,
}

impl Tries {
    fn from_env() -> Self {
        let mut tries = Self::default();
        let Ok(list) = std::env::var("MEESEEKS_TRY") else {
            return tries;
        };
        for item in list
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            match item {
                "srgb" => tries.srgb = true,
                "copy" => tries.copy = true,
                "reconnect" => tries.reconnect = true,
                _ => log::warn!("MEESEEKS_TRY: `{item}` is not something to try, ignoring it"),
            }
        }
        log::info!("trying: {list}");
        tries
    }
}

/// How long the first connection is kept when trying `reconnect`. The
/// renegotiation it waits for has been over well within a second.
const RECONNECT_AFTER: Duration = Duration::from_secs(2);

/// A running `pipewiresrc ! gtk4paintablesink` pipeline. Dropping it stops
/// the pipeline.
pub struct Display {
    pipeline: gst::Pipeline,
    paintable: gdk::Paintable,
    _bus_watch: gst::bus::BusWatchGuard,
    /// The timer behind the frame rate log, when debug logging is on.
    frame_log: Option<glib::SourceId>,
    /// The timer behind `reconnect`, until it has fired.
    reconnect: Rc<Cell<Option<glib::SourceId>>>,
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
        let tries = Tries::from_env();

        // `path` is deprecated in favour of `target-object`, but that one
        // wants a node name or serial and the portal hands out the node id.
        let src = make("pipewiresrc")?;
        src.set_property("fd", fd.as_raw_fd());
        src.set_property("path", node_id.to_string());
        src.set_property("always-copy", tries.copy);

        // Show every frame as it arrives instead of waiting on the clock for
        // its timestamp. There is no audio to stay in step with, and a frame
        // judged late would otherwise be dropped.
        let sink = make("gtk4paintablesink")?;
        sink.set_property("sync", false);
        let paintable = sink.property::<gdk::Paintable>("paintable");

        // The size is what is asked of the stream. With any caps features it
        // is asked whether the sink gets DMA-BUFs or plain memory; without
        // them the caps mean plain memory only.
        let size = || {
            gst::Caps::builder("video/x-raw")
                .field("width", WIDTH)
                .field("height", HEIGHT)
        };
        let plain_memory = size().build();
        let wanted = if tries.copy {
            plain_memory.clone()
        } else {
            size().any_features().build()
        };
        let filter = make("capsfilter")?;
        filter.set_property(
            "caps",
            if tries.reconnect {
                &plain_memory
            } else {
                &wanted
            },
        );

        let pipeline = gst::Pipeline::new();
        pipeline.add_many([&src, &filter, &sink])?;
        gst::Element::link_many([&src, &filter, &sink])?;

        let sink_pad = sink.static_pad("sink");
        if let Some(pad) = &sink_pad {
            // In this order, so that the log shows what was negotiated.
            pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, log_buffer_path);
            if tries.srgb {
                pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, present_as_srgb);
            }
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

        let frame_log = sink_pad
            .filter(|_| log::log_enabled!(log::Level::Debug))
            .map(|pad| log_frame_rates(&pad, &paintable));

        let reconnect = Rc::new(Cell::new(None));
        if tries.reconnect {
            let (pipeline, fired) = (pipeline.clone(), reconnect.clone());
            let timer = glib::timeout_add_local(RECONNECT_AFTER, move || {
                fired.set(None);
                log::info!("reconnecting to the stream");
                // `pipewiresrc` drops its connection to the stream when it
                // negotiates afresh, which coming back up from READY makes it.
                let _ = pipeline.set_state(gst::State::Ready);
                filter.set_property("caps", &wanted);
                if pipeline.set_state(gst::State::Playing).is_err() {
                    log::error!("could not start the pipeline again");
                }
                glib::ControlFlow::Break
            });
            reconnect.set(Some(timer));
        }

        let display = Self {
            pipeline,
            paintable,
            _bus_watch: bus_watch,
            frame_log,
            reconnect,
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
        if let Some(timer) = self.frame_log.take() {
            timer.remove();
        }
        if let Some(timer) = self.reconnect.take() {
            timer.remove();
        }
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn make(factory: &'static str) -> Result<gst::Element, Error> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|_| Error::MissingElement(factory))
}

/// Logs the kind of buffers the sink agreed to take, each time caps reach it:
/// once per start, and again if the stream is negotiated anew. DMA-BUFs are
/// imported as they are; anything else is copied into a texture on the CPU.
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
    gst::PadProbeReturn::Ok
}

/// Rewrites the caps on their way into the sink so that they claim sRGB. The
/// frames themselves are untouched; only what the sink believes about their
/// colours changes.
fn present_as_srgb(_pad: &gst::Pad, info: &mut gst::PadProbeInfo) -> gst::PadProbeReturn {
    let rewritten = match &info.data {
        Some(gst::PadProbeData::Event(event)) => match event.view() {
            gst::EventView::Caps(caps_event) => {
                let mut caps = caps_event.caps_owned();
                for structure in caps.make_mut().iter_mut() {
                    structure.set("colorimetry", "sRGB");
                }
                gst::event::Caps::builder(&caps)
                    .seqnum(event.seqnum())
                    .build()
            }
            _ => return gst::PadProbeReturn::Ok,
        },
        _ => return gst::PadProbeReturn::Ok,
    };
    info.data = Some(gst::PadProbeData::Event(rewritten));
    gst::PadProbeReturn::Ok
}

#[derive(Default)]
struct FrameCounters {
    arrived: AtomicU64,
    handed_on: AtomicU64,
    last_arrival_us: AtomicU64,
    /// The time between each frame and the one before it, in microseconds,
    /// since the last log line.
    gaps_us: Mutex<Vec<u64>>,
    /// Frames given to the sink that nothing has let go of yet. Each one
    /// keeps a PipeWire buffer away from the compositor, which has few.
    held: AtomicU64,
    most_held: AtomicU64,
}

/// Logs, every few seconds at debug level, what happens to the frames:
///
/// - how many a second arrive from PipeWire; low means the compositor sends
///   few,
/// - the median and the longest time between two of them,
/// - how many a second the sink hands to GTK to draw; lower than the first
///   number means this process is not keeping up,
/// - the most buffers that were held at once; near the compositor's pool
///   size means it is being starved from this end.
fn log_frame_rates(pad: &gst::Pad, paintable: &gdk::Paintable) -> glib::SourceId {
    let counters = Arc::new(FrameCounters::default());
    let started = Instant::now();

    let arriving = counters.clone();
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
        let now = started.elapsed().as_micros() as u64;
        let last = arriving.last_arrival_us.swap(now, Ordering::Relaxed);
        if last != 0 {
            arriving.gaps_us.lock().unwrap().push(now - last);
        }
        arriving.arrived.fetch_add(1, Ordering::Relaxed);

        if let Some(gst::PadProbeData::Buffer(buffer)) = &info.data {
            let held = arriving.held.fetch_add(1, Ordering::Relaxed) + 1;
            arriving.most_held.fetch_max(held, Ordering::Relaxed);
            // SAFETY: the buffer is alive, and `frame_released` takes back
            // the reference to the counters that is leaked here, exactly once.
            unsafe {
                gst::ffi::gst_mini_object_weak_ref(
                    buffer.as_mut_ptr().cast(),
                    Some(frame_released),
                    Arc::into_raw(arriving.clone()).cast_mut().cast(),
                );
            }
        }
        gst::PadProbeReturn::Ok
    });

    let handing_on = counters.clone();
    paintable.connect_invalidate_contents(move |_| {
        handing_on.handed_on.fetch_add(1, Ordering::Relaxed);
    });

    glib::timeout_add_seconds_local(FRAME_LOG_INTERVAL, move || {
        let per_second = |count: &AtomicU64| {
            count.swap(0, Ordering::Relaxed) as f64 / f64::from(FRAME_LOG_INTERVAL)
        };
        let arrived = per_second(&counters.arrived);
        let handed_on = per_second(&counters.handed_on);

        let mut gaps = std::mem::take(&mut *counters.gaps_us.lock().unwrap());
        gaps.sort_unstable();
        let median_ms = gaps.get(gaps.len() / 2).map_or(0, |gap| gap / 1000);
        let longest_ms = gaps.last().map_or(0, |gap| gap / 1000);

        let held_now = counters.held.load(Ordering::Relaxed);
        let most_held = counters.most_held.swap(held_now, Ordering::Relaxed);

        log::debug!(
            "last {FRAME_LOG_INTERVAL} s: {arrived:.1} frames/s from PipeWire, \
             gaps median {median_ms} ms and longest {longest_ms} ms, \
             {handed_on:.1} frames/s handed to GTK, at most {most_held} buffers held"
        );
        glib::ControlFlow::Continue
    })
}

/// Called by GStreamer when the last reference to a frame's buffer is gone.
unsafe extern "C" fn frame_released(
    counters: glib::ffi::gpointer,
    _buffer: *mut gst::ffi::GstMiniObject,
) {
    // SAFETY: `counters` is what `Arc::into_raw` returned when the weak
    // reference was taken, and GStreamer calls this once for each.
    let counters = unsafe { Arc::from_raw(counters.cast_const().cast::<FrameCounters>()) };
    counters.held.fetch_sub(1, Ordering::Relaxed);
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
