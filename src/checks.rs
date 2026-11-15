//! The checks run at startup: whether the system has what the app needs.
//! Each check is one small function that returns a [`Check`]; to add one,
//! write it and list it in [`run`]. Knows nothing about the portal session
//! or the pipeline.

use std::fmt::Write as _;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ashpd::zbus;
use gtk::prelude::*;

use crate::desktop::{self, Desktop};
use crate::renderer::Renderer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    /// Worth telling, but no reason not to start.
    Warn,
    /// The app cannot work.
    Fail,
}

/// The outcome of one check.
#[derive(Debug)]
pub struct Check {
    pub name: String,
    pub status: Status,
    /// What is wrong and what to do about it.
    pub hint: Option<String>,
}

impl Check {
    fn pass(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: Status::Pass,
            hint: None,
        }
    }

    fn warn(name: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: Status::Warn,
            hint: Some(hint.into()),
        }
    }

    fn fail(name: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: Status::Fail,
            hint: Some(hint.into()),
        }
    }
}

/// Runs every check, in the order they are shown. `asked` is the renderer
/// GTK has been asked for, if it is a known one. Must be called on the GTK
/// main thread, after GTK has been initialised.
pub async fn run(asked: Option<Renderer>) -> Vec<Check> {
    let bus = session_bus().await;
    let portal = portal(&bus).await;
    let portal_answers = portal.status == Status::Pass;
    let screencast = screencast(&bus, portal_answers).await;
    let remote_control = remote_control(&bus, portal_answers).await;
    vec![
        wayland_session(),
        desktop(),
        pipewire(),
        portal,
        screencast,
        remote_control,
        gst_element(
            "pipewiresrc",
            "Install the GStreamer PipeWire plugin (Fedora: pipewire-gstreamer; \
             Debian and Ubuntu: gstreamer1.0-pipewire; Arch: gst-plugin-pipewire).",
        ),
        gtk_renderer(asked),
    ]
}

/// Whether any check says the app cannot work.
pub fn failed(checks: &[Check]) -> bool {
    checks.iter().any(|check| check.status == Status::Fail)
}

/// The results as text, one line per check.
pub fn report(checks: &[Check]) -> String {
    let mut text = String::from("Meeseeks startup checks\n");
    for check in checks {
        let mark = match check.status {
            Status::Pass => "ok  ",
            Status::Warn => "warn",
            Status::Fail => "FAIL",
        };
        let _ = write!(text, "[{mark}] {}", check.name);
        if let Some(hint) = &check.hint {
            let _ = write!(text, ": {hint}");
        }
        text.push('\n');
    }
    text
}

fn env_is_set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn wayland_session() -> Check {
    let name = "Wayland session";
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|kind| kind == "wayland")
        || env_is_set("WAYLAND_DISPLAY")
    {
        Check::pass(name)
    } else {
        Check::fail(
            name,
            "This is not a Wayland session. X11 is not supported; log in to a \
             GNOME or KDE Plasma session on Wayland.",
        )
    }
}

fn desktop() -> Check {
    let name = "Desktop";
    match desktop::current() {
        (Desktop::Gnome | Desktop::Kde, current) => Check::pass(format!("{name}: {current}")),
        (Desktop::Other, current) if current.is_empty() => Check::warn(
            name,
            "XDG_CURRENT_DESKTOP is not set. Meeseeks is made for GNOME and KDE Plasma; \
             it may not work here.",
        ),
        (Desktop::Other, current) => Check::warn(
            format!("{name}: {current}"),
            "Meeseeks is made for GNOME and KDE Plasma; it may not work here.",
        ),
    }
}

