//! The GStreamer pipeline that turns a PipeWire video node into something GTK
//! can paint. Knows nothing about portals.

use std::fmt;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gst::glib;
use gst::prelude::*;
use gtk::gdk::prelude::{PaintableExt, TextureExt, TextureExtManual};
use gtk::prelude::{GskRendererExt, SnapshotExt};
use gtk::{gdk, graphene, gsk};

/// How often the frame rates go to the debug log, in seconds.
const FRAME_LOG_INTERVAL: u32 = 5;

/// Variations still to be decided on. They are switched on by listing them in
/// `MEESEEKS_TRY`, separated by commas. Temporary: once decided, a variation
/// either becomes the only path or goes.
#[derive(Default)]
struct Tries {
    /// `srgb`: tell the sink the frames are plain sRGB, whatever the stream
    /// says. GTK only hands a frame to the compositor untouched if its colours
    /// are in GTK's default state, and Mutter tags its stream as BT.709 with a
    /// gamma 2.2 curve, which is not.
    srgb: bool,
    /// `copy`: ask for the frames in shared memory only, never as DMA-BUFs.
    /// Frames in shared memory are copied as they arrive, see
    /// `settle_buffer_path`, so the compositor has each buffer straight back
    /// and GTK draws from memory of its own. It costs the zero-copy path;
    /// it is for systems where that one misbehaves.
    copy: bool,
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
                _ => log::warn!("MEESEEKS_TRY: `{item}` is not something to try, ignoring it"),
            }
        }
        log::info!("trying: {list}");
        tries
    }
}

/// Makes GStreamer ready and registers `gtk4paintablesink`, which is linked
/// into this binary rather than looked for among the system's plugins. To be
/// called once at startup, before anything asks for an element.
pub fn init() -> Result<(), Error> {
    gst::init()?;
    gstgtk4::plugin_register_static()?;
    log::info!("gtk4paintablesink is linked into this binary and registered with GStreamer");
    Ok(())
}

/// A running `pipewiresrc ! gtk4paintablesink` pipeline. Dropping it stops
/// the pipeline.
pub struct Display {
    pipeline: gst::Pipeline,
    paintable: gdk::Paintable,
    _bus_watch: gst::bus::BusWatchGuard,
    /// The timer behind the frame rate log, when debug logging is on.
    frame_log: Option<glib::SourceId>,
    /// `pipewiresrc` only duplicates the descriptor once it connects, so ours
    /// has to stay open at least that long.
    _fd: OwnedFd,
}

impl Display {
    /// Starts showing PipeWire node `node_id` of the remote behind `fd`.
    ///
    /// `width` and `height` are the size asked of the stream. On GNOME the
    /// virtual monitor takes whatever size the format negotiation settles
    /// on, so this is what gives it its size.
    ///
    /// `on_end` is called once, from the main loop, if the pipeline fails or
    /// the stream ends on its own. Must be called on the GTK main thread.
    pub fn new(
        fd: OwnedFd,
        node_id: u32,
        (width, height): (i32, i32),
        on_end: impl FnOnce() + 'static,
    ) -> Result<Self, Error> {
        gst::init()?;
        let tries = Tries::from_env();

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

        // The size is what is asked of the stream. With any caps features the
        // sink's DMA-BUF caps pass as well as its plain memory ones; without
        // them the caps mean plain memory only.
        let wanted = gst::Caps::builder("video/x-raw")
            .field("width", width)
            .field("height", height);
        let wanted = if tries.copy {
            wanted.build()
        } else {
            wanted.any_features().build()
        };
        let filter = make("capsfilter")?;
        filter.set_property("caps", &wanted);

        let pipeline = gst::Pipeline::new();
        // Left to itself the pipeline takes the clock `pipewiresrc` offers.
        // That clock falls behind when the compositor negotiates the stream
        // anew after it has started, which Mutter does in some sessions; from
        // then on `pipewiresrc` holds every frame back until its clock has
        // caught up with the frame's timestamp, and a compositor with a
        // pool of 8 buffers runs out of them. The compositor stamps its
        // frames with the monotonic clock, which is what the system clock
        // reads, so with that one a frame is never early. Reported as
        // https://gitlab.freedesktop.org/pipewire/pipewire/-/work_items/5586
        pipeline.use_clock(Some(&gst::SystemClock::obtain()));
        pipeline.add_many([&src, &filter, &sink])?;
        gst::Element::link_many([&src, &filter, &sink])?;

        let sink_pad = sink.static_pad("sink");
        if let Some(pad) = &sink_pad {
            // In this order, so that the log shows what was negotiated.
            let source = src.downgrade();
            pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
                settle_buffer_path(&source, info)
            });
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

