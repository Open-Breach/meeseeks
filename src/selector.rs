//! The resolution selector, shown in the middle of the window while no
//! virtual monitor is running: the presets, a custom size and the Start
//! button.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::settings::{MAX_SIDE, MIN_SIDE};

/// The presets, grouped by aspect ratio.
const PRESETS: &[(&str, &[(i32, i32)])] = &[
    (
        "16:9",
        &[(1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)],
    ),
    ("4:3", &[(1024, 768), (1280, 960), (1600, 1200)]),
    ("Vertical", &[(1080, 1920)]),
];

/// How much of the monitor's height is taken to be gone to the desktop's
/// panel and the window's title bar. On Wayland GTK knows only the full size
/// of the monitor, not the area left for windows.
const FIT_MARGIN: i32 = 90;

const CSS: &str = "
.start-button {
  border-radius: 9999px;
  min-width: 84px;
  min-height: 84px;
  padding: 0;
  font-weight: bold;
}
";

pub struct Selector {
    root: gtk::ScrolledWindow,
    content: gtk::Grid,
    screen: gtk::Label,
    presets: Vec<((i32, i32), gtk::ToggleButton)>,
    width: gtk::SpinButton,
    height: gtk::SpinButton,
    warning: gtk::Label,
    start: gtk::Button,
    /// The room there is for the picture on the monitor, once one is known.
    available: Cell<Option<(i32, i32)>>,
    /// Set while the presets and the fields are brought in line, which is
    /// not the user choosing something.
    syncing: Cell<bool>,
    on_change: RefCell<Option<Box<dyn Fn()>>>,
}

impl Selector {
    /// A selector with `size` chosen.
    pub fn new(size: (i32, i32)) -> Rc<Self> {
        let screen = gtk::Label::new(None);

        let content = gtk::Grid::builder()
            .column_spacing(12)
            .row_spacing(12)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        content.attach(&screen, 0, 0, 2, 1);

        let mut presets = Vec::new();
        let mut row = 1;
        for (ratio, sizes) in PRESETS {
            let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            for &(width, height) in *sizes {
                let button = gtk::ToggleButton::with_label(&format!("{width}×{height}"));
                // In one group, so that a click on the chosen preset leaves
                // it chosen.
                if let Some((_, first)) = presets.first() {
                    button.set_group(Some(first));
                }
                buttons.append(&button);
                presets.push(((width, height), button));
            }
            content.attach(&row_label(ratio), 0, row, 1, 1);
            content.attach(&buttons, 1, row, 1, 1);
            row += 1;
        }

        let side = |value: i32| {
            let field = gtk::SpinButton::with_range(f64::from(MIN_SIDE), f64::from(MAX_SIDE), 10.0);
            field.set_numeric(true);
            field.set_width_chars(5);
            field.set_value(f64::from(value));
            field
        };
        let width = side(size.0);
        let height = side(size.1);
        width.set_tooltip_text(Some("Width"));
        height.set_tooltip_text(Some("Height"));
        let warning = gtk::Label::builder()
            .label("Does not fit this screen")
            .css_classes(["warning"])
            .visible(false)
            .build();
        let custom = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        custom.append(&width);
        custom.append(&gtk::Label::new(Some("×")));
        custom.append(&height);
        custom.append(&warning);
        content.attach(&row_label("Custom"), 0, row, 1, 1);
        content.attach(&custom, 1, row, 1, 1);
        row += 1;

        let start = gtk::Button::builder()
            .label("Start")
            .halign(gtk::Align::Center)
            .margin_top(12)
            .css_classes(["start-button", "suggested-action"])
            .build();
        content.attach(&start, 0, row, 2, 1);

        // Scrolling only so that the selector asks for no room of its own:
        // the window is as large as the chosen size, not as the selector.
        let root = gtk::ScrolledWindow::builder().child(&content).build();

        let selector = Rc::new(Self {
            root,
            content,
            screen,
            presets,
            width,
            height,
            warning,
            start,
            available: Cell::new(None),
            syncing: Cell::new(false),
            on_change: RefCell::new(None),
        });

        for (size, button) in &selector.presets {
            let size = *size;
            let weak = Rc::downgrade(&selector);
            button.connect_toggled(move |button| {
                if let Some(selector) = weak.upgrade()
                    && button.is_active()
                    && !selector.syncing.get()
                {
                    selector.choose(size);
                }
            });
        }
        for field in [&selector.width, &selector.height] {
            let weak = Rc::downgrade(&selector);
            field.connect_value_changed(move |_| {
                if let Some(selector) = weak.upgrade()
                    && !selector.syncing.get()
                {
                    selector.sync();
                }
            });
        }
        selector.sync();
        selector
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    /// The chosen size.
    pub fn size(&self) -> (i32, i32) {
        // What has been typed counts, even if the field still has the focus.
        self.width.update();
        self.height.update();
        (self.width.value_as_int(), self.height.value_as_int())
    }

    /// The size the picture can be shown at: the chosen size, or, where there
    /// is no room for that, the chosen size scaled down to the room there is.
    pub fn picture_area(&self) -> (i32, i32) {
        let (width, height) = self.size();
        match self.available.get() {
            Some((room_w, room_h)) if width > room_w || height > room_h => {
                let scale = (f64::from(room_w) / f64::from(width))
                    .min(f64::from(room_h) / f64::from(height));
                // Truncating keeps it within the room.
                (
                    ((f64::from(width) * scale) as i32).max(1),
                    ((f64::from(height) * scale) as i32).max(1),
                )
            }
            _ => (width, height),
        }
    }

    /// Tells the selector which monitor the window is on: its size the way
    /// GTK lays windows out on it, and the scale the desktop shows it at.
    pub fn set_screen(&self, (width, height): (i32, i32), scale: f64) {
        let pixels = |side: i32| (f64::from(side) * scale).round() as i32;
        let text = if (scale - 1.0).abs() < 0.001 {
            format!("Your screen: {width}×{height}")
        } else {
            format!(
                "Your screen: {}×{}, scaled to {width}×{height}",
                pixels(width),
                pixels(height)
            )
        };
        self.screen.set_label(&text);
        self.available.set(Some((width, height - FIT_MARGIN)));
        self.sync();
    }

    /// `f` is called when the chosen size changes, or the room for it.
    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.on_change.replace(Some(Box::new(f)));
    }

