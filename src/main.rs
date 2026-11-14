//! The shell: one GTK 4 window, opened once the startup checks have passed.
//! While nothing runs it shows the resolution selector; while a virtual
//! monitor runs, its picture, with a Stop button in the title bar.

mod checks;
mod checks_dialog;
mod display;
mod renderer;
mod selector;
mod session;
mod settings;

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use display::Display;
use renderer::Renderer;
use selector::Selector;
use session::Session;
use settings::Settings;

/// The names of the two things the window can show.
const SELECTOR: &str = "selector";
const PICTURE: &str = "picture";

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
    /// The selector or the picture, laid over the sizer.
    pages: gtk::Stack,
    selector: Rc<Selector>,
    picture: gtk::Picture,
    stop: gtk::Button,
    settings: RefCell<Settings>,
    /// The size the stage was last given.
    stage_size: Cell<(i32, i32)>,
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
        let shell = self.clone();
        glib::spawn_future_local(async move {
            match shell.open(size).await {
                Ok(running) => {
                    shell
                        .picture
                        .set_paintable(Some(running.display.paintable()));
                    shell.running.replace(Some(running));
                    shell.pages.set_visible_child_name(PICTURE);
                    shell.selector.set_busy(false);
                    shell.stop.set_visible(true);
                    shell.stop.grab_focus();
                }
                Err(err) => {
                    log::error!("could not start: {err}");
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
        self.pages.set_visible_child_name(SELECTOR);
        self.picture.set_paintable(gdk::Paintable::NONE);
        drop(display);
        // The window may have gone to another monitor while it was running.
        self.size_stage();

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

    /// Gives the stage the size the picture is to be shown at, and the window
    /// with it.
    fn size_stage(&self) {
        let size = self.selector.picture_area();
        if self.stage_size.replace(size) == size {
            return;
        }
        let (width, height) = size;
        self.sizer
            .set_paintable(Some(&gdk::Paintable::new_empty(width, height)));
        if self.settings.borrow().lock_window_size {
            // A window that cannot be resized is as large as its content
            // insists on being.
            self.stage.set_size_request(width, height);
        } else {
            // Has the window forget the size it has, so that it takes the one
            // its content would like.
            self.window.set_default_size(-1, -1);
        }
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

fn build_window(app: &gtk::Application, settings: Settings) {
    selector::load_css();
    let selector = Selector::new((settings.width, settings.height));

    // Contain scales the picture to fit and keeps its aspect ratio; what is
    // left over of the window stays empty.
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Contain)
        .build();
    // Lets GTK hand DMA-BUF frames to the compositor as they are.
    let offload = gtk::GraphicsOffload::builder().child(&picture).build();

    let pages = gtk::Stack::new();
    pages.add_named(selector.widget(), Some(SELECTOR));
    pages.add_named(&offload, Some(PICTURE));

    // The stage asks for the sizer's size and no other: what is laid over a
    // GtkOverlay's child does not count. The window has no size of its own,
    // so it opens just large enough for the stage.
    let sizer = gtk::Picture::new();
    let stage = gtk::Overlay::builder().child(&sizer).build();
    stage.add_overlay(&pages);

    let stop = gtk::Button::builder()
        .label("Stop")
        .css_classes(["destructive-action"])
        .visible(false)
        .build();
    let header = gtk::HeaderBar::new();
    header.pack_start(&stop);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Meeseeks")
        .titlebar(&header)
        .child(&stage)
        .resizable(!settings.lock_window_size)
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
        stage_size: Cell::new((0, 0)),
        screen: RefCell::new(None),
        running: RefCell::new(None),
    });

    let weak = Rc::downgrade(&shell);
    shell.selector.connect_changed(move || {
        // A running monitor keeps the size it was started with.
        if let Some(shell) = weak.upgrade()
            && shell.running.borrow().is_none()
        {
            shell.size_stage();
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

    // Which monitor the window is on is known once it has a surface, and
    // changes as it enters and leaves monitors.
    let weak = Rc::downgrade(&shell);
    window.connect_realize(move |window| {
        let Some(surface) = window.surface() else {
            return;
        };
        for entered in [true, false] {
            let weak = weak.clone();
            let moved = move |_: &gdk::Surface, _: &gdk::Monitor| {
                if let Some(shell) = weak.upgrade() {
                    shell.find_screen();
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
    shell.size_stage();

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
/// fails, the dialog that shows them all. `asked` is the renderer GTK has been
/// asked for.
fn activate(app: &gtk::Application, asked: Option<Renderer>, settings: Settings) {
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
            build_window(&app, settings);
        }
        drop(hold);
    });
}

fn main() -> glib::ExitCode {
    // SAFETY: this runs first thing in `main`, before any other thread exists
    // that could be reading the environment.
    let asked = unsafe { renderer::request(Renderer::default()) };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let settings = Settings::load();

    let app = gtk::Application::builder().build();
    app.connect_activate(move |app| activate(app, asked, settings.clone()));
    app.run()
}
