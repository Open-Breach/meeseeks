//! The few pictograms the app draws itself, so that they look the same under
//! any icon theme: the marks of the startup checks and the warning sign.

use std::f64::consts::PI;

use gtk::cairo;
use gtk::prelude::*;

const GREEN: (f64, f64, f64) = (0.106, 0.522, 0.325);
const RED: (f64, f64, f64) = (0.753, 0.110, 0.157);
const AMBER: (f64, f64, f64) = (0.898, 0.647, 0.039);
const INK: (f64, f64, f64) = (0.180, 0.204, 0.212);
const WHITE: (f64, f64, f64) = (1.0, 1.0, 1.0);

/// A drawing of `size` pixels a side, named `name` for screen readers.
fn pictogram(size: i32, name: &str, draw: impl Fn(&cairo::Context) + 'static) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(size)
        .content_height(size)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Start)
        .accessible_role(gtk::AccessibleRole::Img)
        .build();
    area.update_property(&[gtk::accessible::Property::Label(name)]);
    area.set_draw_func(move |_, cr, _, _| draw(cr));
    area
}

fn colour(cr: &cairo::Context, (r, g, b): (f64, f64, f64)) {
    cr.set_source_rgb(r, g, b);
}

/// A line with round ends through `points`, in the colour and width set.
fn line(cr: &cairo::Context, points: &[(f64, f64)]) {
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            cr.move_to(x, y);
        } else {
            cr.line_to(x, y);
        }
    }
    // Drawing can only fail once the surface is gone.
    let _ = cr.stroke();
}

fn disc(cr: &cairo::Context, fill: (f64, f64, f64)) {
    colour(cr, fill);
    cr.arc(10.0, 10.0, 9.0, 0.0, 2.0 * PI);
    let _ = cr.fill();
}

/// A check that passed: a white tick on a green disc.
pub fn pass() -> gtk::DrawingArea {
    pictogram(20, "Passed", |cr| {
        disc(cr, GREEN);
        colour(cr, WHITE);
        cr.set_line_width(1.8);
        line(cr, &[(6.0, 10.3), (8.7, 13.0), (14.2, 7.4)]);
    })
}

/// A check that failed: a white cross on a red disc.
pub fn fail() -> gtk::DrawingArea {
    pictogram(20, "Failed", |cr| {
        disc(cr, RED);
        colour(cr, WHITE);
        cr.set_line_width(1.8);
        line(cr, &[(6.8, 6.8), (13.2, 13.2)]);
        line(cr, &[(13.2, 6.8), (6.8, 13.2)]);
    })
}

/// Something worth telling: an amber triangle with an exclamation mark.
pub fn warning() -> gtk::DrawingArea {
    pictogram(20, "Warning", |cr| {
        colour(cr, AMBER);
        cr.set_line_width(1.6);
        cr.set_line_join(cairo::LineJoin::Round);
        cr.move_to(10.0, 3.2);
        cr.line_to(17.4, 16.4);
        cr.line_to(2.6, 16.4);
        cr.close_path();
        let _ = cr.fill_preserve();
        let _ = cr.stroke();
        colour(cr, INK);
        cr.set_line_width(1.8);
        line(cr, &[(10.0, 7.8), (10.0, 11.8)]);
        cr.arc(10.0, 14.5, 1.0, 0.0, 2.0 * PI);
        let _ = cr.fill();
    })
}
