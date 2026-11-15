//! The shell: one GTK 4 window, opened once the startup checks have passed.
//! While nothing runs it shows the resolution selector, and can be resized
//! to choose a size; while a virtual monitor runs, its picture, with a Stop
//! button in the title bar. The title bar also opens the options.

mod checks;
mod checks_dialog;
mod desktop;
mod display;
mod environment;
mod input;
mod mirror;
mod options;
mod pictograms;
mod plasma;
mod pointer;
mod renderer;
mod screenshot;
mod selector;
mod session;
mod settings;
mod style;

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::rc::Rc;
use std::time::Instant;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use desktop::Desktop;
use display::Display;
use environment::Environment;
use input::Input;
use mirror::Mirror;
use screenshot::Shot;
use selector::Selector;
use session::Session;
use settings::{MIN_SIDE, Settings};

/// The names of the things the window can show.
const SELECTOR: &str = "selector";
const PICTURE: &str = "picture";
const AWAY: &str = "away";

/// What the window says, in place of the picture, while it is on its own
/// virtual monitor.
const AWAY_MESSAGE: &str = "Move this window back to a physical screen.";
const AWAY_REASON: &str =
    "It is on its own virtual monitor. The picture is paused until it is back.";

/// A started virtual monitor: the portal session that keeps it alive and the
/// pipeline that shows it.
struct Running {
    session: Session,
    display: Display,
    /// The teleporter at work on the picture, where the session came with a
    /// pointer.
    input: Option<Input>,
}

struct Shell {
    app: glib::WeakRef<gtk::Application>,
    window: gtk::ApplicationWindow,
    /// All of the window under its title bar.
    stage: gtk::Overlay,
    /// Draws nothing. It is the size the picture is to be shown at, which is
    /// what makes the stage, and with it the window, ask for that size.
    sizer: gtk::Picture,
    /// The selector, the picture or the message, laid over the sizer.
    pages: gtk::Stack,
    selector: Rc<Selector>,
    picture: gtk::Picture,
    stop: gtk::Button,
    /// Saves what the virtual monitor shows as a picture. In the title bar
    /// next to Stop, and like it only while a monitor runs.
    screenshot: Rc<screenshot::Button>,
    settings: RefCell<Settings>,
    /// Which settings the environment stands in for, for the options to say.
    overridden: options::Overridden,
    /// Set to have `main` start the app again once it has shut down.
    restart: Rc<Cell<bool>>,
    /// The size the stage is meant to have: the one the window was last asked
    /// for, or the one the user has resized it to since.
    stage_size: Cell<(i32, i32)>,
    /// Whether a monitor is being started or is running. Until then the
    /// window can be resized and the selector follows it; from then on the
    /// lock setting decides.
    engaged: Cell<bool>,
    /// Tells when the window is on its own virtual monitor.
    mirror: Mirror,
    /// The monitor the selector was last told about, and the watch on its
    /// size and scale.
    screen: RefCell<Option<(gdk::Monitor, [glib::SignalHandlerId; 2])>>,
    /// `None` while idle, and also while a start or a stop is under way; the
    /// selector is busy for the length of those.
    running: RefCell<Option<Running>>,
}

impl Shell {
    fn start(self: &Rc<Self>) {
        let size = self.selector.size();
        self.selector.set_busy(true);
        self.remember(size);
        self.hold_window();
        // Any monitor that turns up from here on is taken to be the virtual
        // one.
        self.mirror.mark(&WidgetExt::display(&self.window));
        let shell = self.clone();
        glib::spawn_future_local(async move {
            match shell.open(size).await {
                Ok(running) => {
                    shell
                        .picture
                        .set_paintable(Some(running.display.paintable()));
                    shell.await_frame(running.display.paintable());
                    shell.running.replace(Some(running));
                    shell.show_page();
                    shell.selector.set_busy(false);
                    shell.stop.set_visible(true);
                    shell.screenshot.widget().set_visible(true);
                    shell.stop.grab_focus();
                }
                Err(err) => {
                    log::error!("could not start: {err}");
                    shell.release_window();
                    shell.selector.set_busy(false);
                    // Being busy cost the button the focus.
                    shell.selector.focus_start();
                }
            }
        });
    }

