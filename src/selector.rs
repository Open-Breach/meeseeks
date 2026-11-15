//! The resolution selector, shown in the middle of the window while no
//! virtual monitor is running: the app's icon and a title, the presets, a
//! custom size and the Start button.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::pictograms;
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

/// The app's icon, at twice the size it is shown at, so that it stays sharp
/// on a screen scaled to 200 %.
const ICON: &[u8] = include_bytes!("../assets/icon/meeseeks-icon-192.png");
const ICON_SIZE: i32 = 96;

/// What the line under the Start button says: before Start, and while the
/// desktop is being asked.
const HINT_IDLE: &str = "Your desktop asks before the new screen appears.";
const HINT_BUSY: &str = "Waiting for your desktop…";

pub struct Selector {
    root: gtk::ScrolledWindow,
    /// The presets and the custom size.
    sizes: gtk::Box,
    screen: gtk::Label,
    presets: Vec<((i32, i32), gtk::ToggleButton)>,
    width: gtk::SpinButton,
    height: gtk::SpinButton,
    warning: gtk::Box,
    warning_text: gtk::Label,
    /// Says why some presets are grey; shown as long as one is.
    legend: gtk::Label,
    start: gtk::Button,
    hint: gtk::Label,
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
        let head = gtk::Box::new(gtk::Orientation::Vertical, 12);
        if let Ok(icon) = gdk::Texture::from_bytes(&glib::Bytes::from_static(ICON)) {
            head.append(
                &gtk::Image::builder()
                    .paintable(&icon)
                    .pixel_size(ICON_SIZE)
                    .accessible_role(gtk::AccessibleRole::Presentation)
                    .build(),
            );
        }
        let screen = gtk::Label::builder().css_classes(["mk-dim"]).build();
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 4);
        titles.append(
            &gtk::Label::builder()
                .label("Choose a size")
                .css_classes(["mk-title-1"])
                .build(),
        );
        titles.append(&screen);
        head.append(&titles);

        // Hidden overflow has the rows and the presets follow the rounded
        // corners of what they are in.
        let list = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["mk-list"])
            .overflow(gtk::Overflow::Hidden)
            .build();
        let mut presets = Vec::new();
        for (ratio, sizes) in PRESETS {
            let group = gtk::Box::builder()
                .css_classes(["mk-segments"])
                .overflow(gtk::Overflow::Hidden)
                .valign(gtk::Align::Center)
                .build();
            for &(width, height) in *sizes {
                let button = gtk::ToggleButton::builder()
                    .label(format!("{width}×{height}"))
                    .css_classes(["mk-segment"])
                    .build();
                // In one group, so that a click on the chosen preset leaves
                // it chosen.
                if let Some((_, first)) = presets.first() {
                    button.set_group(Some(first));
                }
                group.append(&button);
                presets.push(((width, height), button));
            }
            list.append(&row(ratio, &group));
        }

        let side = |value: i32, name: &str| {
            let field = gtk::SpinButton::with_range(f64::from(MIN_SIDE), f64::from(MAX_SIDE), 10.0);
            field.set_numeric(true);
            field.set_width_chars(5);
            field.set_value(f64::from(value));
            field.set_tooltip_text(Some(name));
            field.add_css_class("mk-spin");
            field
        };
        let width = side(size.0, "Width");
        let height = side(size.1, "Height");
        let fields = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        fields.append(&width);
        fields.append(
            &gtk::Label::builder()
                .label("×")
                .css_classes(["mk-dim"])
                .build(),
        );
        fields.append(&height);
        let warning_text = gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["mk-warning", "mk-caption"])
            .build();
        let warning = gtk::Box::builder().spacing(8).visible(false).build();
        let sign = pictograms::warning();
        sign.set_valign(gtk::Align::Center);
        warning.append(&sign);
        warning.append(&warning_text);
        let custom = gtk::Box::new(gtk::Orientation::Vertical, 8);
        custom.append(&fields);
        custom.append(&warning);
        list.append(&row("Custom", &custom));

        let legend = gtk::Label::builder()
            .label("Greyed-out sizes do not fit on this screen.")
            .css_classes(["mk-dim", "mk-caption"])
            .visible(false)
            .build();
        let sizes = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .css_classes(["mk-sizes"])
            .build();
        sizes.append(&list);
        sizes.append(&legend);

        // The button is the place to press and never moves; the cap in it is
        // what is seen, and what the style sheet lifts and sinks. Were the
        // button itself to move, it would slip from under the pointer at its
        // edges. The cap stands 8 pixels proud of the collar it sits in: the
        // room under it is its side wall, which the style sheet draws.
        let cap = gtk::Box::builder().css_classes(["mk-cap"]).build();
        cap.append(
            &gtk::Label::builder()
                .label("Start")
                .hexpand(true)
                .halign(gtk::Align::Center)
                .build(),
        );
        let start = gtk::Button::builder()
            .child(&cap)
            .css_classes(["mk-start"])
            .margin_start(10)
            .margin_end(10)
            .margin_top(2)
            .margin_bottom(18)
            .build();
        start.update_property(&[gtk::accessible::Property::Label("Start")]);
        let collar = gtk::Box::builder()
            .css_classes(["mk-collar"])
            .halign(gtk::Align::Center)
            .build();
        collar.append(&start);
        // The ring that shows the keyboard's focus goes around the collar,
        // which a style sheet cannot tell from the button's state.
        let ring = collar.downgrade();
        start.connect_state_flags_changed(move |start, _| {
            let Some(collar) = ring.upgrade() else {
                return;
            };
            if start.state_flags().contains(gtk::StateFlags::FOCUS_VISIBLE) {
                collar.add_css_class("mk-focus");
            } else {
                collar.remove_css_class("mk-focus");
            }
        });
        let hint = gtk::Label::builder()
            .label(HINT_IDLE)
            .css_classes(["mk-dim", "mk-caption"])
            .build();
        let go = gtk::Box::new(gtk::Orientation::Vertical, 12);
        go.append(&collar);
        go.append(&hint);

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        content.append(&head);
        content.append(&sizes);
        content.append(&go);

        // Scrolling only so that the selector asks for no room of its own:
        // the window is as large as the chosen size, not as the selector.
        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .css_classes(["mk-page"])
            .build();

        let selector = Rc::new(Self {
            root,
            sizes,
            screen,
            presets,
            width,
            height,
            warning,
            warning_text,
            legend,
            start,
            hint,
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
                    selector.changed();
                }
            });
        }
        selector.refresh();
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

    /// Makes `size` the chosen size without that counting as a choice made
    /// in the selector: nobody is told. For when the window has been resized
    /// and the selector is to follow it.
    pub fn set_size(&self, (width, height): (i32, i32)) {
        self.syncing.set(true);
        self.width.set_value(f64::from(width));
        self.height.set_value(f64::from(height));
        self.syncing.set(false);
        self.refresh();
    }

    /// The size the picture can be shown at: the chosen size, or, where there
    /// is no room for that, the chosen size scaled down to the room there is.
    /// Never less than the smallest size there is to choose, which is as
    /// small as the window gets.
    pub fn picture_area(&self) -> (i32, i32) {
        let (width, height) = self.size();
        match self.available.get() {
            Some((room_w, room_h)) if width > room_w || height > room_h => {
                let scale = (f64::from(room_w) / f64::from(width))
                    .min(f64::from(room_h) / f64::from(height));
                // Truncating keeps it within the room.
                (
                    ((f64::from(width) * scale) as i32).max(MIN_SIDE),
                    ((f64::from(height) * scale) as i32).max(MIN_SIDE),
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
        let available = Some((width, height - FIT_MARGIN));
        if self.available.replace(available) != available {
            self.changed();
        }
    }

    /// `f` is called when a size is chosen in the selector, or the room for
    /// the chosen size changes.
    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.on_change.replace(Some(Box::new(f)));
    }

    pub fn connect_start(&self, f: impl Fn() + 'static) {
        self.start.connect_clicked(move |_| f());
    }

    /// While busy, no size can be chosen and Start stays pressed in; the
    /// line under it says what the wait is for.
    pub fn set_busy(&self, busy: bool) {
        self.sizes.set_sensitive(!busy);
        self.start.set_sensitive(!busy);
        self.hint
            .set_label(if busy { HINT_BUSY } else { HINT_IDLE });
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
        self.changed();
    }

    /// A size has been chosen, or the room for it is another.
    fn changed(&self) {
        self.refresh();
        if let Some(on_change) = &*self.on_change.borrow() {
            on_change();
        }
    }

    /// Brings the presets, the warning and the line about grey presets in
    /// line with the chosen size and the room there is. A preset is shown as
    /// chosen when the size is exactly its own; any other size is a custom
    /// one.
    fn refresh(&self) {
        // From the start, since reading the size can change a field too.
        self.syncing.set(true);
        let size = self.size();
        let mut all_fit = true;
        for (preset, button) in &self.presets {
            let fits = self.fits(*preset);
            all_fit &= fits;
            button.set_sensitive(fits);
            button.set_active(fits && *preset == size);
        }
        self.syncing.set(false);
        self.legend.set_visible(!all_fit);

        self.warning.set_visible(!self.fits(size));
        if let Some((room_w, room_h)) = self.available.get() {
            self.warning_text.set_label(&format!(
                "Does not fit this screen. There is room for {room_w}×{room_h}."
            ));
        }
    }
}

/// One row of the list: what it holds on the left, the control beside it.
fn row(name: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::builder()
        .spacing(16)
        .css_classes(["mk-row"])
        .build();
    row.append(
        &gtk::Label::builder()
            .label(name)
            .xalign(0.0)
            .width_request(72)
            .css_classes(["mk-dim"])
            .build(),
    );
    row.append(control);
    row
}