    pub fn connect_start(&self, f: impl Fn() + 'static) {
        self.start.connect_clicked(move |_| f());
    }

    /// While busy, nothing in the selector can be used.
    pub fn set_busy(&self, busy: bool) {
        self.content.set_sensitive(!busy);
    }

    pub fn focus_start(&self) {
        self.start.grab_focus();
    }

    fn fits(&self, (width, height): (i32, i32)) -> bool {
        self.available
            .get()
            .is_none_or(|(room_w, room_h)| width <= room_w && height <= room_h)
    }

    fn choose(&self, (width, height): (i32, i32)) {
        self.syncing.set(true);
        self.width.set_value(f64::from(width));
        self.height.set_value(f64::from(height));
        self.syncing.set(false);
        self.sync();
    }

    /// Brings the presets and the warning in line with the chosen size and
    /// the room there is, and passes the change on.
    fn sync(&self) {
        let size = self.size();
        self.syncing.set(true);
        for (preset, button) in &self.presets {
            let fits = self.fits(*preset);
            button.set_sensitive(fits);
            button.set_active(fits && *preset == size);
        }
        self.syncing.set(false);

        self.warning.set_visible(!self.fits(size));
        if let Some((room_w, room_h)) = self.available.get() {
            self.warning.set_tooltip_text(Some(&format!(
                "There is room for {room_w}×{room_h}. The picture will be scaled down."
            )));
        }
        if let Some(on_change) = &*self.on_change.borrow() {
            on_change();
        }
    }
}

fn row_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(1.0)
        .css_classes(["dim-label"])
        .build()
}

/// Makes the Start button round. To be called once GTK is initialised.
pub fn load_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
