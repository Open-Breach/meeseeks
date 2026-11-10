//! The shell: one GTK 4 window with a Start/Stop button and the picture.

use gtk::glib;
use gtk::prelude::*;

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

    window.present();
}

fn main() -> glib::ExitCode {
    let app = gtk::Application::builder().build();
    app.connect_activate(build_window);
    app.run()
}
