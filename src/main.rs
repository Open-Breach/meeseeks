//! The shell: one GTK 4 window with a Start/Stop button and the picture.

mod display;
mod session;

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use display::Display;
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
        self.picture.set_paintable(gdk::Paintable::NONE);
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
    // left over of the window stays empty.
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Contain)
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
        .default_width(960)
        .default_height(600)
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
}

fn main() -> glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let app = gtk::Application::builder().build();
    app.connect_activate(build_window);
    app.run()
}
