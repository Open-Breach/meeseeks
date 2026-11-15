//! The options dialog: the renderer and the window lock in one list, the
//! experimental switches in another. What it saves takes effect when the app
//! has been started again.

use std::io;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::pictograms;
use crate::renderer::Renderer;
use crate::settings::Settings;

/// The renderers in the order the dialog lists them.
const RENDERERS: [(Renderer, &str); 3] = [
    (Renderer::Gl, "GL (default)"),
    (Renderer::Vulkan, "Vulkan"),
    (Renderer::Cairo, "Cairo (software)"),
];

/// How wide the descriptions run before they wrap, in characters.
const DESCRIPTION_WIDTH: i32 = 48;

/// What is said beside a setting that an environment variable stands in for.
const RENDERER_OVERRIDDEN: &str = "GSK_RENDERER is set in the environment. This run uses it \
                                   instead of the renderer chosen here.";
const TRIES_OVERRIDDEN: &str = "MEESEEKS_TRY is set in the environment. This run uses it \
                                instead of the two switches here.";

/// Which settings an environment variable is used instead of in this run.
#[derive(Clone, Copy)]
pub struct Overridden {
    pub renderer: bool,
    pub tries: bool,
}

struct Controls {
    renderer: gtk::DropDown,
    copy: gtk::Switch,
    srgb: gtk::Switch,
    lock: gtk::Switch,
}

impl Controls {
    fn show(&self, settings: &Settings) {
        let position = RENDERERS
            .iter()
            .position(|(renderer, _)| *renderer == settings.renderer)
            .unwrap_or(0);
        self.renderer.set_selected(position as u32);
        self.copy.set_active(settings.try_copy);
        self.srgb.set_active(settings.try_srgb);
        self.lock.set_active(settings.lock_window_size);
    }

    /// `settings` with what the controls say in place of what it had.
    fn read(&self, settings: &Settings) -> Settings {
        let renderer = RENDERERS
            .get(self.renderer.selected() as usize)
            .map_or(settings.renderer, |(renderer, _)| *renderer);
        Settings {
            renderer,
            try_copy: self.copy.is_active(),
            try_srgb: self.srgb.is_active(),
            lock_window_size: self.lock.is_active(),
            ..settings.clone()
        }
    }
}

