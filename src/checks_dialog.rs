//! The dialog shown in place of the main window when a startup check fails.

use gtk::prelude::*;

use crate::checks::{self, Check, Status};

/// How wide the hints run before they wrap, in characters.
const HINT_WIDTH: i32 = 64;

/// Shows the results, one line per check. Closing the dialog ends the
/// application, since it is its only window.
pub fn show(app: &gtk::Application, checks: &[Check]) {
    let heading = gtk::Label::builder()
        .label("This system lacks something Meeseeks needs.")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();

    let list = gtk::Grid::builder()
        .column_spacing(10)
        .row_spacing(2)
        .build();
    let mut row = 0;
    for check in checks {
        let (mark, class) = match check.status {
            Status::Pass => ("✓", "success"),
            Status::Warn => ("!", "warning"),
            Status::Fail => ("✗", "error"),
        };
        let mark = gtk::Label::builder()
            .label(mark)
            .css_classes([class])
            .margin_top(6)
            .build();
        let name = gtk::Label::builder()
            .label(&check.name)
            .xalign(0.0)
            .hexpand(true)
            .margin_top(6)
            .build();
        list.attach(&mark, 0, row, 1, 1);
        list.attach(&name, 1, row, 1, 1);
        row += 1;
        if let Some(hint) = &check.hint {
            // A wrapping label has to be told its width, or the window
            // comes out as tall as the text would be in a narrow column.
            let hint = gtk::Label::builder()
                .label(hint)
                .xalign(0.0)
                .wrap(true)
                .width_chars(HINT_WIDTH)
                .max_width_chars(HINT_WIDTH)
                .selectable(true)
                .css_classes(["dim-label"])
                .build();
            list.attach(&hint, 1, row, 1, 1);
            row += 1;
        }
    }

    let copy = gtk::Button::with_label("Copy report");
    let close = gtk::Button::with_label("Close");
    let buttons = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::End)
        .build();
    buttons.append(&copy);
    buttons.append(&close);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&heading);
    content.append(&list);
    content.append(&buttons);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Meeseeks cannot start")
        .resizable(false)
        .child(&content)
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
