//! The shell: one GTK 4 window, opened once the startup checks have passed.
//! While nothing runs it shows the resolution selector, and can be resized
//! to choose a size; while a virtual monitor runs, its picture, with a Stop
//! button in the title bar. The title bar also opens the options.

mod checks;
mod checks_dialog;
mod display;
mod environment;
mod mirror;
mod options;
mod pictograms;
mod renderer;
mod selector;
mod session;
mod settings;
mod style;

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use display::Display;
use environment::Environment;
use mirror::Mirror;
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

/// A started virtual monitor: the portal session that keeps it alive and the
/// pipeline that shows it.
struct Running {
    session: Session,
    display: Display,
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
                    shell.running.replace(Some(running));
                    shell.show_page();
                    shell.selector.set_busy(false);
                    shell.stop.set_visible(true);
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

    async fn open(self: &Rc<Self>, size: (i32, i32)) -> Result<Running, Box<dyn Error>> {
        let (session, stream) = Session::start().await?;

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
            Ok(display) => Ok(Running { session, display }),
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
        let Some(Running { session, display }) = self.running.take() else {
            return;
        };
        self.stop.set_visible(false);
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
            shell.selector.set_busy(false);
            shell.selector.focus_start();
            drop(hold);
        });
    }

    fn show_options(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let settings = self.settings.borrow().clone();
        options::show(&self.window, &settings, self.overridden, move |settings| {
            let Some(shell) = weak.upgrade() else {
                return Ok(());
            };
            settings.save()?;
            // Closing the window stops what is running and ends the
            // application; `main` takes it from there.
            shell.restart.set(true);
            shell.window.close();
            Ok(())
        });
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
    selector::load_css();
    let selector = Selector::new((settings.width, settings.height));

    // Contain scales the picture to fit and keeps its aspect ratio; what is
    // left over of the window stays empty.
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Contain)
        .build();
    // Lets GTK hand DMA-BUF frames to the compositor as they are.
    let offload = gtk::GraphicsOffload::builder().child(&picture).build();

    let away = gtk::Label::builder()
        .label(AWAY_MESSAGE)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["title-2"])
        .build();

    let pages = gtk::Stack::new();
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

    let stop = gtk::Button::builder()
        .label("Stop")
        .css_classes(["destructive-action"])
        .visible(false)
        .build();
    let options = gtk::Button::with_mnemonic("_Options");
    let header = gtk::HeaderBar::new();
    header.pack_start(&stop);
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
        let checks = checks::run(asked).await;
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

fn main() -> glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
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
