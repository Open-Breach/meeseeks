//! The dialog shown in place of the main window when a startup check fails.

use gtk::prelude::*;

use crate::checks::{self, Check, Status};
use crate::pictograms;

/// How wide the hints run before they wrap, in characters.
const HINT_WIDTH: i32 = 68;

/// Shows the results, one row per check. Closing the dialog ends the
/// application, since it is its only window.
pub fn show(app: &gtk::Application, checks: &[Check]) {
    let heading = gtk::Label::builder()
        .label("This system lacks something Meeseeks needs.")
        .xalign(0.0)
        .wrap(true)
        .css_classes(["mk-title-2"])
        .build();

    // Hidden overflow has the rows follow the list's rounded corners.
    let list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["mk-list"])
        .overflow(gtk::Overflow::Hidden)
        .build();
    for check in checks {
        let mark = match check.status {
            Status::Pass => pictograms::pass(),
            Status::Warn => pictograms::warning(),
            Status::Fail => pictograms::fail(),
        };
        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .build();
        text.append(
            &gtk::Label::builder()
                .label(&check.name)
                .xalign(0.0)
                .wrap(true)
                .build(),
        );
        if let Some(hint) = &check.hint {
            // A wrapping label has to be told its width, or the window
            // comes out as tall as the text would be in a narrow column.
            text.append(
                &gtk::Label::builder()
                    .label(hint)
                    .xalign(0.0)
                    .wrap(true)
                    .width_chars(HINT_WIDTH)
                    .max_width_chars(HINT_WIDTH)
                    .selectable(true)
                    .css_classes(["mk-dim", "mk-caption"])
                    .build(),
            );
        }
        let row = gtk::Box::builder()
            .spacing(12)
            .css_classes(["mk-row"])
            .build();
        // The check that stops the app is found without reading every row.
        if check.status == Status::Fail {
            row.add_css_class("mk-failed");
        }
        row.append(&mark);
        row.append(&text);
        list.append(&row);
    }

    let copy = gtk::Button::builder()
        .label("Copy report")
        .css_classes(["mk-button"])
        .build();
    let close = gtk::Button::builder()
        .label("Close")
        .css_classes(["mk-button", "mk-suggested"])
        .build();
    let buttons = gtk::Box::builder()
        .spacing(8)
        .halign(gtk::Align::End)
        .build();
    buttons.append(&copy);
    buttons.append(&close);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    content.append(&heading);
    content.append(&list);
    content.append(&buttons);
    // The margins belong to the content; the colour has to reach the edges.
    let ground = gtk::Box::builder().css_classes(["mk-dialog"]).build();
    ground.append(&content);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Meeseeks cannot start")
        .resizable(false)
        .child(&ground)
        .build();

    let report = checks::report(checks);
    copy.connect_clicked(move |copy| copy.clipboard().set_text(&report));
    let weak = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade() {
            window.close();
        }
    });
    GtkWindowExt::set_focus(&window, Some(&close));
    window.present();
}