    async fn open(self: &Rc<Self>, chosen: (i32, i32)) -> Result<Running, Box<dyn Error>> {
        let ask_remote_control = self.settings.borrow().ask_remote_control;
        let (session, stream, pointer) = Session::start(ask_remote_control).await?;
        // Where the desktop has made the monitor already, at a size of its
        // own, that size is the one to show; the stream is asked for none.
        // Everywhere else the size asked of the stream is the chosen one.
        let size = match stream
            .size
            .filter(|&(width, height)| width > 0 && height > 0)
        {
            Some(made) => {
                self.take_size(made, chosen);
                None
            }
            None => Some(chosen),
        };

        // The pipeline calls this from its own bus handler; stop from a fresh
        // main loop turn rather than tearing the pipeline down under it.
        let shell = Rc::downgrade(self);
        let on_end = move || {
            glib::idle_add_local_once(move || {
                if let Some(shell) = shell.upgrade() {
                    shell.stop();
                }
            });
        };

        match Display::new(stream.fd, stream.node_id, size, on_end) {
            Ok(display) => {
                let input = pointer.map(|pointer| Input::attach(&self.picture, pointer));
                Ok(Running {
                    session,
                    display,
                    input,
                })
            }
            Err(err) => {
                // Without a picture the monitor would be there but unseen.
                if let Err(err) = session.close().await {
                    log::warn!("could not close the session: {err}");
                }
                Err(err.into())
            }
        }
    }

    /// Stops the pipeline and closes the portal session, which makes the
    /// desktop remove the monitor, and brings the selector back. Does nothing
    /// unless a monitor is running.
    fn stop(self: &Rc<Self>) {
        let Some(Running {
            session,
            display,
            mut input,
        }) = self.running.take()
        else {
            return;
        };
        if let Some(input) = &mut input {
            input.detach();
        }
        self.stop.set_visible(false);
        self.screenshot.widget().set_visible(false);
        self.screenshot.reset();
        self.selector.set_busy(true);
        self.picture.set_paintable(gdk::Paintable::NONE);
        drop(display);
        self.show_page();
        self.release_window();

        // Closing the window lands here too; keep the application alive
        // until the portal has been told.
        let hold = self.app.upgrade().map(|app| app.hold());
        let shell = self.clone();
        glib::spawn_future_local(async move {
            if let Err(err) = session.close().await {
                log::warn!("could not close the session: {err}");
            }
            // Its connection to the desktop goes after the session.
            drop(input);
            shell.selector.set_busy(false);
            shell.selector.focus_start();
            drop(hold);
        });
    }

    /// There is nothing to take a screenshot of until the first frame has
    /// arrived, which is when the picture gets its size. The button cannot
    /// be pressed until then.
    fn await_frame(self: &Rc<Self>, paintable: &gdk::Paintable) {
        let button = self.screenshot.widget();
        button.set_sensitive(paintable.intrinsic_width() > 0);
        // The handler goes with the paintable, which is the pipeline's.
        let button = button.downgrade();
        paintable.connect_invalidate_size(move |paintable| {
            if let Some(button) = button.upgrade()
                && paintable.intrinsic_width() > 0
            {
                button.set_sensitive(true);
            }
        });
    }