/// Where the PipeWire daemon's socket should be, the way PipeWire's own
/// clients look for it.
fn pipewire_sockets() -> Vec<PathBuf> {
    let remote = std::env::var_os("PIPEWIRE_REMOTE")
        .filter(|remote| !remote.is_empty())
        .unwrap_or_else(|| "pipewire-0".into());
    if Path::new(&remote).is_absolute() {
        return vec![remote.into()];
    }
    ["PIPEWIRE_RUNTIME_DIR", "XDG_RUNTIME_DIR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(&dir).join(&remote))
        .collect()
}

/// A connection to the daemon's socket is taken as the daemon running; no
/// PipeWire is spoken over it.
fn pipewire() -> Check {
    let name = "PipeWire running";
    let sockets = pipewire_sockets();
    if sockets
        .iter()
        .any(|socket| UnixStream::connect(socket).is_ok())
    {
        return Check::pass(name);
    }
    let tried = match sockets.first() {
        Some(socket) => format!("No PipeWire daemon answers on {}.", socket.display()),
        None => "XDG_RUNTIME_DIR is not set, so there is nowhere to look for PipeWire.".into(),
    };
    Check::fail(
        name,
        format!(
            "{tried} Start it with `systemctl --user start pipewire`, or install \
             it (Fedora, Debian, Ubuntu and Arch: pipewire)."
        ),
    )
}

const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
/// The bit for virtual monitors in the portal's `AvailableSourceTypes`.
const SOURCE_TYPE_VIRTUAL: u32 = 4;
/// How long the portal gets to answer. Asking for it can make the session
/// bus start it first, which takes a moment.
const BUS_TIMEOUT: Duration = Duration::from_secs(10);

/// A connection of the checks' own, so that their calls can time out.
async fn session_bus() -> zbus::Result<zbus::Connection> {
    zbus::connection::Builder::session()
        .method_timeout(BUS_TIMEOUT)
        .build()
        .await
}

async fn portal(bus: &zbus::Result<zbus::Connection>) -> Check {
    let name = "xdg-desktop-portal";
    let bus = match bus {
        Ok(bus) => bus,
        Err(err) => {
            return Check::fail(name, format!("The session bus cannot be reached: {err}."));
        }
    };
    match ping_portal(bus).await {
        Ok(()) => Check::pass(name),
        Err(err) => {
            log::warn!("{PORTAL_NAME} does not answer: {err}");
            Check::fail(
                name,
                "Nothing answers as the portal on the session bus. Install \
                 xdg-desktop-portal and the backend for your desktop \
                 (xdg-desktop-portal-gnome or xdg-desktop-portal-kde).",
            )
        }
    }
}

async fn ping_portal(bus: &zbus::Connection) -> zbus::Result<()> {
    zbus::fdo::PeerProxy::builder(bus)
        .destination(PORTAL_NAME)
        .path(PORTAL_PATH)
        .build()
        .await?
        .ping()
        .await
}

async fn screencast(bus: &zbus::Result<zbus::Connection>, portal_answers: bool) -> Check {
    let name = "ScreenCast portal with virtual monitors";
    let bus = match bus {
        Ok(bus) if portal_answers => bus,
        _ => return Check::fail(name, "Needs xdg-desktop-portal, see above."),
    };
    match source_types(bus).await {
        Ok(types) if types & SOURCE_TYPE_VIRTUAL != 0 => Check::pass(name),
        Ok(types) => Check::fail(
            name,
            format!(
                "The ScreenCast portal of this desktop cannot create virtual \
                 monitors (its source types are {types}, without the 4 for \
                 virtual). GNOME can, and KDE Plasma from 6.5 on."
            ),
        ),
        Err(err) => {
            log::warn!("{SCREENCAST_INTERFACE} does not answer: {err}");
            Check::fail(
                name,
                "The portal has no ScreenCast interface. Install the portal \
                 backend for your desktop (xdg-desktop-portal-gnome or \
                 xdg-desktop-portal-kde).",
            )
        }
    }
}

async fn source_types(bus: &zbus::Connection) -> zbus::Result<u32> {
    portal_property(bus, SCREENCAST_INTERFACE, "AvailableSourceTypes").await
}

/// Reads one property of one of the portal's interfaces, afresh.
async fn portal_property(
    bus: &zbus::Connection,
    interface: &'static str,
    property: &str,
) -> zbus::Result<u32> {
    let proxy: zbus::Proxy = zbus::proxy::Builder::new(bus)
        .destination(PORTAL_NAME)
        .path(PORTAL_PATH)
        .interface(interface)
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    proxy.get_property(property).await
}

const REMOTE_DESKTOP_INTERFACE: &str = "org.freedesktop.portal.RemoteDesktop";
/// The bit for the pointer in the portal's `AvailableDeviceTypes`.
const DEVICE_TYPE_POINTER: u32 = 2;

/// Remote control is what the teleporter needs, and nothing else does: a
/// click on the picture then places the pointer on the virtual monitor. The
/// app gives a monitor without it, so this warns and never fails. It says
/// what `Session::start` will find; whether the user allows the pointer is
/// asked in the desktop's dialog at every Start and cannot be known here.
async fn remote_control(bus: &zbus::Result<zbus::Connection>, portal_answers: bool) -> Check {
    let name = "Remote control, for the teleporter";
    let without = |why: &str| {
        Check::warn(
            name,
            format!(
                "No teleporter: {why} The monitor works without it; only a click on the \
                 picture will not place the pointer there."
            ),
        )
    };
    let bus = match bus {
        Ok(bus) if portal_answers => bus,
        _ => return without("it needs xdg-desktop-portal, see above."),
    };
    match desktop::current().0 {
        desktop if desktop.has_teleporter() => {}
        Desktop::Kde => return without("on KDE Plasma it is not used yet."),
        _ => return without("it is used on GNOME only so far."),
    }
    match device_types(bus).await {
        Ok(types) if types & DEVICE_TYPE_POINTER != 0 => Check::pass(name),
        Ok(types) => without(&format!(
            "the RemoteDesktop portal of this desktop offers no pointer (its device types \
             are {types}, without the 2 for the pointer)."
        )),
        Err(err) => {
            log::debug!("{REMOTE_DESKTOP_INTERFACE} does not answer: {err}");
            without("the portal has no RemoteDesktop interface.")
        }
    }
}

async fn device_types(bus: &zbus::Connection) -> zbus::Result<u32> {
    portal_property(bus, REMOTE_DESKTOP_INTERFACE, "AvailableDeviceTypes").await
}

fn gst_element(factory: &str, hint: &str) -> Check {
    let name = format!("GStreamer element {factory}");
    if let Err(err) = gst::init() {
        return Check::fail(name, format!("GStreamer cannot be initialised: {err}."));
    }
    match gst::ElementFactory::make(factory).build() {
        Ok(_) => Check::pass(name),
        Err(_) => Check::fail(name, hint),
    }
}

/// Has GTK set a renderer up the way it will for the real window and looks
/// at which one came out. GTK moves on to the next renderer by itself when
/// the one it was asked for cannot be initialised, so a fallback is a warning.
fn gtk_renderer(asked: Option<Renderer>) -> Check {
    let name = "GTK 4 renderer";
    let probe = gtk::Window::new();
    WidgetExt::realize(&probe);
    let got = probe.renderer().map(|renderer| renderer.type_().name());
    probe.destroy();

    let Some(got) = got else {
        return Check::fail(name, "GTK could not initialise any renderer.");
    };
    let got_label = Renderer::from_type_name(got).map_or(got, Renderer::label);
    match asked {
        Some(asked) if Renderer::from_type_name(got) != Some(asked) => Check::warn(
            format!("{name}: {got_label}"),
            format!(
                "{} could not be initialised; GTK fell back to {got_label}.",
                asked.label()
            ),
        ),
        _ => Check::pass(format!("{name}: {got_label}")),
    }
}
