//! The options dialog: the renderer, the experimental switches and the window
//! lock. What it saves takes effect when the app has been started again.

use std::io;
use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

use crate::renderer::Renderer;
use crate::settings::Settings;

/// The renderers in the order the dialog lists them.
const RENDERERS: [(Renderer, &str); 3] = [
    (Renderer::Gl, "GL (default)"),
    (Renderer::Vulkan, "Vulkan"),
    (Renderer::Cairo, "Cairo (software)"),
];

/// How wide the descriptions run before they wrap, in characters.
const DESCRIPTION_WIDTH: i32 = 46;

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

    let grid = gtk::Grid::builder()
        .column_spacing(24)
        .row_spacing(12)
        .build();
    let mut row = 0;
    let mut add = |title: &str, description: &str, control: &gtk::Widget| {
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.append(&gtk::Label::builder().label(title).xalign(0.0).build());
        text.append(&note(description, "dim-label"));
        text.set_hexpand(true);
        control.set_halign(gtk::Align::End);
        control.set_valign(gtk::Align::Center);
        grid.attach(&text, 0, row, 1, 1);
        grid.attach(control, 1, row, 1, 1);
        row += 1;
    };
    add(
        "Renderer",
        "What GTK draws the window with.",
        controls.renderer.upcast_ref(),
    );
    add(
        "Copy frames",
        "Experimental. Take the frames as shared memory only, never as \
         DMA-BUF, and copy each one. Same as MEESEEKS_TRY=copy.",
        controls.copy.upcast_ref(),
    );
    add(
        "Present as sRGB",
        "Experimental. Tell GTK the frames are plain sRGB, whatever the \
         stream says. Same as MEESEEKS_TRY=srgb.",
        controls.srgb.upcast_ref(),
    );
    add(
        "Lock window size",
        "The window keeps the size of the virtual monitor, so the picture \
         is never scaled.",
        controls.lock.upcast_ref(),
    );
    if overridden.renderer {
        grid.attach(
            &note(
                "GSK_RENDERER is set in the environment. This run uses it \
                 instead of the renderer chosen here.",
                "warning",
            ),
            0,
            row,
            2,
            1,
        );
        row += 1;
    }
    if overridden.tries {
        grid.attach(
            &note(
                "MEESEEKS_TRY is set in the environment. This run uses it \
                 instead of the two experimental switches here.",
                "warning",
            ),
            0,
            row,
            2,
            1,
        );
    }

    let reset = gtk::Button::with_mnemonic("_Reset to defaults");
    let cancel = gtk::Button::with_mnemonic("_Cancel");
    let save_button = gtk::Button::with_mnemonic("_Save");
    save_button.add_css_class("suggested-action");
    reset.set_hexpand(true);
    reset.set_halign(gtk::Align::Start);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    buttons.append(&reset);
    buttons.append(&cancel);
    buttons.append(&save_button);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&grid);
    content.append(&buttons);

    let dialog = gtk::Window::builder()
        .title("Options")
        .transient_for(parent)
        .modal(true)
        .destroy_with_parent(true)
        .resizable(false)
        .child(&content)
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

    dialog.present();
}

/// A line of small print that wraps.
fn note(text: &str, class: &str) -> gtk::Label {
    // A wrapping label has to be told its width, or the dialog comes out as
    // tall as the text would be in a narrow column.
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .width_chars(DESCRIPTION_WIDTH)
        .max_width_chars(DESCRIPTION_WIDTH)
        .css_classes([class])
        .build()
}