    /// Saves the frame the virtual monitor shows right now as a picture, and
    /// has the button say how that went. The frame is the stream's, so it is
    /// the monitor at its own size whatever the window shows of it.
    fn shoot(self: &Rc<Self>) {
        let since = Instant::now();
        let frame = {
            let running = self.running.borrow();
            let Some(running) = running.as_ref() else {
                return;
            };
            self.window
                .renderer()
                .and_then(|renderer| running.display.frame(&renderer))
        };
        let Some(frame) = frame else {
            log::warn!("no screenshot: there is no frame to save yet");
            self.screenshot.show(false);
            return;
        };
        let (width, height) = (frame.width(), frame.height());
        let drawn = since.elapsed();
        let shot = Shot::new(frame);
        let shell = self.clone();
        glib::spawn_future_local(async move {
            // Off the main thread: a large frame takes a moment to pack,
            // which the picture is not to wait for.
            let saved = match gio::spawn_blocking(move || shot.save()).await {
                Ok(Ok(path)) => {
                    log::info!("screenshot saved: {}", path.display());
                    true
                }
                Ok(Err(err)) => {
                    log::warn!("the screenshot could not be saved: {err}");
                    false
                }
                Err(_) => {
                    log::warn!("the screenshot could not be saved: writing it was given up");
                    false
                }
            };
            log::debug!(
                "screenshot: a frame of {width}×{height}, drawn in {:.1} ms on the main thread, \
                 done {:.0} ms after the click",
                drawn.as_secs_f64() * 1000.0,
                since.elapsed().as_secs_f64() * 1000.0
            );
            shell.screenshot.show(saved);
        });
    }

    fn show_options(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let settings = self.settings.borrow().clone();
        let on_plasma = desktop::current().0 == Desktop::Kde;
        options::show(
            &self.window,
            &settings,
            self.overridden,
            on_plasma,
            move |settings| {
                let Some(shell) = weak.upgrade() else {
                    return Ok(());
                };
                settings.save()?;
                // Closing the window stops what is running and ends the
                // application; `main` takes it from there.
                shell.restart.set(true);
                shell.window.close();
                Ok(())
            },
        );
    }

    /// The desktop has made the monitor `made` large, whatever was chosen.
    /// The selector, the settings and the window follow.
    fn take_size(&self, made: (i32, i32), chosen: (i32, i32)) {
        if made == chosen {
            return;
        }
        log::info!(
            "the desktop made the monitor {}×{}, not the {}×{} chosen; the window follows",
            made.0,
            made.1,
            chosen.0,
            chosen.1
        );
        self.selector.set_size(made);
        self.remember(made);
        self.give_stage(self.selector.picture_area());
        // Again, for the lock to hold the window at the new size.
        self.hold_window();
    }

    /// Keeps the size for the next time the app is started.
    fn remember(&self, (width, height): (i32, i32)) {
        let mut settings = self.settings.borrow_mut();
        if (settings.width, settings.height) == (width, height) {
            return;
        }
        settings.width = width;
        settings.height = height;
        if let Err(err) = settings.save() {
            log::warn!("could not save the settings: {err}");
        }
    }

    /// Shows what there is to show: the selector, the picture, or, while the
    /// window is on its own virtual monitor, the request to move it away.
    /// The picture is then not drawn, so the stream no longer shows itself.
    fn show_page(&self) {
        let page = if self.running.borrow().is_none() {
            SELECTOR
        } else if self.mirror.virtual_monitor().is_some() {
            AWAY
        } else {
            PICTURE
        };
        let shown = self.pages.visible_child_name();
        if shown.as_deref() == Some(page) {
            return;
        }
        if page == AWAY {
            log::info!(
                "the window is on the virtual monitor; not drawing the stream \
                 until it is back on a physical screen"
            );
        } else if shown.as_deref() == Some(AWAY) && page == PICTURE {
            log::info!("the window is back on a physical screen; drawing the stream again");
        }
        self.pages.set_visible_child_name(page);
    }

    /// Asks the window to give the stage this size.
    fn give_stage(&self, size: (i32, i32)) {
        self.stage_size.set(size);
        let (width, height) = size;
        self.sizer
            .set_paintable(Some(&gdk::Paintable::new_empty(width, height)));
        // Has the window forget the size it has, so that it takes the one its
        // content would like, which is the sizer's.
        self.window.set_default_size(-1, -1);
    }