/// Opens the dialog over `parent`, showing `settings`.
///
/// `save` is called with the new settings once the user has changed
/// something, pressed Save and agreed to the restart. It is to write them
/// and start the app again; if it comes back with an error, the dialog shows
/// it and stays.
pub fn show(
    parent: &gtk::ApplicationWindow,
    settings: &Settings,
    overridden: Overridden,
    save: impl Fn(&Settings) -> io::Result<()> + 'static,
) {
    let labels: Vec<&str> = RENDERERS.iter().map(|(_, label)| *label).collect();
    let controls = Rc::new(Controls {
        renderer: gtk::DropDown::from_strings(&labels),
        copy: gtk::Switch::new(),
        srgb: gtk::Switch::new(),
        lock: gtk::Switch::new(),
    });
    controls.show(settings);

    let general = list();
    general.append(&row(
        "Renderer",
        "What GTK draws the window with.",
        overridden.renderer.then_some(RENDERER_OVERRIDDEN),
        &controls.renderer,
    ));
    general.append(&row(
        "Lock window size",
        "The window keeps the size of the virtual monitor, so the picture \
         is never scaled.",
        None,
        &controls.lock,
    ));

    let heading = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_start(4)
        .build();
    heading.append(
        &gtk::Label::builder()
            .label("Experimental")
            .xalign(0.0)
            .css_classes(["mk-heading"])
            .build(),
    );
    if overridden.tries {
        heading.append(&overridden_note(TRIES_OVERRIDDEN));
    }
    let tries = list();
    tries.append(&row(
        "Copy frames",
        "Take the frames as shared memory only, never as DMA-BUF, and copy \
         each one. Same as MEESEEKS_TRY=copy.",
        None,
        &controls.copy,
    ));
    tries.append(&row(
        "Present as sRGB",
        "Tell GTK the frames are plain sRGB, whatever the stream says. Same \
         as MEESEEKS_TRY=srgb.",
        None,
        &controls.srgb,
    ));
    let experimental = gtk::Box::new(gtk::Orientation::Vertical, 8);
    experimental.append(&heading);
    experimental.append(&tries);

    let reset = button("_Reset to defaults", false);
    let cancel = button("_Cancel", false);
    let save_button = button("_Save", true);
    reset.set_hexpand(true);
    reset.set_halign(gtk::Align::Start);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.append(&reset);
    buttons.append(&cancel);
    buttons.append(&save_button);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(20)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    content.append(&general);
    content.append(&experimental);
    content.append(&buttons);
    // The margins belong to the content; the colour has to reach the edges.
    let ground = gtk::Box::builder().css_classes(["mk-dialog"]).build();
    ground.append(&content);

    let dialog = gtk::Window::builder()
        .title("Options")
        .transient_for(parent)
        .modal(true)
        .destroy_with_parent(true)
        .resizable(false)
        .child(&ground)
        .build();

    let resetting = controls.clone();
    reset.connect_clicked(move |_| resetting.show(&Settings::default()));

    let weak = dialog.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(dialog) = weak.upgrade() {
            dialog.close();
        }
    });

    let weak = dialog.downgrade();
    let shown = settings.clone();
    let save = Rc::new(save);
    save_button.connect_clicked(move |_| {
        let Some(dialog) = weak.upgrade() else {
            return;
        };
        let settings = controls.read(&shown);
        if settings == shown {
            dialog.close();
            return;
        }
        let question = gtk::AlertDialog::builder()
            .modal(true)
            .message("Restart Meeseeks?")
            .detail(
                "Meeseeks closes and starts again to apply the new settings. \
                 A running virtual monitor is removed.",
            )
            .buttons(["Cancel", "Save and restart"])
            .cancel_button(0)
            .default_button(1)
            .build();
        let save = save.clone();
        let weak = weak.clone();
        question.choose(Some(&dialog), gio::Cancellable::NONE, move |answer| {
            if answer != Ok(1) {
                return;
            }
            if let Err(err) = save(&settings) {
                log::error!("could not save the settings: {err}");
                gtk::AlertDialog::builder()
                    .modal(true)
                    .message("The settings could not be saved")
                    .detail(err.to_string())
                    .build()
                    .show(weak.upgrade().as_ref());
            }
        });
    });

    // As in any dialog of the desktop: Return saves, Escape leaves.
    dialog.set_default_widget(Some(&save_button));
    let keys = gtk::EventControllerKey::new();
    let weak = dialog.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key != gdk::Key::Escape {
            return glib::Propagation::Proceed;
        }
        if let Some(dialog) = weak.upgrade() {
            dialog.close();
        }
        glib::Propagation::Stop
    });
    dialog.add_controller(keys);

    dialog.present();
}

/// A white card for rows.
fn list() -> gtk::Box {
    // Hidden overflow has the rows follow the card's rounded corners.
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["mk-list"])
        .overflow(gtk::Overflow::Hidden)
        .build()
}

/// One setting: what it is called and what it does on the left, its control
/// on the right. `note` goes under the description.
fn row(
    title: &str,
    description: &str,
    note: Option<&str>,
    control: &(impl IsA<gtk::Widget> + IsA<gtk::Accessible>),
) -> gtk::Box {
    let title = gtk::Label::builder().label(title).xalign(0.0).build();
    let description = small(description, "mk-dim");
    control.update_relation(&[
        gtk::accessible::Relation::LabelledBy(&[title.upcast_ref()]),
        gtk::accessible::Relation::DescribedBy(&[description.upcast_ref()]),
    ]);
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();
    text.append(&title);
    text.append(&description);
    if let Some(note) = note {
        text.append(&overridden_note(note));
    }
    control.set_halign(gtk::Align::End);
    control.set_valign(gtk::Align::Center);
    let row = gtk::Box::builder()
        .spacing(16)
        .css_classes(["mk-row"])
        .build();
    row.append(&text);
    row.append(control);
    row
}

/// A warning sign and a line that says an environment variable is in the way.
fn overridden_note(text: &str) -> gtk::Box {
    let note = gtk::Box::builder().spacing(8).margin_top(4).build();
    note.append(&pictograms::warning());
    note.append(&small(text, "mk-warning"));
    note
}

/// A line of small print that wraps.
fn small(text: &str, class: &str) -> gtk::Label {
    // A wrapping label has to be told its width, or the dialog comes out as
    // tall as the text would be in a narrow column.
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .width_chars(DESCRIPTION_WIDTH)
        .max_width_chars(DESCRIPTION_WIDTH)
        .css_classes([class, "mk-caption"])
        .build()
}

/// A button of the dialog; `suggested` for the one that goes ahead.
fn button(label: &str, suggested: bool) -> gtk::Button {
    let button = gtk::Button::with_mnemonic(label);
    button.add_css_class("mk-button");
    if suggested {
        button.add_css_class("mk-suggested");
    }
    button
}