        let display = Self {
            pipeline,
            paintable,
            _bus_watch: bus_watch,
            frame_log,
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

    /// The frame the stream brought last, at the monitor's own size whatever
    /// size the picture is shown at, or `None` before the first has arrived.
    /// Its bytes are in main memory, so it can be worked on from any thread.
    /// Must be called on the GTK main thread.
    ///
    /// The frame is drawn once more, by `renderer`, into a texture of its
    /// own, instead of being read out of the pipeline. GTK draws either kind
    /// of frame the stream may bring: one in shared memory is its own bytes
    /// and a DMA-BUF stays the compositor's, which only a renderer knows how
    /// to read. Drawn by the renderer of the window that shows the picture,
    /// the colours come out as they do there.
    pub fn frame(&self, renderer: &gsk::Renderer) -> Option<gdk::Texture> {
        let (width, height) = (
            self.paintable.intrinsic_width(),
            self.paintable.intrinsic_height(),
        );
        if width <= 0 || height <= 0 {
            return None;
        }
        let snapshot = gtk::Snapshot::new();
        self.paintable
            .snapshot(&snapshot, f64::from(width), f64::from(height));
        let node = snapshot.to_node()?;
        let area = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
        Some(in_memory(renderer.render_texture(&node, Some(&area))))
    }
}

/// The format `gdk::Texture::download` writes: cairo's ARGB32, which is these
/// bytes in this order.
#[cfg(target_endian = "little")]
const DOWNLOADED: gdk::MemoryFormat = gdk::MemoryFormat::B8g8r8a8Premultiplied;
#[cfg(target_endian = "big")]
const DOWNLOADED: gdk::MemoryFormat = gdk::MemoryFormat::A8r8g8b8Premultiplied;

/// The same picture with its bytes in main memory. What a GL renderer hands
/// back is still on the graphics card, and reading that out takes GTK's GL
/// context, which is the main thread's: from another thread the read fails
/// and takes the app down with it. So it is read here.
fn in_memory(texture: gdk::Texture) -> gdk::Texture {
    if texture.is::<gdk::MemoryTexture>() {
        return texture;
    }
    let (width, height) = (texture.width(), texture.height());
    let stride = width as usize * 4;
    let mut bytes = vec![0; stride * height as usize];
    texture.download(&mut bytes, stride);
    gdk::MemoryTexture::new(
        width,
        height,
        DOWNLOADED,
        &glib::Bytes::from_owned(bytes),
        stride,
    )
    .upcast()
}

impl Drop for Display {
    fn drop(&mut self) {
        if let Some(timer) = self.frame_log.take() {
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

/// Runs each time caps reach the sink: once per start, and again if the
/// stream is negotiated anew. Logs the kind of buffers the sink agreed to
/// take and tells `pipewiresrc`, the `source`, how to hand them on.
///
/// DMA-BUFs go to the sink as they are. Shared memory is lent by the
/// compositor as well, but `pipewiresrc` gives it up when the stream is
/// negotiated anew, even while GTK still draws from it, which crashes. So
/// those frames are copied as they arrive. The setting is read for every
/// frame, on the thread this runs on, and the caps come before the frames
/// they describe.
fn settle_buffer_path(
    source: &glib::WeakRef<gst::Element>,
    info: &mut gst::PadProbeInfo,
) -> gst::PadProbeReturn {
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
    if let Some(source) = source.upgrade() {
        source.set_property("always-copy", !dmabuf);
    }
    let path = if dmabuf {
        "DMA-BUF"
    } else {
        "shared memory, copied"
    };
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
    /// A GStreamer element the pipeline needs cannot be made. `pipewiresrc`
    /// comes from a plugin of the system's, which may be missing.
    MissingElement(&'static str),
    /// GStreamer itself refused something.
    Gst(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingElement(factory) => write!(
                f,
                "GStreamer element `{factory}` not found; the startup checks name the package \
                 that provides `pipewiresrc`"
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