    /// A size has been chosen in the selector, or the room for it has
    /// changed: the stage takes the size the picture can be shown at. Not
    /// once a monitor is being started; that keeps the size it started with.
    fn follow_selector(&self) {
        if !self.engaged.get() {
            self.give_stage(self.selector.picture_area());
        }
    }

    /// The stage has been given a size. Until Start, one that the window was
    /// not asked for is the user resizing the window, and becomes the chosen
    /// size.
    fn stage_resized(&self, size: (i32, i32)) {
        if self.engaged.get() || size == self.stage_size.get() {
            return;
        }
        self.stage_size.set(size);
        self.selector.set_size(size);
    }

    /// From Start on the lock setting decides whether the window can be
    /// resized.
    fn hold_window(&self) {
        self.engaged.set(true);
        if self.settings.borrow().lock_window_size {
            // A window that cannot be resized is as large as its content
            // insists on being.
            let (width, height) = self.stage_size.get();
            self.stage.set_size_request(width, height);
            self.window.set_resizable(false);
        } else {
            // As small as the user likes; the picture scales.
            self.stage.set_size_request(-1, -1);
        }
    }

    /// Until Start the window can be resized, whatever the lock setting, and
    /// the selector follows it. The stage goes back to the size it had at
    /// Start, which it may have left if the lock is off.
    fn release_window(&self) {
        self.engaged.set(false);
        self.window.set_resizable(true);
        // No smaller than the smallest size there is to choose.
        self.stage.set_size_request(MIN_SIDE, MIN_SIDE);
        self.give_stage(self.stage_size.get());
    }

    /// Tells the selector which monitor the window is on, or, as long as it
    /// is on none, the first there is. From then on the selector also hears
    /// of it when that monitor's size or scale changes.
    fn find_screen(self: &Rc<Self>) {
        let display = WidgetExt::display(&self.window);
        let Some(monitor) = self
            .window
            .surface()
            .and_then(|surface| display.monitor_at_surface(&surface))
            .or_else(|| display.monitors().item(0).and_downcast::<gdk::Monitor>())
        else {
            return;
        };

        let mut screen = self.screen.borrow_mut();
        if screen
            .as_ref()
            .is_none_or(|(watched, _)| *watched != monitor)
        {
            if let Some((watched, handlers)) = screen.take() {
                for handler in handlers {
                    watched.disconnect(handler);
                }
            }
            let changed = || {
                let weak = Rc::downgrade(self);
                move |_: &gdk::Monitor| {
                    if let Some(shell) = weak.upgrade() {
                        shell.find_screen();
                    }
                }
            };
            let handlers = [
                monitor.connect_geometry_notify(changed()),
                monitor.connect_scale_notify(changed()),
            ];
            screen.replace((monitor.clone(), handlers));
        }
        drop(screen);

        let geometry = monitor.geometry();
        self.selector
            .set_screen((geometry.width(), geometry.height()), monitor.scale());
    }
}

