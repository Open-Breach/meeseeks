//! The shell: one GTK 4 window with a Start/Stop button and the picture,
//! opened once the startup checks have passed.

mod checks;
mod checks_dialog;
mod display;
mod renderer;
mod session;

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use display::Display;
use renderer::Renderer;
use session::Session;

/// A started virtual monitor: the portal session that keeps it alive and the
/// pipeline that shows it.
struct Running {
    session: Session,
    display: Display,
}

struct Shell {
    app: glib::WeakRef<gtk::Application>,
    button: gtk::Button,
    picture: gtk::Picture,
    /// `None` while idle, and also while a start or a stop is under way; the
    /// button is insensitive for the length of those.
    running: RefCell<Option<Running>>,
}

impl Shell {
    fn toggle(self: &Rc<Self>) {
        if self.running.borrow().is_some() {
            self.stop();
        } else {
            self.start();
        }
    }

    fn start(self: &Rc<Self>) {
        self.button.set_sensitive(false);
        let shell = self.clone();
        glib::spawn_future_local(async move {
            match shell.open().await {
                Ok(running) => {
                    shell
                        .picture
                        .set_paintable(Some(running.display.paintable()));
                    shell.running.replace(Some(running));
                    shell.button.set_label("Stop");
                }
                Err(err) => log::error!("could not start: {err}"),
            }
            shell.button.set_sensitive(true);
        });
    }

    async fn open(self: &Rc<Self>) -> Result<Running, Box<dyn Error>> {
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

        match Display::new(stream.fd, stream.node_id, on_end) {
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
    /// desktop remove the monitor. Does nothing unless a monitor is running.
    fn stop(self: &Rc<Self>) {
        let Some(Running { session, display }) = self.running.take() else {
            return;
        };
        self.button.set_sensitive(false);
        self.picture.set_paintable(Some(&display::placeholder()));
        drop(display);

        // Closing the window lands here too; keep the application alive
        // until the portal has been told.
        let hold = self.app.upgrade().map(|app| app.hold());
        let shell = self.clone();
        glib::spawn_future_local(async move {
            if let Err(err) = session.close().await {
                log::warn!("could not close the session: {err}");
            }
            shell.button.set_label("Start");
            shell.button.set_sensitive(true);
            drop(hold);
        });
    }
}

fn build_window(app: &gtk::Application) {
    let button = gtk::Button::builder()
        .label("Start")
        .halign(gtk::Align::Center)
        .margin_top(6)
        .margin_bottom(6)
        .build();

    // Contain scales the picture to fit and keeps its aspect ratio; what is
    // left over of the window stays empty. The placeholder gives the picture
    // the monitor's size before there is a stream, and the window, which has
    // no size of its own, opens just large enough to show it unscaled.
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Contain)
        .paintable(&display::placeholder())
        .build();
    // Lets GTK hand DMA-BUF frames to the compositor as they are.
    let offload = gtk::GraphicsOffload::builder()
        .child(&picture)
        .hexpand(true)
        .vexpand(true)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&button);
    content.append(&offload);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Meeseeks")
        .child(&content)
        .build();

    let shell = Rc::new(Shell {
        app: app.downgrade(),
        button: button.clone(),
        picture,
        running: RefCell::new(None),
    });

    let weak = Rc::downgrade(&shell);
    button.connect_clicked(move |_| {
        if let Some(shell) = weak.upgrade() {
            shell.toggle();
        }
    });
    window.connect_close_request(move |_| {
        shell.stop();
        glib::Propagation::Proceed
    });

    window.present();

    // Which renderer GTK chose decides how the frames get drawn, so say it.
    if let Some(renderer) = window.renderer() {
        log::info!("GTK draws with {}", renderer.type_().name());
    }
}

/// Runs the startup checks and opens the main window, or, if one of them
/// fails, the dialog that shows them all. `asked` is the renderer GTK has been
/// asked for.
fn activate(app: &gtk::Application, asked: Option<Renderer>) {
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
            build_window(&app);
        }
        drop(hold);
    });
}

fn main() -> glib::ExitCode {
    // SAFETY: this runs first thing in `main`, before any other thread exists
    // that could be reading the environment.
    let asked = unsafe { renderer::request(Renderer::default()) };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let app = gtk::Application::builder().build();
    app.connect_activate(move |app| activate(app, asked));
    app.run()
}