fn build_window(
    app: &gtk::Application,
    settings: Settings,
    overridden: options::Overridden,
    restart: Rc<Cell<bool>>,
) {
    let selector = Selector::new(
        (settings.width, settings.height),
        desktop::current().0.decides_size(),
    );

    // Contain scales the picture to fit and keeps its aspect ratio; what is
    // left over of the window stays empty.
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Contain)
        .build();
    // Lets GTK hand DMA-BUF frames to the compositor as they are.
    let offload = gtk::GraphicsOffload::builder().child(&picture).build();

    let message = gtk::Box::new(gtk::Orientation::Vertical, 6);
    for (text, classes) in [(AWAY_MESSAGE, "mk-title-1"), (AWAY_REASON, "mk-dim")] {
        message.append(
            &gtk::Label::builder()
                .label(text)
                .wrap(true)
                .justify(gtk::Justification::Center)
                .css_classes([classes])
                .build(),
        );
    }
    let sign = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .valign(gtk::Align::Center)
        .vexpand(true)
        .margin_start(24)
        .margin_end(24)
        .build();
    sign.append(&pictograms::move_back());
    sign.append(&message);
    let away = gtk::Box::builder().css_classes(["mk-page"]).build();
    sign.set_hexpand(true);
    away.append(&sign);

    // Only as large as the page that is shown has to be. A stack is as large
    // as its largest page otherwise, and the picture, scaled down with the
    // lock off, could then be no lower than the selector at its smallest:
    // its foot was drawn past the window's.
    let pages = gtk::Stack::builder()
        .hhomogeneous(false)
        .vhomogeneous(false)
        .build();
    pages.add_named(selector.widget(), Some(SELECTOR));
    pages.add_named(&offload, Some(PICTURE));
    pages.add_named(&away, Some(AWAY));

    // The stage asks for the sizer's size and no other: what is laid over a
    // GtkOverlay's child does not count. The window has no size of its own,
    // so it opens just large enough for the stage.
    let sizer = gtk::Picture::new();
    let stage = gtk::Overlay::builder()
        .child(&sizer)
        .width_request(MIN_SIDE)
        .height_request(MIN_SIDE)
        .build();
    // Draws nothing either. It is there to be given the stage's size, which
    // a GtkDrawingArea tells of. Under the pages, so that it is not in the
    // pointer's way.
    let watcher = gtk::DrawingArea::new();
    stage.add_overlay(&watcher);
    stage.add_overlay(&pages);

    // A white square and the word, as on any player.
    let stop_face = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    stop_face.append(
        &gtk::Box::builder()
            .css_classes(["mk-stop-square"])
            .valign(gtk::Align::Center)
            .build(),
    );
    stop_face.append(&gtk::Label::new(Some("Stop")));
    let stop = gtk::Button::builder()
        .child(&stop_face)
        .css_classes(["mk-stop"])
        .visible(false)
        .build();
    let options = gtk::Button::with_mnemonic("_Options");
    let screenshot = screenshot::Button::new();
    let header = gtk::HeaderBar::new();
    header.pack_start(&stop);
    header.pack_start(screenshot.widget());
    header.pack_end(&options);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Meeseeks")
        .titlebar(&header)
        .child(&stage)
        .build();

    let shell = Rc::new(Shell {
        app: app.downgrade(),
        window: window.clone(),
        stage,
        sizer,
        pages,
        selector,
        picture,
        stop,
        screenshot,
        settings: RefCell::new(settings),
        overridden,
        restart,
        stage_size: Cell::new((0, 0)),
        engaged: Cell::new(false),
        mirror: Mirror::default(),
        screen: RefCell::new(None),
        running: RefCell::new(None),
    });

    let weak = Rc::downgrade(&shell);
    shell.selector.connect_changed(move || {
        if let Some(shell) = weak.upgrade() {
            shell.follow_selector();
        }
    });
    let weak = Rc::downgrade(&shell);
    watcher.connect_resize(move |_, width, height| {
        if let Some(shell) = weak.upgrade() {
            shell.stage_resized((width, height));
        }
    });
    let weak = Rc::downgrade(&shell);
    shell.selector.connect_start(move || {
        if let Some(shell) = weak.upgrade() {
            shell.start();
        }
    });
    let weak = Rc::downgrade(&shell);
    shell.stop.connect_clicked(move |_| {
        if let Some(shell) = weak.upgrade() {
            shell.stop();
        }
    });

    let weak = Rc::downgrade(&shell);
    shell.screenshot.widget().connect_clicked(move |_| {
        if let Some(shell) = weak.upgrade() {
            shell.shoot();
        }
    });

    let weak = Rc::downgrade(&shell);
    options.connect_clicked(move |_| {
        if let Some(shell) = weak.upgrade() {
            shell.show_options();
        }
    });

    // Which monitors the window is on is known once it has a surface, and
    // changes as it enters and leaves them. The selector wants to know for
    // the room there is, and the picture must not be drawn on its own
    // virtual monitor.
    let weak = Rc::downgrade(&shell);
    window.connect_realize(move |window| {
        let Some(surface) = window.surface() else {
            return;
        };
        for entered in [true, false] {
            let weak = weak.clone();
            let moved = move |_: &gdk::Surface, monitor: &gdk::Monitor| {
                if let Some(shell) = weak.upgrade() {
                    if entered {
                        shell.mirror.entered(monitor);
                    } else {
                        shell.mirror.left(monitor);
                    }
                    shell.find_screen();
                    shell.show_page();
                }
            };
            if entered {
                surface.connect_enter_monitor(moved);
            } else {
                surface.connect_leave_monitor(moved);
            }
        }
    });
    shell.find_screen();
    shell.follow_selector();

    let closing = shell.clone();
    window.connect_close_request(move |_| {
        closing.stop();
        glib::Propagation::Proceed
    });

    window.present();
    shell.selector.focus_start();

    // Which renderer GTK chose decides how the frames get drawn, so say it.
    if let Some(renderer) = window.renderer() {
        log::info!("GTK draws with {}", renderer.type_().name());
    }
}

/// Runs the startup checks and opens the main window, or, if one of them
/// fails, the dialog that shows them all.
fn activate(
    app: &gtk::Application,
    settings: Settings,
    environment: &Environment,
    restart: Rc<Cell<bool>>,
) {
    style::load();
    let asked = environment.asked;
    let overridden = options::Overridden {
        renderer: environment.renderer_overridden,
        tries: environment.tries_overridden,
    };
    // There is no window yet to keep the application alive.
    let hold = app.hold();
    let app = app.clone();
    glib::spawn_future_local(async move {
        let checks = checks::run(asked, settings.ask_remote_control).await;
        if checks::failed(&checks) {
            eprint!("{}", checks::report(&checks));
            checks_dialog::show(&app, &checks);
        } else {
            for check in &checks {
                match (check.status, &check.hint) {
                    (checks::Status::Warn, Some(hint)) => log::warn!("{}: {hint}", check.name),
                    _ => log::debug!("check passed: {}", check.name),
                }
            }
            build_window(&app, settings, overridden, restart);
        }
        drop(hold);
    });
}

/// The application id: what the window says it belongs to, which is how the
/// desktop finds `meeseeks.desktop` for it, and with that the icon in the
/// dock. A plain name rather than a reverse-DNS one, because that is the name
/// Rick's tarball installer gives the desktop entry.
const APP_ID: &str = "meeseeks";

fn main() -> glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    // GTK names the window's application after the program, which would
    // otherwise be whatever the binary's file is called.
    glib::set_prgname(Some(APP_ID));
    // Before the startup checks and long before the pipeline: the sink is
    // part of this binary and has to be made known to GStreamer.
    if let Err(err) = display::init() {
        log::error!("GStreamer could not be made ready: {err}");
    }
    let settings = Settings::load();
    // SAFETY: nothing so far has started a thread, and GTK, which reads what
    // this sets, is not initialised yet.
    let environment = Rc::new(unsafe { environment::apply(&settings) });
    let restart = Rc::new(Cell::new(false));

    let app = gtk::Application::builder().build();
    app.connect_activate({
        let environment = environment.clone();
        let restart = restart.clone();
        move |app| activate(app, settings.clone(), &environment, restart.clone())
    });
    let status = app.run();

    if restart.get() {
        let err = environment.relaunch();
        log::error!("could not start the app again: {err}");
        return glib::ExitCode::FAILURE;
    }
    status
}
